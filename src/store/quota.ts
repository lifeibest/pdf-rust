import { create } from "zustand";

import { messageOf } from "@/lib/errors";
import { scheduleVipReport } from "@/lib/vipReport";
import {
  activateLicense,
  deactivateLicense,
  isAppError,
  licenseStatus,
  quotaStatus,
  type LicenseStatus,
  type QuotaStatus,
} from "@/lib/tauri";

interface QuotaState {
  license: LicenseStatus | null;
  quota: QuotaStatus | null;
  /** 全局激活引导弹窗是否打开（配额用尽时自动打开） */
  modalOpen: boolean;
  loading: boolean;
  /** 激活失败等原因 */
  error: string | null;
  /** 拉取授权与配额快照 */
  refresh(): Promise<void>;
  openModal(): void;
  closeModal(): void;
  /** 校验并保存激活码；成功返回 true 并关闭弹窗 */
  activate(code: string): Promise<boolean>;
  /** 移除本地激活码，恢复免费额度 */
  deactivate(): Promise<void>;
}

/**
 * 授权与配额的全局状态。
 *
 * 激活码校验在 Rust 侧离线完成，本 store 只负责快照缓存与「配额用尽 → 弹激活引导」
 * 的全局联动，因此任何功能页触发超额都能统一弹出同一个引导弹窗。
 */
export const useQuota = create<QuotaState>((set, get) => ({
  license: null,
  quota: null,
  modalOpen: false,
  loading: false,
  error: null,

  async refresh() {
    set({ loading: true });
    try {
      const [license, quota] = await Promise.all([licenseStatus(), quotaStatus()]);
      set({ license, quota, error: null });
    } catch (error) {
      set({ error: messageOf(error) });
    } finally {
      set({ loading: false });
    }
  },

  openModal() {
    set({ modalOpen: true });
    void get().refresh();
  },

  closeModal() {
    set({ modalOpen: false, error: null });
  },

  async activate(code) {
    const trimmed = code.trim();
    if (trimmed.length === 0) {
      set({ error: "empty" });
      return false;
    }
    set({ loading: true, error: null });
    try {
      const license = await activateLicense(trimmed);
      const quota = await quotaStatus();
      set({ license, quota, modalOpen: false, error: null });
      // 激活成功后静默上报（best-effort，失败不影响激活结果）。
      void scheduleVipReport();
      return true;
    } catch (error) {
      // 激活码类错误映射为 i18n 哨兵值，其余保留后端原文
      const mapped =
        isAppError(error) && error.kind === "licenseInvalid" ? "licenseInvalid" : messageOf(error);
      set({ error: mapped });
      return false;
    } finally {
      set({ loading: false });
    }
  },

  async deactivate() {
    set({ loading: true });
    try {
      const license = await deactivateLicense();
      const quota = await quotaStatus();
      set({ license, quota, error: null });
    } catch (error) {
      set({ error: messageOf(error) });
    } finally {
      set({ loading: false });
    }
  },
}));
