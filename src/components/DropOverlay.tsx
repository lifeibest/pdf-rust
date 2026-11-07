import { FolderInput } from "lucide-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { extname } from "@/lib/paths";
import { matchFeatures } from "@/features/registry";
import { useDrop } from "@/store/drop";

/**
 * 全局拖拽遮罩。
 *
 * 只要窗口内有文件在拖动就铺满整屏，并预告松手后会发生什么：
 * 命中唯一功能显示功能名，多个命中提示将出现选择器，没有命中则直接说明。
 */
export function DropOverlay(): ReactElement | null {
  const { t } = useTranslation();
  const dragging = useDrop((state) => state.dragging);
  const hoverPaths = useDrop((state) => state.hoverPaths);

  if (!dragging) return null;

  const candidates = matchFeatures(hoverPaths.map(extname));
  const [best] = candidates;
  const summary =
    candidates.length === 0
      ? t("drop.noMatch")
      : candidates.length === 1
        ? t(best.titleKey)
        : t("home.droppedMultiple");

  return (
    <div className="pointer-events-none fixed inset-0 z-50 flex items-center justify-center bg-brand-950/35 p-8 backdrop-blur-[2px]">
      <div className="flex max-w-md flex-col items-center gap-3 rounded-3xl border-2 border-dashed border-white/70 bg-surface/90 px-10 py-8 text-center shadow-panel">
        <span className="inline-flex h-14 w-14 items-center justify-center rounded-2xl bg-brand-500 text-white">
          <FolderInput size={26} />
        </span>
        <p className="text-base font-semibold text-ink">{t("drop.dragActive")}</p>
        <p className="text-sm text-ink-muted">{summary}</p>
        <p className="text-xs text-ink-faint">
          {t("common.totalSize", { size: `${hoverPaths.length} ${t("common.items")}` })}
        </p>
      </div>
    </div>
  );
}
