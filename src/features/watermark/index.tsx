import { RotateCcw, Trash2 } from "lucide-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { DropZone } from "@/components/DropZone";
import { FileList } from "@/components/FileList";
import { Notice } from "@/components/Notice";
import { OptionsForm } from "@/components/OptionsForm";
import { OutputPicker } from "@/components/OutputPicker";
import { Section } from "@/components/Section";
import { TaskProgress } from "@/components/TaskProgress";
import type { FeaturePageProps } from "@/features/types";
import { asNumber, asString } from "@/features/types";
import { useFeatureController } from "@/hooks/useFeatureController";
import { usePdfThumbs } from "@/hooks/usePdfThumbs";
import { useSettings } from "@/store/settings";

const COLOR_HEX: Record<string, string> = {
  gray: "#787878",
  red: "#d23030",
  blue: "#2a6cd2",
  black: "#1c1c1c",
};

const WATERMARK_PRESETS = [
  "watermark.presetUse",
  "watermark.presetInternal",
  "watermark.presetSample",
] as const;

/**
 * PDF 加水印页：左侧提供首页缩略图 + 实时 CSS 水印叠加预览，右侧提供常用水印快捷填入。
 */
export default function WatermarkPage({ feature }: FeaturePageProps): ReactElement {
  const { t } = useTranslation();
  const controller = useFeatureController(feature);
  const { source, output, runner, values, setValue, resetValues, primary, pageCount } = controller;
  const thumbMaxPx = useSettings((state) => state.settings.thumbMaxPx);

  const thumbs = usePdfThumbs(primary?.path ?? null, {
    password: primary?.password ?? null,
    total: pageCount > 0 ? 1 : 0,
    maxPx: Math.max(thumbMaxPx, 512),
  });

  const text = asString(values, "text", "").trim();
  const layout = asString(values, "layout", "tile");
  const color = COLOR_HEX[asString(values, "color", "gray")] ?? COLOR_HEX.gray;
  const fontSize = asNumber(values, "fontSize", 36);
  const opacity = Math.min(1, Math.max(0.05, asNumber(values, "opacity", 22) / 100));
  const angle = asNumber(values, "angle", -35);

  const firstThumb = thumbs.thumbs[0] ?? null;

  return (
    <div className="flex flex-col gap-5">
      <p className="text-sm text-ink-muted">{t(feature.descKey)}</p>

      {source.rejected.length > 0 ? (
        <Notice tone="warn" onDismiss={source.dismissRejected}>
          {t("drop.rejected", { names: source.rejected.join("、") })}
        </Notice>
      ) : null}

      <div className="grid items-start gap-5 xl:grid-cols-[minmax(0,1fr)_21rem]">
        <div className="flex min-w-0 flex-col gap-5">
          <Section
            title={t("feature.singleSource")}
            action={
              source.files.length > 0 ? (
                <button
                  type="button"
                  className="btn-quiet h-7 px-2 py-0 text-xs"
                  onClick={source.clear}
                >
                  <Trash2 size={13} />
                  {t("common.clear")}
                </button>
              ) : undefined
            }
          >
            <div className="flex flex-col gap-3">
              <DropZone
                feature={feature}
                busy={source.busy}
                compact={source.files.length > 0}
                onPick={() => void source.pick()}
              />
              {source.files.length > 0 ? (
                <FileList
                  files={source.files}
                  onRemove={source.remove}
                  onReorder={source.reorder}
                  onPassword={source.setPassword}
                />
              ) : null}
            </div>
          </Section>

          {firstThumb ? (
            <Section title={t("watermark.previewTitle")} hint={t("watermark.previewHint")}>
              <div className="flex justify-center">
                <div className="relative inline-block max-h-[28rem] overflow-hidden rounded-xl border border-line bg-white shadow-sm">
                  <img
                    src={firstThumb.url}
                    alt="preview"
                    draggable={false}
                    className="block max-h-[28rem] w-auto object-contain"
                  />
                  {text.length > 0 ? (
                    <div
                      aria-hidden="true"
                      className="pointer-events-none absolute inset-0 overflow-hidden select-none"
                    >
                      {layout === "center" ? (
                        <div className="flex h-full w-full items-center justify-center">
                          <span
                            className="whitespace-nowrap font-semibold"
                            style={{
                              color,
                              opacity,
                              fontSize: `${Math.max(12, Math.round(fontSize * 0.48))}px`,
                              transform: `rotate(${angle}deg)`,
                            }}
                          >
                            {text}
                          </span>
                        </div>
                      ) : (
                        <div className="grid h-[140%] w-[140%] -translate-x-[20%] -translate-y-[20%] grid-cols-3 grid-rows-4 place-items-center gap-4">
                          {Array.from({ length: 12 }).map((_, idx) => (
                            <span
                              key={idx}
                              className="whitespace-nowrap font-semibold"
                              style={{
                                color,
                                opacity,
                                fontSize: `${Math.max(10, Math.round(fontSize * 0.4))}px`,
                                transform: `rotate(${angle}deg)`,
                              }}
                            >
                              {text}
                            </span>
                          ))}
                        </div>
                      )}
                    </div>
                  ) : null}
                </div>
              </div>
            </Section>
          ) : null}
        </div>

        <aside className="flex min-w-0 flex-col gap-5">
          <Section
            title={t("feature.options")}
            action={
              <button
                type="button"
                className="btn-quiet h-7 w-7 rounded-lg px-0 py-0"
                onClick={resetValues}
                title={t("common.reset")}
                aria-label={t("common.reset")}
              >
                <RotateCcw size={13} />
              </button>
            }
          >
            <div className="flex flex-col gap-3">
              <div className="flex flex-wrap items-center gap-1.5">
                <span className="text-[11px] text-ink-faint">{t("watermark.quickFill")}</span>
                {WATERMARK_PRESETS.map((key) => {
                  const presetText = t(key);
                  return (
                    <button
                      key={key}
                      type="button"
                      disabled={runner.running}
                      onClick={() => setValue("text", presetText)}
                      className="rounded-full border border-line bg-surface-subtle px-2.5 py-0.5 text-[11px] text-ink-muted transition-colors hover:border-brand-400 hover:text-ink"
                    >
                      {presetText}
                    </button>
                  );
                })}
              </div>

              <OptionsForm
                fields={feature.options}
                values={values}
                onChange={setValue}
                pageCount={pageCount}
                disabled={runner.running}
              />
            </div>
          </Section>

          <Section title={t("feature.output")}>
            <OutputPicker api={output} disabled={runner.running} />
          </Section>

          <TaskProgress
            feature={feature}
            runner={runner}
            canRun={controller.canRun}
            hint={controller.hint ?? undefined}
            onRun={() => controller.run()}
            onRunAnother={controller.runAnother}
          />
        </aside>
      </div>
    </div>
  );
}
