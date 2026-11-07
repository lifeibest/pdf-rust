import clsx from "clsx";
import { RotateCcw, RotateCw, Trash2 } from "lucide-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { DropZone } from "@/components/DropZone";
import { FileList } from "@/components/FileList";
import { Notice } from "@/components/Notice";
import { OptionsForm } from "@/components/OptionsForm";
import { OutputPicker } from "@/components/OutputPicker";
import { PageThumbGrid } from "@/components/PageThumbGrid";
import { Section } from "@/components/Section";
import { TaskProgress } from "@/components/TaskProgress";
import type { FeaturePageProps } from "@/features/types";
import { asBoolean, asString } from "@/features/types";
import { useFeatureController } from "@/hooks/useFeatureController";
import { usePdfThumbs } from "@/hooks/usePdfThumbs";
import { useSettings } from "@/store/settings";
import { ROTATION_CHOICES } from "./meta";

/**
 * PDF 旋转页。
 *
 * 两件事值得单独做一页：
 * 1. 角度用图形化的三选块而不是下拉框，配合缩略图的实时旋转做到所见即所得；
 * 2. 「原地修改」会覆盖源文件，此时输出目录整块隐藏，并在运行前给出警示。
 */
export default function RotatePage({ feature }: FeaturePageProps): ReactElement {
  const { t } = useTranslation();
  const controller = useFeatureController(feature);
  const { source, output, runner, values, setValue, resetValues, primary, pageCount } = controller;
  const thumbMaxPx = useSettings((state) => state.settings.thumbMaxPx);

  const degrees = Number.parseInt(asString(values, "degrees", "90"), 10) || 90;
  const inPlace = asBoolean(values, "inPlace", false);

  const thumbs = usePdfThumbs(primary?.path ?? null, {
    password: primary?.password ?? null,
    total: pageCount,
    maxPx: thumbMaxPx,
  });

  // 角度已经换成图形化选择块，剩下的字段（作用范围、原地修改）仍走通用表单
  const restFields = feature.options.filter((field) => field.key !== "degrees");

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

          {thumbs.thumbs.length > 0 || thumbs.loading || thumbs.error ? (
            <Section title={t("rotate.preview")} hint={t("rotate.previewHint")}>
              <PageThumbGrid
                thumbs={thumbs.thumbs}
                loading={thumbs.loading}
                error={thumbs.error}
                total={pageCount}
                truncated={thumbs.truncated}
                previewRotation={degrees}
              />
              <p className="mt-2 text-center text-[11px] text-ink-faint">{t("rotate.scopeNote")}</p>
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
            <div className="flex flex-col gap-4">
              <AnglePicker
                degrees={degrees}
                disabled={runner.running}
                onSelect={(next) => setValue("degrees", String(next))}
              />

              <OptionsForm
                fields={restFields}
                values={values}
                onChange={setValue}
                pageCount={pageCount}
                disabled={runner.running}
              />

              {inPlace ? <Notice tone="warn">{t("rotate.inPlaceWarning")}</Notice> : null}
            </div>
          </Section>

          {/* 原地模式直接覆盖源文件，输出目录与文件名都没有意义 */}
          {inPlace ? null : (
            <Section title={t("feature.output")}>
              <OutputPicker api={output} disabled={runner.running} />
            </Section>
          )}

          <TaskProgress
            feature={feature}
            runner={runner}
            canRun={controller.canRun}
            hint={controller.hint ?? undefined}
            onRun={() => controller.run({ needsDir: !inPlace })}
            onRunAnother={controller.runAnother}
          />
        </aside>
      </div>
    </div>
  );
}

export interface AnglePickerProps {
  degrees: number;
  onSelect(degrees: number): void;
  disabled?: boolean;
}

/** 三块图形化角度按钮，图标本身按对应角度旋转，直观表达结果。 */
function AnglePicker({ degrees, onSelect, disabled = false }: AnglePickerProps): ReactElement {
  const { t } = useTranslation();

  return (
    <fieldset disabled={disabled} className="flex flex-col gap-1.5">
      <legend className="label mb-0">{t("rotate.angle")}</legend>
      <div className="grid grid-cols-3 gap-2">
        {ROTATION_CHOICES.map((choice) => {
          const active = choice === degrees;
          return (
            <button
              key={choice}
              type="button"
              onClick={() => onSelect(choice)}
              aria-pressed={active}
              className={clsx(
                "flex flex-col items-center gap-1.5 rounded-xl border px-2 py-3",
                "transition-colors duration-150 disabled:opacity-50",
                active
                  ? "border-brand-400 bg-brand-50 text-brand-700 dark:bg-brand-950/40 dark:text-brand-300"
                  : "border-line bg-surface text-ink-muted hover:bg-surface-subtle",
              )}
            >
              <RotateCw
                size={20}
                style={{ transform: `rotate(${choice}deg)` }}
                className="transition-transform duration-200"
              />
              <span className="text-xs font-medium tabular-nums">{choice}°</span>
            </button>
          );
        })}
      </div>
      <p className="text-[11px] leading-relaxed text-ink-faint">{t("rotate.angleHint")}</p>
    </fieldset>
  );
}
