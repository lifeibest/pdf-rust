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
import { ThumbStrip } from "@/components/ThumbStrip";
import type { FeaturePageProps } from "@/features/types";
import { useFeatureController } from "@/hooks/useFeatureController";
import { useFirstPageThumbs } from "@/hooks/useFirstPageThumbs";

const IDCARD_WATERMARK_PRESETS = [
  "idcard.presetOnboarding",
  "idcard.presetBank",
  "idcard.presetVerify",
] as const;

/**
 * 身份证 / 证件 A4 1:1 拼版页：提供照片预览与常用防盗水印一键填入。
 */
export default function IdCardPage({ feature }: FeaturePageProps): ReactElement {
  const { t } = useTranslation();
  const controller = useFeatureController(feature);
  const { source, output, runner, values, setValue, resetValues } = controller;

  const thumbs = useFirstPageThumbs(source.files, source.files.length > 0);

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
            title={t("feature.source")}
            hint={t("idcard.sourceHint")}
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
                  sortable
                  onRemove={source.remove}
                  onReorder={source.reorder}
                />
              ) : null}
            </div>
          </Section>

          {source.files.length > 0 ? (
            <Section title={t("img2pdf.preview")} hint={t("idcard.previewHint")}>
              <ThumbStrip files={source.files} urls={thumbs.urls} />
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
              <OptionsForm
                fields={feature.options}
                values={values}
                onChange={setValue}
                disabled={runner.running}
              />

              <div className="flex flex-wrap items-center gap-1.5">
                <span className="text-[11px] text-ink-faint">{t("watermark.quickFill")}</span>
                {IDCARD_WATERMARK_PRESETS.map((key) => {
                  const presetText = t(key);
                  return (
                    <button
                      key={key}
                      type="button"
                      disabled={runner.running}
                      onClick={() => setValue("watermarkText", presetText)}
                      className="rounded-full border border-line bg-surface-subtle px-2.5 py-0.5 text-[11px] text-ink-muted transition-colors hover:border-brand-400 hover:text-ink"
                    >
                      {presetText}
                    </button>
                  );
                })}
              </div>
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
