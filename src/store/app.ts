import { create } from "zustand";

import { messageOf } from "@/lib/errors";
import { appInfo, reloadPdfium, type AppInfo } from "@/lib/tauri";

interface AppState {
  info: AppInfo | null;
  loading: boolean;
  /** 读取应用信息失败的原因（非 Tauri 环境下会有值） */
  error: string | null;
  /** PDFium 是否就绪，首页据此决定是否显示告警 */
  engineReady: boolean;
  load(force?: boolean): Promise<void>;
  /** 「重新检测」：只刷新引擎状态，比重读整份信息快 */
  recheckEngine(): Promise<void>;
}

export const useApp = create<AppState>((set, get) => ({
  info: null,
  loading: false,
  error: null,
  engineReady: false,

  async load(force = false) {
    if (get().info && !force) return;
    set({ loading: true });
    try {
      const info = await appInfo();
      set({ info, engineReady: info.pdfium.ready, error: null });
    } catch (error) {
      set({ error: messageOf(error) });
    } finally {
      set({ loading: false });
    }
  },

  async recheckEngine() {
    set({ loading: true });
    try {
      const status = await reloadPdfium();
      const info = get().info;
      set({
        engineReady: status.ready,
        info: info ? { ...info, pdfium: status } : info,
        error: null,
      });
    } catch (error) {
      set({ error: messageOf(error) });
    } finally {
      set({ loading: false });
    }
  },
}));
