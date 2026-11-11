import { RotateCcw, Trash2 } from "lucide-react";
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
import { asBoolean } from "@/features/types";
import { useFeatureController } from "@/hooks/useFeatureController";
import { usePdfThumbs } from "@/hooks/usePdfThumbs";
import { useSettings } from "@/store/settings";

/**
 * 删除空白页。
 *
 * 与旋转页同构：缩略图供用户对照确认哪些页会被判为空白；
 * 「原地修改」会覆盖源文件，此时输出目录整块隐藏，并在运行前给出警示。
 */
export default function RemoveBlankPage({ feature }: FeaturePageProps): ReactElement {
  const { t } = useTranslation();
  const controller = useFeatureController(feature);
  const { source, output, runner, values, setValue, resetValues, primary, pageCount } = controller;
  const thumbMaxPx = useSettings((state) => state.settings.thumbMaxPx);

  const inPlace = asBoolean(values, "inPlace", false);

  const thumbs = usePdfThumbs(primary?.path ?? null, {
    password: primary?.password ?? null,
    total: pageCount,
    maxPx: thumbMaxPx,
  });

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
            <Section title={t("rmblank.preview")} hint={t("rmblank.previewHint")}>
              <PageThumbGrid
                thumbs={thumbs.thumbs}
                loading={thumbs.loading}
                error={thumbs.error}
                total={pageCount}
                truncated={thumbs.truncated}
              />
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
              <OptionsForm
                fields={feature.options}
                values={values}
                onChange={setValue}
                pageCount={pageCount}
                disabled={runner.running}
              />

              {inPlace ? <Notice tone="warn">{t("rmblank.inPlaceWarning")}</Notice> : null}
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