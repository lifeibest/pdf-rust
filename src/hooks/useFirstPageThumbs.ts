import { useEffect, useState } from "react";

import { assetUrl, renderThumbnails } from "@/lib/tauri";
import { extname } from "@/lib/paths";
import { useSettings } from "@/store/settings";
import type { SourceFile } from "@/features/types";

export interface FirstPageThumbs {
  /** key 为 `SourceFile.id`，value 为首页缩略图的 asset URL */
  urls: Record<string, string>;
  loading: boolean;
}

const EMPTY: FirstPageThumbs = { urls: {}, loading: false };

/**
 * 批量渲染多份文档的首页缩略图。
 *
 * 合并功能用它做「顺序预览条」：一眼看清排在前面的会是哪几页。
 * 只请求第 0 页，后端有磁盘缓存，因此来回切换页面几乎不产生开销。
 */
export function useFirstPageThumbs(
  files: SourceFile[],
  enabled: boolean,
): FirstPageThumbs {
  const maxPx = useSettings((state) => state.settings.thumbMaxPx);
  const [state, setState] = useState<FirstPageThumbs>(EMPTY);

  // 只把「id + 路径 + 密码」作为依赖签名：文件顺序变化不应触发重新渲染
  const targets = enabled
    ? files
        .filter((file) => extname(file.path) === "pdf")
        .map((file) => ({ id: file.id, path: file.path, password: file.password ?? null }))
    : [];
  const signature = JSON.stringify(targets.map((item) => [item.id, item.path, item.password]));

  useEffect(() => {
    if (targets.length === 0) {
      setState(EMPTY);
      return;
    }

    let stale = false;
    setState({ urls: {}, loading: true });

    void (async () => {
      const urls: Record<string, string> = {};
      for (const target of targets) {
        try {
          const [first] = await renderThumbnails(target.path, target.password, [0], maxPx);
          if (first) urls[target.id] = assetUrl(first.path);
        } catch {
          // 单个文档渲染失败（加密未解锁、引擎缺失）不该影响其余项
        }
        if (stale) return;
        // 逐个回填，大文档列表不用等到全部渲染完才看到第一张
        setState({ urls: { ...urls }, loading: false });
      }
    })();

    return () => {
      stale = true;
    };
    // `signature` 已经是 `targets` 内容的完整摘要，不需要再依赖 `targets` 本身
  }, [signature, maxPx]);

  return state;
}
