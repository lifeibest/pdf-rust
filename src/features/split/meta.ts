import { Scissors } from "lucide-react";

import { PDF_EXTENSIONS } from "@/features/accepts";
import { fields, zipField } from "@/features/options";
import type {
  BuildContext,
  FeatureDefinition,
  OptionValues,
  SourceFile,
} from "@/features/types";
import { asBoolean, asNumber, asString } from "@/features/types";
import { expandSpec } from "@/lib/pages";
import type { SplitMode, TaskRequest } from "@/lib/tauri";

type SplitModeId = "every_n" | "each_page" | "by_ranges" | "extract";

function buildMode(values: OptionValues): SplitMode {
  const mode = asString(values, "splitMode", "every_n") as SplitModeId;
  switch (mode) {
    case "each_page":
      return { mode: "each_page" };
    case "by_ranges":
      return { mode: "by_ranges", spec: asString(values, "rangesSpec").trim() };
    case "extract":
      return { mode: "extract", spec: asString(values, "extractSpec").trim() };
    case "every_n":
    default:
      return { mode: "every_n", size: Math.max(1, asNumber(values, "groupSize", 1)) };
  }
}

function buildRequest(
  files: SourceFile[],
  values: OptionValues,
  context: BuildContext,
): TaskRequest {
  const zip = asBoolean(values, "zip", false);
  return {
    kind: "split",
    options: {
      source: files[0]?.path ?? "",
      password: files[0]?.password ?? null,
      mode: buildMode(values),
      zip,
      // 非打包模式下单个文件名由后端按 `{stem}_part001_p1-3.pdf` 规则生成
      outputName: zip ? context.outputName : null,
    },
  };
}

function validate(files: SourceFile[], values: OptionValues): string | null {
  if (files.length === 0) return "feature.needOneFile";

  const mode = asString(values, "splitMode", "every_n") as SplitModeId;
  if (mode === "every_n") {
    return asNumber(values, "groupSize", 1) >= 1 ? null : "errors.rangeReason.invalidNumber";
  }
  if (mode === "each_page") return null;

  const key = mode === "by_ranges" ? "rangesSpec" : "extractSpec";
  const spec = asString(values, key).trim();
  if (spec.length === 0) return "errors.rangeReason.empty";

  const total = files[0].doc?.pageCount ?? 0;
  if (total === 0) return null; // 文档信息还没探测完，交给后端判断

  const result = expandSpec(spec, total);
  return result.ok ? null : `errors.rangeReason.${result.reason}`;
}

const split: FeatureDefinition = {
  id: "split",
  route: "/split",
  titleKey: "features.split.title",
  descKey: "features.split.desc",
  icon: Scissors,
  accent: "from-amber-500 to-orange-500",
  category: "organize",
  order: 20,
  accepts: { ext: [...PDF_EXTENSIONS], multiple: false },
  options: fields(
    {
      type: "select",
      key: "splitMode",
      labelKey: "options.splitMode",
      default: "every_n",
      options: [
        { value: "every_n", labelKey: "options.splitValue.every_n" },
        { value: "each_page", labelKey: "options.splitValue.each_page" },
        { value: "by_ranges", labelKey: "options.splitValue.by_ranges" },
        { value: "extract", labelKey: "options.splitValue.extract" },
      ],
    },
    {
      type: "number",
      key: "groupSize",
      labelKey: "options.groupSize",
      min: 1,
      max: 9999,
      step: 1,
      default: 1,
      unitKey: "options.unit.page",
      showIf: { key: "splitMode", value: "every_n" },
    },
    {
      type: "text",
      key: "rangesSpec",
      labelKey: "options.rangesSpec",
      hintKey: "options.rangesSpecHint",
      placeholderKey: "options.specPlaceholder",
      default: "",
      showIf: { key: "splitMode", value: "by_ranges" },
    },
    {
      type: "text",
      key: "extractSpec",
      labelKey: "options.extractSpec",
      hintKey: "options.extractSpecHint",
      placeholderKey: "options.specPlaceholder",
      default: "",
      showIf: { key: "splitMode", value: "extract" },
    },
    zipField(),
  ),
  needsOutputDir: true,
  sortEnabled: false,
  thumbEnabled: true,
  mixedSources: false,
  buildRequest,
  validate,
};

export default split;
