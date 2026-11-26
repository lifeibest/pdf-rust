import clsx from "clsx";
import {
  DndContext,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import { SortableContext, arrayMove, rectSortingStrategy, useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import {
  GripVertical,
  Loader2,
  RotateCcw,
  RotateCw,
  Trash2,
  Undo2,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { DropZone } from "@/components/DropZone";
import { FileList } from "@/components/FileList";
import { Notice } from "@/components/Notice";
import { OutputPicker } from "@/components/OutputPicker";
import { Section } from "@/components/Section";
import { TaskProgress } from "@/components/TaskProgress";
import type { FeaturePageProps } from "@/features/types";
import { usePendingDrop } from "@/hooks/useDropFiles";
import { useOutputPicker } from "@/hooks/useOutputPicker";
import { usePdfThumbs } from "@/hooks/usePdfThumbs";
import { useSourceFiles } from "@/hooks/useSourceFiles";
import { useTaskRunner } from "@/hooks/useTaskRunner";
import { useSettings } from "@/store/settings";

interface EditablePage {
  /** 稳定键，等于源文档 0-based 页索引的字符串形式 */
  id: string;
  pageIndex: number;
  rotateDeg: number;
  deleted: boolean;
}

function buildInitialPages(total: number): EditablePage[] {
  return Array.from({ length: total }, (_, index) => ({
    id: String(index),
    pageIndex: index,
    rotateDeg: 0,
    deleted: false,
  }));
}

/**
 * 可视化页面整理页：缩略图网格支持拖拽换序、单页旋转 90° 与删页/撤销。
 */
export default function OrganizePage({ feature }: FeaturePageProps): ReactElement {
  const { t } = useTranslation();
  const source = useSourceFiles(feature);
  const output = useOutputPicker({ enabled: true });
  const runner = useTaskRunner(feature.id);
  const thumbMaxPx = useSettings((state) => state.settings.thumbMaxPx);
  const notifyOnFinish = useSettings((state) => state.settings.notifyOnFinish);

  usePendingDrop(feature.id, source.add);

  const primary = source.files.length > 0 ? source.files[0] : null;
  const pageCount = primary?.doc?.pageCount ?? 0;

  const thumbs = usePdfThumbs(primary?.path ?? null, {
    password: primary?.password ?? null,
    total: pageCount,
    maxPx: thumbMaxPx,
  });

  const [pages, setPages] = useState<EditablePage[]>([]);

  // 切换源文档或探测出页数时重置编辑状态
  useEffect(() => {
    setPages(pageCount > 0 ? buildInitialPages(pageCount) : []);
  }, [primary?.path, pageCount]);

  const thumbUrlByIndex = useMemo(() => {
    const map = new Map<number, string>();
    for (const item of thumbs.thumbs) {
      map.set(item.index, item.url);
    }
    return map;
  }, [thumbs.thumbs]);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
  );

  const handleDragEnd = useCallback((event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    setPages((current) => {
      const from = current.findIndex((item) => item.id === active.id);
      const to = current.findIndex((item) => item.id === over.id);
      if (from < 0 || to < 0) return current;
      return arrayMove(current, from, to);
    });
  }, []);

  const rotatePage = useCallback((id: string) => {
    setPages((current) =>
      current.map((item) =>
        item.id === id ? { ...item, rotateDeg: (item.rotateDeg + 90) % 360 } : item,
      ),
    );
  }, []);

  const toggleDelete = useCallback((id: string) => {
    setPages((current) =>
      current.map((item) => (item.id === id ? { ...item, deleted: !item.deleted } : item)),
    );
  }, []);

  const resetPages = useCallback(() => {
    if (pageCount > 0) setPages(buildInitialPages(pageCount));
  }, [pageCount]);

  const kept = useMemo(() => pages.filter((p) => !p.deleted), [pages]);
  const deletedCount = pages.length - kept.length;

  const hint = useMemo(() => {
    if (!primary) return t("feature.needOneFile");
    if (source.needsPassword) return t("errors.passwordRequired");
    if (pageCount > 0 && kept.length === 0) return t("organize.needAtLeastOne");
    return null;
  }, [kept.length, pageCount, primary, source.needsPassword, t]);

  const canRun = hint === null && !source.busy && !runner.running && kept.length > 0;

  const handleRun = useCallback(async () => {
    if (!canRun || !primary) return;
    const dir = await output.resolve();
    if (!dir) return;

    const record = await runner.start(
      {
        kind: "organize",
        options: {
          source: primary.path,
          password: primary.password ?? null,
          pages: kept.map((item) => ({
            pageIndex: item.pageIndex,
            rotateDeg: item.rotateDeg,
          })),
          outputName: output.name.trim().length > 0 ? output.name.trim() : null,
        },
      },
      {
        outDir: dir,
        password: primary.password ?? null,
        notify: notifyOnFinish,
      },
    );
    if (record) output.commit(dir);
  }, [canRun, kept, notifyOnFinish, output, primary, runner]);

  const runAnother = useCallback(() => {
    source.clear();
    runner.reset();
    setPages([]);
  }, [runner, source]);

  return (
    <div className="flex flex-col gap-5">
      <p className="text-sm text-ink-muted">{t(feature.descKey)}</p>

      {source.rejected.length > 0 ? (
        <Notice tone="warn" onDismiss={source.dismissRejected}>
          {t("drop.rejected", { names: source.rejected.join("、") })}
        </Notice>
      ) : null}

      <div className="grid items-start gap-5 xl:grid-cols-[minmax(0,1fr)_21rem]">
        <div className="flex min-w-0 flex-col gap-5">
          <Section
            title={t("feature.singleSource")}
            action={
              source.files.length > 0 ? (
                <button
                  type="button"
                  className="btn-quiet h-7 px-2 py-0 text-xs"
                  onClick={source.clear}
                >
                  <Trash2 size={13} />
                  {t("common.clear")}
                </button>
              ) : undefined
            }
          >
            <div className="flex flex-col gap-3">
              <DropZone
                feature={feature}
                busy={source.busy}
                compact={source.files.length > 0}
                onPick={() => void source.pick()}
              />
              {source.files.length > 0 ? (
                <FileList
                  files={source.files}
                  onRemove={source.remove}
                  onReorder={source.reorder}
                  onPassword={source.setPassword}
                />
              ) : null}
            </div>
          </Section>

          {pages.length > 0 ? (
            <Section
              title={t("organize.boardTitle")}
              hint={t("organize.boardHint")}
              action={
                <button
                  type="button"
                  className="btn-quiet h-7 px-2 py-0 text-xs"
                  onClick={resetPages}
                  disabled={runner.running}
                >
                  <RotateCcw size={13} />
                  {t("common.reset")}
                </button>
              }
            >
              <div className="mb-3 flex flex-wrap items-center gap-2">
                <span className="chip">{t("organize.keptCount", { count: kept.length })}</span>
                {deletedCount > 0 ? (
                  <span className="chip border-rose-300/60 bg-rose-50 text-rose-600 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-300">
                    {t("organize.deletedCount", { count: deletedCount })}
                  </span>
                ) : null}
                {thumbs.loading ? (
                  <span className="inline-flex items-center gap-1 text-[11px] text-ink-faint">
                    <Loader2 size={12} className="animate-spin" />
                    {t("thumb.rendering")}
                  </span>
                ) : null}
              </div>

              <DndContext
                sensors={sensors}
                collisionDetection={closestCenter}
                onDragEnd={handleDragEnd}
              >
                <SortableContext items={pages.map((p) => p.id)} strategy={rectSortingStrategy}>
                  <div className="grid grid-cols-3 gap-2.5 sm:grid-cols-4 lg:grid-cols-5">
                    {pages.map((item, position) => (
                      <SortablePageCard
                        key={item.id}
                        item={item}
                        position={position}
                        thumbUrl={thumbUrlByIndex.get(item.pageIndex) ?? null}
                        disabled={runner.running}
                        onRotate={rotatePage}
                        onToggleDelete={toggleDelete}
                      />
                    ))}
                  </div>
                </SortableContext>
              </DndContext>
            </Section>
          ) : null}
        </div>

        <aside className="flex min-w-0 flex-col gap-5">
          <Section title={t("feature.output")}>
            <OutputPicker api={output} disabled={runner.running} />
          </Section>

          <TaskProgress
            feature={feature}
            runner={runner}
            canRun={canRun}
            hint={hint ?? undefined}
            onRun={() => void handleRun()}
            onRunAnother={runAnother}
          />
        </aside>
      </div>
    </div>
  );
}

interface SortablePageCardProps {
  item: EditablePage;
  position: number;
  thumbUrl: string | null;
  disabled: boolean;
  onRotate(id: string): void;
  onToggleDelete(id: string): void;
}

function SortablePageCard({
  item,
  position,
  thumbUrl,
  disabled,
  onRotate,
  onToggleDelete,
}: SortablePageCardProps): ReactElement {
  const { t } = useTranslation();
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: item.id,
    disabled,
  });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  const originalNum = item.pageIndex + 1;

  return (
    <div
      ref={setNodeRef}
      style={style}
      className={clsx(
        "group relative flex flex-col overflow-hidden rounded-xl border bg-surface-subtle transition-all",
        isDragging ? "z-20 border-brand-400 shadow-lg" : "border-line",
        item.deleted && "opacity-45",
      )}
    >
      {/* 顶部拖拽区 + 页码 */}
      <div
        {...attributes}
        {...listeners}
        className="flex cursor-grab items-center justify-between gap-1 border-b border-line/60 bg-surface px-2 py-1 active:cursor-grabbing"
      >
        <span className="inline-flex items-center gap-1 text-[11px] font-medium tabular-nums text-ink-muted">
          <GripVertical size={12} className="text-ink-faint" />
          {position + 1}
          {originalNum !== position + 1 ? (
            <span className="text-[10px] text-ink-faint">({t("organize.origPage", { page: originalNum })})</span>
          ) : null}
        </span>
        {item.rotateDeg !== 0 ? (
          <span className="rounded bg-brand-50 px-1 text-[10px] font-medium text-brand-600 dark:bg-brand-950/50 dark:text-brand-300">
            {item.rotateDeg}°
          </span>
        ) : null}
      </div>

      {/* 缩略图区域 */}
      <div className="relative flex aspect-[3/4] items-center justify-center p-1.5">
        {thumbUrl ? (
          <img
            src={thumbUrl}
            alt={String(originalNum)}
            loading="lazy"
            draggable={false}
            className="max-h-full max-w-full object-contain shadow-sm transition-transform duration-200"
            style={
              item.rotateDeg !== 0
                ? { transform: `rotate(${item.rotateDeg}deg) scale(0.86)` }
                : undefined
            }
          />
        ) : (
          <span className="text-xs font-medium text-ink-faint">P{originalNum}</span>
        )}
        {item.deleted ? (
          <span className="absolute inset-0 flex items-center justify-center bg-black/15">
            <span className="rounded-md bg-rose-600 px-1.5 py-0.5 text-[10px] font-medium text-white">
              {t("organize.deletedTag")}
            </span>
          </span>
        ) : null}
      </div>

      {/* 底部快捷按钮：旋转 / 删除或撤销 */}
      <div className="flex items-center justify-between border-t border-line/60 bg-surface px-1.5 py-1">
        <button
          type="button"
          className="btn-quiet h-6 px-1.5 py-0 text-[11px]"
          disabled={disabled || item.deleted}
          onClick={() => onRotate(item.id)}
          title={t("organize.rotatePage")}
        >
          <RotateCw size={12} />
        </button>
        <button
          type="button"
          className={clsx(
            "btn-quiet h-6 px-1.5 py-0 text-[11px]",
            item.deleted ? "text-brand-600" : "hover:text-rose-500",
          )}
          disabled={disabled}
          onClick={() => onToggleDelete(item.id)}
          title={item.deleted ? t("organize.restorePage") : t("organize.deletePage")}
        >
          {item.deleted ? <Undo2 size={12} /> : <Trash2 size={12} />}
        </button>
      </div>
    </div>
  );
}
