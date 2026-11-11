import { ImageDown } from "lucide-react";

import { PDF_EXTENSIONS } from "@/features/accepts";
import { fields, qualityField, scopeField, zipField } from "@/features/options";
import type {
  BuildContext,
  FeatureDefinition,
  OptionValues,
  SourceFile,
} from "@/features/types";
import { asBoolean, asNumber, asScope, asString } from "@/features/types";
import { toPageScope, validateScope } from "@/lib/pages";
import type { ExtractFormat, TaskRequest } from "@/lib/tauri";

function buildRequest(
  files: SourceFile[],
  values: OptionValues,
  context: BuildContext,
): TaskRequest {
  const zip = asBoolean(values, "zip", false);

  return {
    kind: "extractimg",
    options: {
      source: files[0]?.path ?? "",
      password: files[0]?.password ?? null,
      scope: toPageScope(asScope(values, "scope")),
      minSizeKb: asNumber(values, "minSizeKb", 0),
      format: asString(values, "format", "auto") as ExtractFormat,
      quality: asNumber(values, "quality", 85),
      zip,
      // 非打包模式下文件名由后端按 `{stem}_p001_i01.jpg` 规则生成
      outputName: zip ? context.outputName : null,
    },
  };
}

function validate(files: SourceFile[], values: OptionValues): string | null {
  if (files.length === 0) return "feature.needOneFile";

  const total = files[0].doc?.pageCount ?? 0;
  if (total === 0) return null; // 文档信息还没探测完，交给后端判断

  const reason = validateScope(asScope(values, "scope"), total);
  return reason ? `errors.rangeReason.${reason}` : null;
}

const extractimg: FeatureDefinition = {
  id: "extractimg",
  route: "/extractimg",
  titleKey: "features.extractimg.title",
  descKey: "features.extractimg.desc",
  icon: ImageDown,
  accent: "from-teal-500 to-cyan-500",
  category: "convert",
  order: 25,
  accepts: { ext: [...PDF_EXTENSIONS], multiple: false },
  options: fields(
    scopeField(),
    {
      type: "number",
      key: "minSizeKb",
      labelKey: "options.minSizeKb",
      hintKey: "options.minSizeHint",
      min: 0,
      max: 102_400,
      step: 1,
      default: 0,
      unitKey: "options.unit.kb",
    },
    {
      type: "select",
      key: "format",
      labelKey: "options.format",
      default: "auto",
      options: [
        { value: "auto", labelKey: "options.extractFormatValue.auto" },
        { value: "png", labelKey: "options.formatValue.png" },
        { value: "jpeg", labelKey: "options.formatValue.jpeg" },
      ],
    },
    qualityField(),
    zipField(),
  ),
  needsOutputDir: true,
  sortEnabled: false,
  thumbEnabled: true,
  mixedSources: false,
  buildRequest,
  validate,
  // 没有自定义 Page：通用页（DropZone + 页面预览 + 选项表单 + 输出）已经够用
};

export default extractimg;