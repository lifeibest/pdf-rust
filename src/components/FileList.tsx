import clsx from "clsx";
import {
  closestCenter,
  DndContext,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { FileText, GripVertical, ImageIcon, Lock, X } from "lucide-react";
import { useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { formatBytes } from "@/lib/format";
import { extname, shortenPath } from "@/lib/paths";
import type { SourceFile } from "@/features/types";

export interface FileListProps {
  files: SourceFile[];
  /** 多来源功能才允许拖拽排序 */
  sortable?: boolean;
  onRemove(id: string): void;
  onReorder(activeId: string, overId: string): void;
  /** 提供时，加密文档会显示密码输入行 */
  onPassword?(id: string, password: string): Promise<void> | void;
}

/** 来源文件列表：序号、类型、体积、页数，可选拖拽排序与密码输入。 */
export function FileList({
  files,
  sortable = false,
  onRemove,
  onReorder,
  onPassword,
}: FileListProps): ReactElement {
  const sensors = useSensors(
    // 4px 的激活距离：既能拖拽排序，又不会吃掉按钮上的普通点击
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
  );

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    onReorder(String(active.id), String(over.id));
  };

  return (
    <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
      <SortableContext items={files.map((file) => file.id)} strategy={verticalListSortingStrategy}>
        <ul className="flex flex-col gap-2">
          {files.map((file, index) => (
            <FileRow
              key={file.id}
              file={file}
              index={index}
              sortable={sortable}
              onRemove={onRemove}
              onPassword={onPassword}
            />
          ))}
        </ul>
      </SortableContext>
    </DndContext>
  );
}

interface FileRowProps {
  file: SourceFile;
  index: number;
  sortable: boolean;
  onRemove(id: string): void;
  onPassword?(id: string, password: string): Promise<void> | void;
}

function FileRow({ file, index, sortable, onRemove, onPassword }: FileRowProps): ReactElement {
  const { t } = useTranslation();
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: file.id,
    disabled: !sortable,
  });

  const [password, setPassword] = useState("");
  const isPdf = extname(file.path) === "pdf";
  const locked = file.doc?.encrypted === true && file.doc.pageCount === 0;

  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Transform.toString(transform), transition }}
      className={clsx(
        "card flex flex-col gap-2 p-3",
        isDragging && "z-10 shadow-card-hover ring-2 ring-brand-500/30",
      )}
    >
      <div className="flex items-center gap-3">
        {sortable ? (
          <span
            className="cursor-grab touch-none text-ink-faint hover:text-ink-muted active:cursor-grabbing"
            {...attributes}
            {...listeners}
            aria-label={t("feature.sourceHint")}
          >
            <GripVertical size={16} />
          </span>
        ) : (
          <span className="w-5 text-center text-xs font-medium text-ink-faint">{index + 1}</span>
        )}

        <span
          className={clsx(
            "inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-xl",
            isPdf ? "bg-rose-50 text-rose-500 dark:bg-rose-500/15" : "bg-sky-50 text-sky-500 dark:bg-sky-500/15",
          )}
        >
          {isPdf ? <FileText size={16} /> : <ImageIcon size={16} />}
        </span>

        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium text-ink" title={file.name}>
            {file.name}
          </p>
          <p className="truncate text-xs text-ink-faint" title={file.path}>
            {shortenPath(file.path)}
          </p>
        </div>

        <div className="shrink-0 text-right">
          <p className="text-xs text-ink-muted">
            {file.doc && file.doc.pageCount > 0
              ? t("common.pageCount", { value: file.doc.pageCount })
              : formatBytes(file.size)}
          </p>
          {file.doc && file.doc.pageCount > 0 ? (
            <p className="text-[11px] text-ink-faint">{formatBytes(file.size)}</p>
          ) : null}
        </div>

        <button
          type="button"
          onClick={() => onRemove(file.id)}
          className="btn-quiet h-8 w-8 shrink-0 rounded-lg px-0 py-0"
          aria-label={t("common.remove")}
          title={t("common.remove")}
        >
          <X size={15} />
        </button>
      </div>

      {locked && onPassword ? (
        <div className="flex items-center gap-2 rounded-xl bg-surface-subtle px-3 py-2">
          <Lock size={14} className="shrink-0 text-ink-faint" />
          <input
            type="password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            placeholder={t("common.passwordPlaceholder")}
            className="field h-8 flex-1 py-1 text-xs"
          />
          <button
            type="button"
            className="btn-ghost h-8 px-3 py-0 text-xs"
            disabled={password.length === 0}
            onClick={() => void onPassword(file.id, password)}
          >
            {t("common.save")}
          </button>
        </div>
      ) : null}
    </li>
  );
}
