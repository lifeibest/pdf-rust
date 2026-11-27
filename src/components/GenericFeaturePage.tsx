import { Trash2, RotateCcw } from "lucide-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { useFeatureController } from "@/hooks/useFeatureController";
import { usePdfThumbs } from "@/hooks/usePdfThumbs";
import { useSettings } from "@/store/settings";
import { asBoolean } from "@/features/types";
import type { FeaturePageProps } from "@/features/types";
import { DropZone } from "./DropZone";
import { FileList } from "./FileList";
import { Notice } from "./Notice";
import { OptionsForm } from "./OptionsForm";
import { OutputPicker } from "./OutputPicker";
import { PageThumbGrid } from "./PageThumbGrid";
import { Section } from "./Section";
import { TaskProgress } from "./TaskProgress";

/**
 * 通用功能页。
 *
 * 注册表里没有自定义 `Page` 的功能都落到这里，因此「新增一个简单功能」
 * 真的只需要写 `meta.ts`：来源选择、选项表单、输出设置、进度与结果
 * 全部由本页按 `FeatureDefinition` 拼装。
 */
export default function GenericFeaturePage({ feature }: FeaturePageProps): ReactElement {
  const { t } = useTranslation();
  const controller = useFeatureController(feature);
  const thumbMaxPx = useSettings((state) => state.settings.thumbMaxPx);

  const { source, output, runner, values, setValue, resetValues, primary, pageCount } = controller;
  const single = !feature.accepts.multiple;
  const showThumbs = feature.thumbEnabled && single;

  const thumbs = usePdfThumbs(showThumbs ? (primary?.path ?? null) : null, {
    password: primary?.password ?? null,
    total: pageCount,
    maxPx: thumbMaxPx,
    enabled: showThumbs,
  });

  // 约定：带 `zip` 开关的功能在打包模式或「拼成一张长图」模式下才显示输出文件名；
  // 拼成长图时只产出单张图片，自动隐藏「打包为 zip」开关避免冲突。
  const hasZipOption = feature.options.some((field) => field.key === "zip");
  const isStitching = asBoolean(values, "stitch", false);
  const showName = hasZipOption ? asBoolean(values, "zip", false) || isStitching : true;
  const visibleOptions = isStitching
    ? feature.options.filter((field) => field.key !== "zip")
    : feature.options;

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
            title={t(single ? "feature.singleSource" : "feature.source")}
            hint={feature.sortEnabled ? t("feature.sourceHint") : undefined}
            action={
              source.files.length > 0 ? (
                <button type="button" className="btn-quiet h-7 px-2 py-0 text-xs" onClick={source.clear}>
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
                  sortable={feature.sortEnabled}
                  onRemove={source.remove}
                  onReorder={source.reorder}
                  onPassword={source.setPassword}
                />
              ) : null}
            </div>
          </Section>

          {showThumbs && (thumbs.thumbs.length > 0 || thumbs.loading || thumbs.error) ? (
            <Section title={t("thumb.title")}>
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
          {feature.options.length > 0 ? (
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
              <OptionsForm
                fields={visibleOptions}
                values={values}
                onChange={setValue}
                pageCount={pageCount}
                disabled={runner.running}
              />
            </Section>
          ) : null}

          <Section title={t("feature.output")}>
            <OutputPicker
              api={output}
              showDir={feature.needsOutputDir}
              showName={showName}
              disabled={runner.running}
            />
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
