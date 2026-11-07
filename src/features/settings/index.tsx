import clsx from "clsx";
import { CheckCircle2, FolderOpen, HardDrive, Monitor, Moon, RotateCcw, Sun, Trash2 } from "lucide-react";
import { useEffect, useRef, useState, type ReactElement, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { isPermissionGranted, requestPermission } from "@tauri-apps/plugin-notification";

import { Notice } from "@/components/Notice";
import { Switch } from "@/components/OptionsForm";
import { Section } from "@/components/Section";
import { LANGUAGES } from "@/i18n";
import { messageOf } from "@/lib/errors";
import { formatBytes } from "@/lib/format";
import { shortenPath } from "@/lib/paths";
import { clearTemp, clearThumbCache, pickDir, type ImageFormat } from "@/lib/tauri";
import { useSettings, type OutputMode, type ThemeMode } from "@/store/settings";

const THEME_CHOICES: { value: ThemeMode; labelKey: string; icon: typeof Sun }[] = [
  { value: "light", labelKey: "settings.themeLight", icon: Sun },
  { value: "dark", labelKey: "settings.themeDark", icon: Moon },
  { value: "system", labelKey: "settings.themeSystem", icon: Monitor },
];

const OUTPUT_MODES: { value: OutputMode; labelKey: string }[] = [
  { value: "remember", labelKey: "settings.outputModeRemember" },
  { value: "ask", labelKey: "settings.outputModeAsk" },
  { value: "fixed", labelKey: "settings.outputModeFixed" },
];

const IMAGE_FORMATS: { value: ImageFormat; labelKey: string }[] = [
  { value: "png", labelKey: "options.formatValue.png" },
  { value: "jpeg", labelKey: "options.formatValue.jpeg" },
  { value: "webp", labelKey: "options.formatValue.webp" },
];

/** 清理结果的有效期：够读一遍，又不会一直占着版面。 */
const FLASH_MS = 2000;

/**
 * 设置页。
 *
 * 所有改动直接写进 `store/settings.ts`（内部带 300ms 防抖落盘），因此这里没有
 * 「保存」按钮；只在改动后短暂闪一条成功提示，让用户知道已经生效。
 */
export default function SettingsPage(): ReactElement {
  const { t } = useTranslation();
  const settings = useSettings((state) => state.settings);
  const patch = useSettings((state) => state.patch);
  const reset = useSettings((state) => state.reset);

  const [flashed, setFlashed] = useState(false);
  const [cleaning, setCleaning] = useState(false);
  /** 是否已经执行过一次清理，用来区分「没清过」与「清过了但没东西可清」 */
  const [cleaned, setCleaned] = useState(false);
  const [cleanLines, setCleanLines] = useState<string[]>([]);
  const [cleanError, setCleanError] = useState<string | null>(null);
  const [confirmReset, setConfirmReset] = useState(false);
  const flashTimer = useRef<number | null>(null);

  useEffect(() => {
    const timer = flashTimer.current;
    return () => {
      if (timer !== null) window.clearTimeout(timer);
    };
  }, []);

  /** 包一层：每次改动都闪一下「已保存」。 */
  function update(partial: Parameters<typeof patch>[0]): void {
    patch(partial);
    flash();
  }

  function flash(): void {
    setFlashed(true);
    if (flashTimer.current !== null) window.clearTimeout(flashTimer.current);
    flashTimer.current = window.setTimeout(() => setFlashed(false), FLASH_MS);
  }

  async function handleNotify(next: boolean): Promise<void> {
    update({ notifyOnFinish: next });
    if (!next) return;
    // 通知由后端发送，但权限只能在前端申请
    const granted = await isPermissionGranted().catch(() => false);
    if (!granted) await requestPermission().catch(() => null);
  }

  async function handleChooseFixedDir(): Promise<void> {
    const picked = await pickDir(t("settings.fixedDir")).catch(() => null);
    const dir = picked && !picked.cancelled ? (picked.paths[0] ?? null) : null;
    if (dir) update({ fixedOutputDir: dir });
  }

  async function handleClean(): Promise<void> {
    setCleaning(true);
    setCleanLines([]);
    setCleanError(null);
    try {
      const [thumbBytes, report] = await Promise.all([clearThumbCache(), clearTemp()]);
      const lines: string[] = [];
      if (thumbBytes > 0) {
        lines.push(t("settings.cacheCleaned", { size: formatBytes(thumbBytes) }));
      }
      if (report.tempDirs > 0) {
        lines.push(
          t("settings.tempCleaned", {
            dirs: report.tempDirs,
            size: formatBytes(report.tempBytes),
          }),
        );
      }
      setCleanLines(lines);
      setCleaned(true);
    } catch (error) {
      setCleanError(messageOf(error));
    } finally {
      setCleaning(false);
    }
  }

  return (
    <div className="flex flex-col gap-5">
      <p className="text-sm text-ink-muted">{t("features.settings.desc")}</p>

      {flashed ? (
        <Notice tone="success">
          <span className="inline-flex items-center gap-1.5">
            <CheckCircle2 size={13} />
            {t("settings.saved")}
          </span>
        </Notice>
      ) : null}

      <div className="grid items-start gap-5 lg:grid-cols-2">
        <div className="flex min-w-0 flex-col gap-5">
          <Section title={t("settings.appearance")}>
            <div className="flex flex-col gap-5">
              <Row label={t("settings.language")} hint={t("settings.languageHint")}>
                <select
                  className="field w-44"
                  value={settings.language}
                  onChange={(event) =>
                    update({ language: event.target.value as typeof settings.language })
                  }
                >
                  {LANGUAGES.map((language) => (
                    <option key={language.code} value={language.code}>
                      {language.label}
                    </option>
                  ))}
                </select>
              </Row>

              <Row label={t("settings.theme")}>
                <div className="grid w-full grid-cols-3 gap-1 rounded-xl bg-surface-subtle p-1">
                  {THEME_CHOICES.map((choice) => {
                    const Icon = choice.icon;
                    const active = settings.theme === choice.value;
                    return (
                      <button
                        key={choice.value}
                        type="button"
                        onClick={() => update({ theme: choice.value })}
                        aria-pressed={active}
                        className={clsx(
                          "flex items-center justify-center gap-1.5 rounded-lg px-2 py-1.5",
                          "text-xs font-medium transition-colors",
                          active
                            ? "bg-surface text-brand-600 shadow-card"
                            : "text-ink-muted hover:text-ink",
                        )}
                      >
                        <Icon size={14} />
                        {t(choice.labelKey)}
                      </button>
                    );
                  })}
                </div>
              </Row>
            </div>
          </Section>

          <Section title={t("settings.output")} hint={t("settings.outputModeHint")}>
            <div className="flex flex-col gap-5">
              <Row label={t("settings.outputMode")}>
                <select
                  className="field w-full"
                  value={settings.outputMode}
                  onChange={(event) => update({ outputMode: event.target.value as OutputMode })}
                >
                  {OUTPUT_MODES.map((mode) => (
                    <option key={mode.value} value={mode.value}>
                      {t(mode.labelKey)}
                    </option>
                  ))}
                </select>
              </Row>

              {settings.outputMode === "fixed" ? (
                <Row label={t("settings.fixedDir")}>
                  <div className="flex items-center gap-2">
                    <span
                      className="field flex min-w-0 flex-1 items-center gap-2 truncate text-xs text-ink-muted"
                      title={settings.fixedOutputDir ?? undefined}
                    >
                      {settings.fixedOutputDir ? (
                        <>
                          <FolderOpen size={14} className="shrink-0 text-ink-faint" />
                          <span className="truncate">
                            {shortenPath(settings.fixedOutputDir, 28)}
                          </span>
                        </>
                      ) : (
                        <span className="text-ink-faint">{t("common.defaultDir")}</span>
                      )}
                    </span>
                    <button
                      type="button"
                      className="btn-ghost shrink-0"
                      onClick={() => void handleChooseFixedDir()}
                    >
                      {t("common.choose")}
                    </button>
                  </div>
                </Row>
              ) : null}
            </div>
          </Section>

          <Section title={t("settings.imageDefaults")}>
            <div className="flex flex-col gap-5">
              <Row label={t("settings.defaultFormat")}>
                <select
                  className="field w-full"
                  value={settings.imageFormat}
                  onChange={(event) => update({ imageFormat: event.target.value as ImageFormat })}
                >
                  {IMAGE_FORMATS.map((format) => (
                    <option key={format.value} value={format.value}>
                      {t(format.labelKey)}
                    </option>
                  ))}
                </select>
              </Row>

              <Row label={t("settings.defaultDpi")}>
                <NumberInput
                  value={settings.imageDpi}
                  min={24}
                  max={1200}
                  step={6}
                  onChange={(value) => update({ imageDpi: value })}
                />
              </Row>

              <Row label={t("settings.defaultQuality")}>
                <NumberInput
                  value={settings.imageQuality}
                  min={1}
                  max={100}
                  onChange={(value) => update({ imageQuality: value })}
                />
              </Row>
            </div>
          </Section>
        </div>

        <div className="flex min-w-0 flex-col gap-5">
          <Section title={t("settings.behavior")}>
            <div className="flex flex-col gap-5">
              <Row label={t("settings.notify")} hint={t("settings.notifyHint")}>
                <Switch
                  checked={settings.notifyOnFinish}
                  onChange={(next) => void handleNotify(next)}
                  label={settings.notifyOnFinish ? t("common.on") : t("common.off")}
                />
              </Row>

              <Row label={t("settings.thumbMaxPx")} hint={t("settings.thumbHint")}>
                <NumberInput
                  value={settings.thumbMaxPx}
                  min={64}
                  max={2048}
                  step={32}
                  onChange={(value) => update({ thumbMaxPx: value })}
                />
              </Row>
            </div>
          </Section>

          <Section title={t("settings.storage")}>
            <div className="flex flex-col gap-3">
              <button
                type="button"
                className="btn-ghost self-start"
                disabled={cleaning}
                onClick={() => void handleClean()}
              >
                {cleaning ? <Trash2 size={15} className="animate-pulse" /> : <HardDrive size={15} />}
                {t(cleaning ? "settings.cleaning" : "settings.cleanCache")}
              </button>

              {cleanError ? <Notice tone="error">{cleanError}</Notice> : null}
              {cleanLines.map((line) => (
                <Notice key={line} tone="success">
                  {line}
                </Notice>
              ))}
              {cleaned && cleanLines.length === 0 && !cleanError ? (
                <Notice tone="info">{t("settings.nothingToClean")}</Notice>
              ) : null}
            </div>
          </Section>

          <Section title={t("settings.resetAll")} hint={t("settings.resetHint")}>
            {confirmReset ? (
              <div className="flex flex-col gap-3">
                <p className="text-xs text-ink-muted">{t("settings.resetConfirm")}</p>
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    className="btn-primary"
                    onClick={() => {
                      reset();
                      setConfirmReset(false);
                      flash();
                    }}
                  >
                    {t("common.reset")}
                  </button>
                  <button
                    type="button"
                    className="btn-ghost"
                    onClick={() => setConfirmReset(false)}
                  >
                    {t("common.cancel")}
                  </button>
                </div>
              </div>
            ) : (
              <button
                type="button"
                className="btn-ghost self-start"
                onClick={() => setConfirmReset(true)}
              >
                <RotateCcw size={15} />
                {t("settings.resetAll")}
              </button>
            )}
          </Section>
        </div>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------- 版面零件 */

interface RowProps {
  label: string;
  hint?: string;
  children: ReactNode;
}

/** 「标签在上、控件在下」的设置行；`hint` 用来说明这一项影响什么。 */
function Row({ label, hint, children }: RowProps): ReactElement {
  return (
    <div className="flex flex-col gap-1.5">
      <span className="label mb-0">{label}</span>
      {children}
      {hint ? <p className="text-[11px] leading-relaxed text-ink-faint">{hint}</p> : null}
    </div>
  );
}

interface NumberInputProps {
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange(value: number): void;
}

/** 数字输入：清空时不回写，越界时夹到 `[min, max]`。 */
function NumberInput({ value, min, max, step = 1, onChange }: NumberInputProps): ReactElement {
  return (
    <input
      type="number"
      className="field w-32 text-xs tabular-nums"
      value={value}
      min={min}
      max={max}
      step={step}
      onChange={(event) => {
        const raw = Number.parseFloat(event.target.value);
        if (Number.isFinite(raw)) onChange(Math.min(max, Math.max(min, Math.round(raw))));
      }}
    />
  );
}
