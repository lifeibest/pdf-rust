import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { validateScope } from "@/lib/pages";
import { useSettings } from "@/store/settings";
import { usePendingDrop } from "./useDropFiles";
import { useSourceFiles, type SourceFilesApi } from "./useSourceFiles";
import { useOutputPicker, type OutputPickerApi } from "./useOutputPicker";
import { useTaskRunner, type TaskRunner } from "./useTaskRunner";
import {
  asScope,
  defaultValues,
  type FeatureDefinition,
  type OptionValues,
  type SourceFile,
} from "@/features/types";

export interface RunOverrides {
  /** 覆盖 `feature.needsOutputDir`（旋转的原地模式不需要输出目录） */
  needsDir?: boolean;
}

export interface FeatureController {
  source: SourceFilesApi;
  values: OptionValues;
  setValue(key: string, value: OptionValues[string]): void;
  resetValues(): void;
  output: OutputPickerApi;
  runner: TaskRunner;
  /** 单来源功能的当前文档；多来源功能为列表第一项 */
  primary: SourceFile | null;
  /** 主文档页数，未知时为 0（加密未解锁、探测失败） */
  pageCount: number;
  canRun: boolean;
  /** `canRun` 为假时显示给用户的原因 */
  hint: string | null;
  run(overrides?: RunOverrides): Promise<void>;
  /** 完成后清空来源，方便再处理一批 */
  runAnother(): void;
}

/**
 * 功能页的通用控制器。
 *
 * 把「来源文件 + 选项表单 + 输出设置 + 任务运行」四件事收拢到一处，
 * `GenericFeaturePage` 与各自定义页都基于它，因此行为完全一致，
 * 差别只在版面编排。
 */
export function useFeatureController(feature: FeatureDefinition): FeatureController {
  const { t } = useTranslation();

  const source = useSourceFiles(feature);
  const output = useOutputPicker({ enabled: feature.needsOutputDir });
  const runner = useTaskRunner(feature.id);
  const notifyOnFinish = useSettings((state) => state.settings.notifyOnFinish);

  const [values, setValues] = useState<OptionValues>(() => defaultValues(feature.options));

  const setValue = useCallback((key: string, value: OptionValues[string]) => {
    setValues((current) => ({ ...current, [key]: value }));
  }, []);

  const resetValues = useCallback(() => {
    setValues(defaultValues(feature.options));
  }, [feature.options]);

  // 全局拖拽命中本功能时，把路径补进来源列表
  usePendingDrop(feature.id, source.add);

  const primary = source.files.length > 0 ? source.files[0] : null;
  const pageCount = primary?.doc?.pageCount ?? 0;

  /** 页范围字段的即时校验；页数未知时跳过，交给后端兜底 */
  const scopeError = useMemo(() => {
    if (pageCount <= 0) return null;
    for (const field of feature.options) {
      if (field.type !== "pageRange") continue;
      if (field.showIf && values[field.showIf.key] !== field.showIf.value) continue;
      const reason = validateScope(asScope(values, field.key), pageCount);
      if (reason) return `errors.rangeReason.${reason}`;
    }
    return null;
  }, [feature.options, pageCount, values]);

  const customError = feature.validate?.(source.files, values) ?? null;

  const hint = useMemo(() => {
    if (source.files.length === 0) {
      return t(feature.accepts.multiple ? "feature.needFiles" : "feature.needOneFile");
    }
    if (source.needsPassword) return t("errors.passwordRequired");
    if (scopeError) return t(scopeError);
    if (customError) return t(customError);
    return null;
  }, [customError, feature.accepts.multiple, scopeError, source.files.length, source.needsPassword, t]);

  const canRun = hint === null && !source.busy && !runner.running;

  const run = useCallback(
    async (overrides: RunOverrides = {}) => {
      if (!canRun) return;
      // custom 模式功能不经 task 管线，也没有 buildRequest，此处直接拒绝
      if (!feature.buildRequest) return;

      const needsDir = overrides.needsDir ?? feature.needsOutputDir;
      const dir = needsDir ? await output.resolve() : null;
      // 「每次询问」模式下用户取消了目录选择，此时不该悄悄写到默认目录
      if (needsDir && !dir) return;

      const request = feature.buildRequest(source.files, values, {
        outDir: dir,
        outputName: output.name.trim().length > 0 ? output.name.trim() : null,
      });

      const record = await runner.start(request, {
        outDir: dir,
        password: primary?.password ?? null,
        notify: notifyOnFinish,
      });
      if (record) output.commit(dir);
    },
    [
      canRun,
      feature,
      notifyOnFinish,
      output,
      primary?.password,
      runner,
      source.files,
      values,
    ],
  );

  const runAnother = useCallback(() => {
    source.clear();
    runner.reset();
  }, [runner, source]);

  return {
    source,
    values,
    setValue,
    resetValues,
    output,
    runner,
    primary,
    pageCount,
    canRun,
    hint,
    run,
    runAnother,
  };
}
