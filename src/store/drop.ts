import { create } from "zustand";

import type { FeatureDefinition, FeatureId } from "@/features/types";

/**
 * 命中唯一功能时暂存的待添加路径。
 *
 * 之所以放进全局 store 而不是路由参数：拖拽可能发生在任意页面，而跳转目标页
 * 有可能已经挂载（此时不会产生 mount），只有 store + 订阅能覆盖两种情况。
 *
 * 存路径而不是存已构造好的 `SourceFile`：体积与文档探测统一交给目标页的
 * `useSourceFiles.add()`，避免这里重复一套异步逻辑。
 */
export interface PendingDrop {
  feature: FeatureId;
  paths: string[];
}

/** 命中多个功能时的选择器状态。 */
export interface DropChooser {
  paths: string[];
  candidates: FeatureDefinition[];
}

interface DropState {
  /** 文件正在窗口内拖动，用于显示全局遮罩 */
  dragging: boolean;
  /** 当前拖动的路径，遮罩上用来提示匹配结果 */
  hoverPaths: string[];
  pending: PendingDrop | null;
  chooser: DropChooser | null;

  begin(paths: string[]): void;
  end(): void;
  setPending(pending: PendingDrop): void;
  /** 取走属于某功能的待添加路径；不属于该功能时返回 null 且不清空 */
  takePending(feature: FeatureId): string[] | null;
  clearPending(): void;
  openChooser(chooser: DropChooser): void;
  closeChooser(): void;
}

export const useDrop = create<DropState>((set, get) => ({
  dragging: false,
  hoverPaths: [],
  pending: null,
  chooser: null,

  begin(paths) {
    set({ dragging: true, hoverPaths: paths });
  },

  end() {
    set({ dragging: false, hoverPaths: [] });
  },

  setPending(pending) {
    set({ pending });
  },

  takePending(feature) {
    const pending = get().pending;
    if (!pending || pending.feature !== feature) return null;
    set({ pending: null });
    return pending.paths;
  },

  clearPending() {
    set({ pending: null });
  },

  openChooser(chooser) {
    set({ chooser });
  },

  closeChooser() {
    set({ chooser: null });
  },
}));
