import { Images, RotateCcw, Trash2 } from "lucide-react";
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
import { useFeatureController } from "@/hooks/useFeatureController";
import { formatBytes } from "@/lib/format";

/**
 * 图片转 PDF 页。
 *
 * 与通用页的差别在于「没有预览」：asset 协议的 scope 只覆盖缓存目录，
 * 用户任意位置的图片无法直接放进 `<img>`，因此这里明确说明顺序即列表顺序，
 * 并给出排版方式的解释，避免用户对 `fit` / `fill` 的差别产生误解。
 */
export default function ImagesToPdfPage({ feature }: FeaturePageProps): ReactElement {
  const { t } = useTranslation();
  const controller = useFeatureController(feature);
  const { source, output, runner, values, setValue, resetValues } = controller;

  const bytes = source.files.reduce((sum, file) => sum + file.size, 0);

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
            hint={t("feature.sourceHint")}
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
                <>
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="chip">
                      <Images size={12} />
                      {t("img2pdf.summary", { count: source.files.length })}
                    </span>
                    <span className="chip">{t("common.totalSize", { size: formatBytes(bytes) })}</span>
                  </div>

                  <FileList
                    files={source.files}
                    sortable
                    onRemove={source.remove}
                    onReorder={source.reorder}
                  />
                </>
              ) : null}
            </div>
          </Section>

          <Notice tone="info">{t("img2pdf.noPreview")}</Notice>
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
              <p className="text-[11px] leading-relaxed text-ink-faint">{t("img2pdf.fitHint")}</p>
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
