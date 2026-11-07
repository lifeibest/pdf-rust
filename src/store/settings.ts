import { create } from "zustand";
import { load, type Store } from "@tauri-apps/plugin-store";

import { DEFAULT_LANGUAGE, isLanguageCode, type LanguageCode } from "@/i18n";
import type { ImageFormat } from "@/lib/tauri";

export type ThemeMode = "light" | "dark" | "system";
export type OutputMode = "remember" | "ask" | "fixed";

export interface Settings {
  language: LanguageCode;
  theme: ThemeMode;
  /** 输出目录策略 */
  outputMode: OutputMode;
  /** `outputMode === "fixed"` 时使用的目录 */
  fixedOutputDir: string | null;
  /** `outputMode === "remember"` 时记住的上次目录 */
  lastOutputDir: string | null;
  imageFormat: ImageFormat;
  imageDpi: number;
  imageQuality: number;
  /** 任务结束时发送系统通知 */
  notifyOnFinish: boolean;
  thumbMaxPx: number;
}

export const DEFAULT_SETTINGS: Settings = {
  language: DEFAULT_LANGUAGE,
  theme: "system",
  outputMode: "remember",
  fixedOutputDir: null,
  lastOutputDir: null,
  imageFormat: "png",
  imageDpi: 150,
  imageQuality: 85,
  notifyOnFinish: true,
  thumbMaxPx: 256,
};

const STORE_FILE = "settings.json";
const STORE_KEY = "settings";

interface SettingsState {
  settings: Settings;
  /** 磁盘配置是否已读回；未就绪时不要写入，否则会覆盖用户已保存的值 */
  ready: boolean;
  load(): Promise<void>;
  patch(partial: Partial<Settings>): void;
  reset(): void;
}

let storePromise: Promise<Store | null> | null = null;

/** 打开（或创建）持久化文件。非 Tauri 环境（如纯浏览器预览）下返回 null。 */
function openStore(): Promise<Store | null> {
  storePromise ??= load(STORE_FILE, { autoSave: 300 }).catch(() => null);
  return storePromise;
}

/** 磁盘上的值可能来自旧版本，逐字段校验后再与默认值合并。 */
function sanitize(raw: unknown): Settings {
  if (typeof raw !== "object" || raw === null) return { ...DEFAULT_SETTINGS };
  const value = raw as Record<string, unknown>;
  const base = { ...DEFAULT_SETTINGS };

  return {
    language: isLanguageCode(value.language) ? value.language : base.language,
    theme: isThemeMode(value.theme) ? value.theme : base.theme,
    outputMode: isOutputMode(value.outputMode) ? value.outputMode : base.outputMode,
    fixedOutputDir: asDir(value.fixedOutputDir),
    lastOutputDir: asDir(value.lastOutputDir),
    imageFormat: isImageFormat(value.imageFormat) ? value.imageFormat : base.imageFormat,
    imageDpi: asClampedNumber(value.imageDpi, 24, 1200, base.imageDpi),
    imageQuality: asClampedNumber(value.imageQuality, 1, 100, base.imageQuality),
    notifyOnFinish:
      typeof value.notifyOnFinish === "boolean" ? value.notifyOnFinish : base.notifyOnFinish,
    thumbMaxPx: asClampedNumber(value.thumbMaxPx, 64, 2048, base.thumbMaxPx),
  };
}

function isThemeMode(value: unknown): value is ThemeMode {
  return value === "light" || value === "dark" || value === "system";
}

function isOutputMode(value: unknown): value is OutputMode {
  return value === "remember" || value === "ask" || value === "fixed";
}

function isImageFormat(value: unknown): value is ImageFormat {
  return value === "png" || value === "jpeg" || value === "webp";
}

function asDir(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

function asClampedNumber(
  value: unknown,
  min: number,
  max: number,
  fallback: number,
): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return fallback;
  return Math.min(max, Math.max(min, Math.round(value)));
}

export const useSettings = create<SettingsState>((set, get) => ({
  settings: { ...DEFAULT_SETTINGS },
  ready: false,

  async load() {
    const store = await openStore();
    if (!store) {
      // 持久化不可用时仍以默认值运行，界面不应因此卡住
      set({ ready: true });
      return;
    }

    const raw = await store.get<unknown>(STORE_KEY).catch(() => null);
    set({ settings: sanitize(raw), ready: true });
  },

  patch(partial) {
    const next = { ...get().settings, ...partial };
    set({ settings: next });
    // ready 之前不写盘：此时内存里还是默认值，写下去会覆盖用户的旧配置
    if (get().ready) {
      void openStore().then((store) => store?.set(STORE_KEY, next));
    }
  },

  reset() {
    set({ settings: { ...DEFAULT_SETTINGS } });
    if (get().ready) {
      void openStore().then((store) => store?.set(STORE_KEY, DEFAULT_SETTINGS));
    }
  },
}));

/**
 * 按策略决定输出目录。
 *
 * 返回 `null` 表示「本次需要询问用户」（`ask` 模式，或 `remember`/`fixed` 模式下
 * 还没有可用目录）；此时交给后端 `default_output_dir` 兜底或弹出目录选择框。
 */
export function resolveOutputDir(settings: Settings): string | null {
  switch (settings.outputMode) {
    case "fixed":
      return settings.fixedOutputDir;
    case "remember":
      return settings.lastOutputDir;
    case "ask":
      return null;
  }
}
