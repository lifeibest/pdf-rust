import clsx from "clsx";
import { AlertTriangle, Loader2 } from "lucide-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import type { ThumbItem } from "@/hooks/usePdfThumbs";

export interface PageThumbGridProps {
  thumbs: ThumbItem[];
  loading?: boolean;
  error?: string | null;
  /** 文档总页数，用于「仅预览前 N 页」提示 */
  total?: number;
  truncated?: boolean;
  /**
   * 预览旋转角（度）。
   *
   * 缩略图本身已包含页面自带的 `/Rotate`，这里叠加的是「本次操作将产生的旋转」，
   * 因此旋转功能可以所见即所得地预览结果。
   */
  previewRotation?: number;
  className?: string;
}

/** 页面缩略图网格。 */
export function PageThumbGrid({
  thumbs,
  loading = false,
  error = null,
  total = 0,
  truncated = false,
  previewRotation = 0,
  className,
}: PageThumbGridProps): ReactElement | null {
  const { t } = useTranslation();

  if (loading && thumbs.length === 0) {
    return (
      <div className={clsx("flex items-center justify-center gap-2 py-8 text-xs text-ink-faint", className)}>
        <Loader2 size={14} className="animate-spin" />
        {t("thumb.rendering")}
      </div>
    );
  }

  if (error) {
    return (
      <div className={clsx("flex items-center justify-center gap-2 py-8 text-xs text-ink-faint", className)}>
        <AlertTriangle size={14} />
        {t("thumb.failed")}
      </div>
    );
  }

  if (thumbs.length === 0) return null;

  return (
    <div className={clsx("flex flex-col gap-2", className)}>
      <div className="grid grid-cols-4 gap-2 sm:grid-cols-5 lg:grid-cols-6">
        {thumbs.map((thumb) => (
          <figure
            key={thumb.index}
            className="relative overflow-hidden rounded-xl border border-line bg-surface-subtle"
            title={t("thumb.pageOf", { page: thumb.page, total })}
          >
            <div className="flex aspect-[3/4] items-center justify-center p-1">
              <img
                src={thumb.url}
                alt={t("thumb.pageOf", { page: thumb.page, total })}
                loading="lazy"
                draggable={false}
                className="max-h-full max-w-full object-contain shadow-sm transition-transform duration-200"
                style={
                  previewRotation !== 0
                    ? { transform: `rotate(${previewRotation}deg) scale(0.88)` }
                    : undefined
                }
              />
            </div>
            <span className="absolute left-1 top-1 rounded-md bg-black/45 px-1 text-[10px] font-medium text-white">
              {thumb.page}
            </span>
          </figure>
        ))}
      </div>

      {truncated ? (
        <p className="text-center text-[11px] text-ink-faint">
          {t("thumb.tooMany", { total, shown: thumbs.length })}
        </p>
      ) : null}
    </div>
  );
}
