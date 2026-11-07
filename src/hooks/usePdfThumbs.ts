import { useEffect, useState } from "react";

import { messageOf } from "@/lib/errors";
import { assetUrl, renderThumbnails } from "@/lib/tauri";

export interface ThumbItem {
  /** 0-based 页索引，与后端 `ThumbOut.page` 一致 */
  index: number;
  /** 1-based 页码，界面直接展示 */
  page: number;
  url: string;
  width: number;
  height: number;
  rotationDeg: number;
}

export interface ThumbOptions {
  /** 加密文档的打开密码 */
  password?: string | null;
  /** 文档总页数，用于决定截断与「仅预览前 N 页」提示 */
  total?: number;
  /** 最多渲染多少页；大文档全量渲染会明显卡顿 */
  limit?: number;
  /** 缩略图最长边像素 */
  maxPx?: number | null;
  /** 关闭时不发请求（例如尚未选中文件、密码还没填对） */
  enabled?: boolean;
}

export interface PdfThumbs {
  thumbs: ThumbItem[];
  loading: boolean;
  /** 渲染失败的原因（后端 message），null 表示没有错误 */
  error: string | null;
  /** 因 `limit` 被截断时，实际展示的页数 */
  shown: number;
  truncated: boolean;
}

const EMPTY: PdfThumbs = {
  thumbs: [],
  loading: false,
  error: null,
  shown: 0,
  truncated: false,
};

/**
 * 加载单个 PDF 的页面缩略图。
 *
 * 后端已按 (文件, 尺寸) 做磁盘缓存，重复打开几乎零成本；这里只负责
 * 「换文件 / 换密码时丢弃旧结果」与竞态防护。
 */
export function usePdfThumbs(
  path: string | null,
  options: ThumbOptions = {},
): PdfThumbs {
  const {
    password = null,
    total = 0,
    limit = 12,
    maxPx = null,
    enabled = true,
  } = options;

  const [state, setState] = useState<PdfThumbs>(EMPTY);

  useEffect(() => {
    if (!path || !enabled) {
      setState(EMPTY);
      return;
    }

    const pages = total > limit ? Array.from({ length: limit }, (_, i) => i) : null;
    const shown = total > limit ? limit : total;
    let stale = false;

    setState({ thumbs: [], loading: true, error: null, shown, truncated: total > limit });

    void renderThumbnails(path, password, pages, maxPx)
      .then((items) => {
        if (stale) return;
        setState({
          thumbs: items.map((item) => ({
            index: item.page,
            page: item.page + 1,
            url: assetUrl(item.path),
            width: item.width,
            height: item.height,
            rotationDeg: item.rotationDeg,
          })),
          loading: false,
          error: null,
          shown,
          truncated: total > limit,
        });
      })
      .catch((error: unknown) => {
        if (stale) return;
        setState({
          thumbs: [],
          loading: false,
          error: messageOf(error),
          shown,
          truncated: total > limit,
        });
      });

    return () => {
      stale = true;
    };
    // `shown` 由 total/limit 推导，不必单独入依赖
  }, [path, password, total, limit, maxPx, enabled]);

  return state;
}
