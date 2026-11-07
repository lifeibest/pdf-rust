import clsx from "clsx";
import { FolderUp, Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";

import { extname } from "@/lib/paths";
import { matchFeatures } from "@/features/registry";
import { useDrop } from "@/store/drop";
import type { FeatureDefinition } from "@/features/types";

export interface DropZoneProps {
  feature: FeatureDefinition;
  onPick(): void;
  /** 正在读取文件信息时禁用交互 */
  busy?: boolean;
  /** 已有文件时收成一行紧凑按钮，把版面留给列表 */
  compact?: boolean;
}

/**
 * 文件投放区。
 *
 * 原生的窗口拖拽由 `useDropFiles` 统一接管（Tauri 会拦下 HTML5 拖放），
 * 因此这里不再绑定 `onDrop`，只负责「点击选择」与拖拽时的视觉反馈。
 */
export function DropZone({ feature, onPick, busy = false, compact = false }: DropZoneProps) {
  const { t } = useTranslation();
  const dragging = useDrop((state) => state.dragging);
  const hoverPaths = useDrop((state) => state.hoverPaths);

  // 只有拖入的类型确实归本功能处理时才高亮，避免每个页面都在闪
  const targeted =
    dragging && matchFeatures(hoverPaths.map(extname))[0]?.id === feature.id;

  if (compact) {
    return (
      <button
        type="button"
        onClick={onPick}
        disabled={busy}
        className={clsx(
          "btn-ghost w-full justify-start",
          targeted && "border-brand-400 bg-brand-50 text-brand-700 dark:bg-brand-950/40",
        )}
      >
        {busy ? <Loader2 size={16} className="animate-spin" /> : <FolderUp size={16} />}
        {t("common.addFiles")}
      </button>
    );
  }

  return (
    <button
      type="button"
      onClick={onPick}
      disabled={busy}
      className={clsx(
        "group flex w-full flex-col items-center justify-center gap-2 rounded-2xl",
        "border-2 border-dashed border-line bg-surface px-6 py-10 text-center",
        "transition-colors duration-150 hover:border-brand-400 hover:bg-brand-50/40",
        "disabled:pointer-events-none disabled:opacity-60",
        "dark:hover:bg-brand-950/20",
        targeted && "border-brand-500 bg-brand-50 dark:bg-brand-950/40",
      )}
    >
      <span
        className={clsx(
          "inline-flex h-12 w-12 items-center justify-center rounded-2xl",
          "bg-surface-subtle text-ink-muted transition-colors group-hover:text-brand-600",
          targeted && "bg-brand-100 text-brand-600 dark:bg-brand-900/50",
        )}
      >
        {busy ? <Loader2 size={22} className="animate-spin" /> : <FolderUp size={22} />}
      </span>
      <span className="text-sm font-medium text-ink">
        {targeted ? t("drop.dragActive") : t("drop.title")}
      </span>
      <span className="text-xs text-ink-faint">
        {t("drop.accepted", { ext: formatExtensions(feature.accepts.ext) })}
      </span>
      <span className="mt-1 text-xs text-ink-faint">{t("drop.hint")}</span>
    </button>
  );
}

/** 把扩展名列表压成一行人类可读的文本，过长时截断。 */
function formatExtensions(ext: string[]): string {
  const labels = ext.map((item) => item.toUpperCase());
  const shown = labels.slice(0, 6);
  return shown.length < labels.length ? `${shown.join(" / ")} …` : shown.join(" / ");
}
