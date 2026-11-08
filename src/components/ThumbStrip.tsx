import clsx from "clsx";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { extname } from "@/lib/paths";
import type { SourceFile } from "@/features/types";

export interface ThumbStripProps {
  files: SourceFile[];
  /** key 为 `SourceFile.id`，value 为预览缩略图 URL（PDF 首页 / 图片缩略）；缺失时退化为扩展名占位 */
  urls: Record<string, string>;
}

/**
 * 横向滚动的预览缩略图条，顺序与来源列表一致。
 *
 * 合并页用它做「顺序预览」，图片转 PDF 页用它做「排版前预览」。
 * 缩略图 URL 由 `useFirstPageThumbs` 生成（后端渲染到缓存目录后经 asset 协议加载）。
 */
export function ThumbStrip({ files, urls }: ThumbStripProps): ReactElement {
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
                // 缩略图尚未就绪或渲染失败（加密未解锁、格式不支持）时，退化为扩展名占位
                <span className="text-[11px] font-medium uppercase text-ink-faint">
                  {extname(file.path) || t("common.unknown")}
                </span>
              )}
              <span className="absolute left-1 top-1 rounded-md bg-black/45 px-1 text-[10px] font-medium text-white">
                {index + 1}
              </span>
            </span>
            <span
              className={clsx(
                "truncate text-[11px]",
                isPdf ? "text-ink-muted" : "text-sky-600 dark:text-sky-400",
              )}
            >
              {file.name}
            </span>
          </li>
        );
      })}
    </ol>
  );
}
