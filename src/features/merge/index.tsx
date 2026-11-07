import clsx from "clsx";
import { FileText, Layers, RotateCcw, Trash2 } from "lucide-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { DropZone } from "@/components/DropZone";
import { FileList } from "@/components/FileList";
import { Notice } from "@/components/Notice";
import { OptionsForm } from "@/components/OptionsForm";
import { OutputPicker } from "@/components/OutputPicker";
import { Section } from "@/components/Section";
import { TaskProgress } from "@/components/TaskProgress";
import type { FeaturePageProps, SourceFile } from "@/features/types";
import { useFeatureController } from "@/hooks/useFeatureController";
import { useFirstPageThumbs } from "@/hooks/useFirstPageThumbs";
import { formatBytes } from "@/lib/format";
import { extname } from "@/lib/paths";

/**
 * PDF 合并页。
 *
 * 相比通用页多出两块合并特有的信息：来源汇总（几个文件、合计多少页）与
 * 「顺序预览条」—— 合并的输出顺序完全由列表顺序决定，把每个来源的第一页
 * 平铺出来，用户不用运行就能确认排序是否正确。
 */
export default function MergePage({ feature }: FeaturePageProps): ReactElement {
  const { t } = useTranslation();
  const controller = useFeatureController(feature);
  const { source, output, runner, values, setValue, resetValues } = controller;

  const thumbs = useFirstPageThumbs(source.files, source.files.length > 0);
  const totals = summarize(source.files);

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
            title={t("feature.source")}
            hint={t("feature.sourceHint")}
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
                <>
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="chip">
                      <Layers size={12} />
                      {t("merge.files", { count: source.files.length })}
                    </span>
                    <span className="chip">
                      <FileText size={12} />
                      {t("merge.pages", { count: totals.pages })}
                    </span>
                    <span className="chip">{t("common.totalSize", { size: totals.size })}</span>
                    {source.busy ? (
                      <span className="text-[11px] text-ink-faint">{t("common.loading")}</span>
                    ) : null}
                  </div>

                  <FileList
                    files={source.files}
                    sortable
                    onRemove={source.remove}
                    onReorder={source.reorder}
                    onPassword={source.setPassword}
                  />
                </>
              ) : null}
            </div>
          </Section>

          {source.files.length > 0 ? (
            <Section title={t("merge.orderPreview")} hint={t("merge.orderHint")}>
              {source.needsPassword ? (
                <Notice tone="info">{t("merge.lockedHint")}</Notice>
              ) : null}
              <ThumbStrip files={source.files} urls={thumbs.urls} />
            </Section>
          ) : null}
        </div>

        <aside className="flex min-w-0 flex-col gap-5">
          <Section
            title={t("feature.options")}
            action={
              <button
                type="button"
                className="btn-quiet h-7 w-7 rounded-lg px-0 py-0"
                onClick={resetValues}
                title={t("common.reset")}
                aria-label={t("common.reset")}
              >
                <RotateCcw size={13} />
              </button>
            }
          >
            <OptionsForm
              fields={feature.options}
              values={values}
              onChange={setValue}
              disabled={runner.running}
            />
          </Section>

          <Section title={t("feature.output")}>
            <OutputPicker api={output} disabled={runner.running} />
          </Section>

          <TaskProgress
            feature={feature}
            runner={runner}
            canRun={controller.canRun}
            hint={controller.hint ?? undefined}
            onRun={() => controller.run()}
            onRunAnother={controller.runAnother}
          />
        </aside>
      </div>
    </div>
  );
}

/** 汇总来源列表：页数只统计已探测到的 PDF，体积始终可算。 */
function summarize(files: SourceFile[]): { pages: number; size: string } {
  const pages = files.reduce((sum, file) => sum + (file.doc?.pageCount ?? 0), 0);
  const bytes = files.reduce((sum, file) => sum + file.size, 0);
  return { pages, size: formatBytes(bytes) };
}

interface ThumbStripProps {
  files: SourceFile[];
  /** key 为 `SourceFile.id`；图片没有缩略图，因此不会出现在这里 */
  urls: Record<string, string>;
}

/** 横向滚动的首页缩略图条，顺序与来源列表一致。 */
function ThumbStrip({ files, urls }: ThumbStripProps): ReactElement {
  const { t } = useTranslation();

  return (
    <ol className="mt-3 flex gap-2 overflow-x-auto pb-1">
      {files.map((file, index) => {
        const url = urls[file.id];
        const isPdf = extname(file.path) === "pdf";
        return (
          <li
            key={file.id}
            className="flex w-24 shrink-0 flex-col gap-1"
            title={`${index + 1}. ${file.name}`}
          >
            <span className="relative flex aspect-[3/4] items-center justify-center overflow-hidden rounded-xl border border-line bg-surface-subtle">
              {url ? (
                <img
                  src={url}
                  alt={file.name}
                  loading="lazy"
                  draggable={false}
                  className="max-h-full max-w-full object-contain"
                />
              ) : (
                // 图片不能走 asset 协议预览（作用域只到缓存目录），退化为扩展名占位
                <span className="text-[11px] font-medium uppercase text-ink-faint">
                  {extname(file.path) || t("common.unknown")}
                </span>
              )}
              <span className="absolute left-1 top-1 rounded-md bg-black/45 px-1 text-[10px] font-medium text-white">
                {index + 1}
              </span>
            </span>
            <span className={clsx("truncate text-[11px]", isPdf ? "text-ink-muted" : "text-sky-600 dark:text-sky-400")}>
              {file.name}
            </span>
          </li>
        );
      })}
    </ol>
  );
}
