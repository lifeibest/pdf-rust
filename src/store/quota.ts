import { create } from "zustand";

import { messageOf } from "@/lib/errors";
import { isFullToken, redeemShortCode } from "@/lib/licenseRedeem";
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
  /** 超过授权台数时的设备计数（error==="overLimit" 时有效），供提示文案插值 */
  overLimit: { devices: number; cap: number } | null;
  /** 拉取授权与配额快照 */
  refresh(): Promise<void>;
  openModal(): void;
  closeModal(): void;
  /** 校验并保存激活码；成功返回 true 并关闭弹窗 */
  activate(code: string): Promise<boolean>;
  /** 移除本地激活码，恢复免费额度 */
  deactivate(): Promise<void>;
}

/** 把离线验签等错误统一成 i18n 哨兵值，其余保留后端原文。 */
function mapActivateError(error: unknown): string {
  if (isAppError(error) && error.kind === "licenseInvalid") return "licenseInvalid";
  return messageOf(error);
}

/**
 * 授权与配额的全局状态。
 *
 * 激活支持两种输入：8 位短码（先一次性联网兑换回签名 token）与完整签名 token（旧长码，直接离线验签）；
 * 验签始终在 Rust 侧离线完成。本 store 只负责快照缓存与「配额用尽 → 弹激活引导」
 * 的全局联动，因此任何功能页触发超额都能统一弹出同一个引导弹窗。
 */
export const useQuota = create<QuotaState>((set, get) => ({
  license: null,
  quota: null,
  modalOpen: false,
  loading: false,
  error: null,
  overLimit: null,

  async refresh() {
    set({ loading: true });
    try {
      const [license, quota] = await Promise.all([licenseStatus(), quotaStatus()]);
      set({ license, quota, error: null, overLimit: null });
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
    set({ modalOpen: false, error: null, overLimit: null });
  },

  async activate(code) {
    const trimmed = code.trim();
    if (trimmed.length === 0) {
      set({ error: "empty", overLimit: null });
      return false;
    }
    set({ loading: true, error: null, overLimit: null });
    try {
      let token = trimmed;
      // 短码：先联网兑换（后端登记本机设备并做授权台数硬校验，超限则不返回 token）；
      // 完整签名 token（旧长码）走纯离线验签。
      if (!isFullToken(trimmed)) {
        const outcome = await redeemShortCode(trimmed);
        if (!outcome.ok) {
          if (outcome.reason === "overLimit") {
            set({
              error: "overLimit",
              overLimit: { devices: outcome.devices ?? 0, cap: outcome.cap ?? 0 },
            });
          } else {
            set({ error: outcome.reason === "notFound" ? "redeemNotFound" : "redeemNetwork" });
          }
          return false;
        }
        token = outcome.token;
      }
      const license = await activateLicense(token);
      const quota = await quotaStatus();
      set({ license, quota, modalOpen: false, error: null, overLimit: null });
      return true;
    } catch (error) {
      set({ error: mapActivateError(error) });
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
      set({ license, quota, error: null, overLimit: null });
    } catch (error) {
      set({ error: messageOf(error) });
    } finally {
      set({ loading: false });
    }
  },
}));
