import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { pickDir } from "@/lib/tauri";
import { resolveOutputDir, useSettings } from "@/store/settings";

export interface OutputPickerApi {
  /** 当前选定的输出目录；null 表示「运行时再决定」 */
  dir: string | null;
  /** 输出文件名（不含扩展名），空字符串表示交给后端自动命名 */
  name: string;
  setDir(dir: string | null): void;
  setName(name: string): void;
  /** 弹出系统目录选择框 */
  chooseDir(): Promise<void>;
  /**
   * 运行前解析最终目录。
   *
   * `ask` 策略（或还没有可用目录）时现场弹框；用户取消则返回 null，
   * 调用方应据此中止运行而不是回退到默认目录。
   */
  resolve(): Promise<string | null>;
  /** 任务成功后调用，把目录写回「记住上次使用」 */
  commit(dir: string | null): void;
}

export interface OutputPickerOptions {
  /** 单来源功能不需要输出目录时置 false，`resolve()` 直接返回 null */
  enabled?: boolean;
}

/**
 * 输出目录与文件名的状态管理。
 *
 * 三种策略（记住上次 / 每次询问 / 固定目录）都收敛到这里，功能页只需
 * 在运行前调用 `resolve()`、成功后调用 `commit()`。
 */
export function useOutputPicker(options: OutputPickerOptions = {}): OutputPickerApi {
  const { enabled = true } = options;
  const { t } = useTranslation();

  const ready = useSettings((state) => state.ready);
  const settings = useSettings((state) => state.settings);
  const patch = useSettings((state) => state.patch);

  const [dir, setDir] = useState<string | null>(null);
  const [name, setName] = useState("");
  /** 持久化配置是异步读回的，只在就绪那一刻应用一次策略 */
  const initialized = useRef(false);

  useEffect(() => {
    if (!ready || initialized.current) return;
    initialized.current = true;
    setDir(resolveOutputDir(settings));
  }, [ready, settings]);

  const chooseDir = useCallback(async () => {
    const result = await pickDir(t("common.outputDir")).catch(() => null);
    if (!result || result.cancelled) return;
    const [picked] = result.paths;
    if (picked) setDir(picked);
  }, [t]);

  const resolve = useCallback(async () => {
    if (!enabled) return null;
    if (dir) return dir;

    const result = await pickDir(t("common.outputDir")).catch(() => null);
    if (!result || result.cancelled) return null;
    const [picked] = result.paths;
    if (!picked) return null;
    setDir(picked);
    return picked;
  }, [dir, enabled, t]);

  const commit = useCallback(
    (used: string | null) => {
      if (!used) return;
      if (settings.outputMode === "remember" && used !== settings.lastOutputDir) {
        patch({ lastOutputDir: used });
      }
    },
    [patch, settings.lastOutputDir, settings.outputMode],
  );

  return { dir, name, setDir, setName, chooseDir, resolve, commit };
}
