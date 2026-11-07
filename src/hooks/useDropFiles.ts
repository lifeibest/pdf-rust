import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { UnlistenFn } from "@tauri-apps/api/event";

import { extname } from "@/lib/paths";
import { ACCEPTED_EXTENSIONS, matchFeatures } from "@/features/registry";
import { useDrop } from "@/store/drop";
import type { FeatureId } from "@/features/types";

/**
 * 全局窗口拖拽。
 *
 * Tauri 的 `dragDropEnabled` 会拦下 WebView 的 HTML5 拖放事件，因此这里统一
 * 走原生 `onDragDropEvent`：在任意页面拖入文件都能按扩展名匹配功能，
 * 命中唯一功能则直接跳转，多个命中时弹出选择器。
 *
 * 必须在 Router 内部调用（用到 `useNavigate`）。
 */
export function useDropFiles(): void {
  const navigate = useNavigate();

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let disposed = false;

    const subscribe = getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload = event.payload;

        // `over` 事件不带路径（只有坐标），而 `enter` 已经给出了本次拖拽的全部路径
        if (payload.type === "enter") {
          useDrop.getState().begin(payload.paths);
          return;
        }

        if (payload.type === "over") return;

        if (payload.type === "leave") {
          useDrop.getState().end();
          return;
        }

        useDrop.getState().end();
        handleDrop(payload.paths, navigate);
      })
      .catch(() => null);

    void subscribe.then((fn) => {
      if (!fn) return;
      // 组件在订阅完成前就卸载了（StrictMode 下会发生），立刻撤销
      if (disposed) fn();
      else unlisten = fn;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [navigate]);
}

function handleDrop(paths: string[], navigate: (to: string) => void): void {
  const kept = paths.filter((path) => ACCEPTED_EXTENSIONS.includes(extname(path)));
  if (kept.length === 0) return;

  const candidates = matchFeatures(kept.map(extname));
  if (candidates.length === 0) return;

  const [best] = candidates;
  if (candidates.length === 1) {
    useDrop.getState().setPending({ feature: best.id, paths: kept });
    navigate(best.route);
    return;
  }

  useDrop.getState().openChooser({ paths: kept, candidates });
}

/**
 * 消费全局拖拽预填的路径。
 *
 * 订阅 store 而不是在 mount 时取一次：拖拽的目标页很可能已经打开着，
 * 这时不会有 mount，只有订阅能收到。
 */
export function usePendingDrop(
  feature: FeatureId,
  add: (paths: string[]) => Promise<void>,
): void {
  const pending = useDrop((state) => state.pending);

  useEffect(() => {
    if (!pending || pending.feature !== feature) return;
    const paths = useDrop.getState().takePending(feature);
    if (paths && paths.length > 0) void add(paths);
  }, [pending, feature, add]);
}
