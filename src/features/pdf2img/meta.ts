import { Images } from "lucide-react";

import { PDF_EXTENSIONS } from "@/features/accepts";
import {
  exportDpiField,
  fields,
  imageFormatField,
  maxEdgeField,
  qualityField,
  scopeField,
  zipField,
} from "@/features/options";
import type {
  BuildContext,
  FeatureDefinition,
  OptionValues,
  SourceFile,
} from "@/features/types";
import { asBoolean, asNumber, asScope, asString } from "@/features/types";
import { toPageScope, validateScope } from "@/lib/pages";
import type { ImageFormat, TaskRequest } from "@/lib/tauri";

function buildRequest(
  files: SourceFile[],
  values: OptionValues,
  context: BuildContext,
): TaskRequest {
  const zip = asBoolean(values, "zip", false);
  // 0 表示「不限制」，交给后端的 DEFAULT_MAX_EDGE_PX
  const maxEdge = asNumber(values, "maxEdgePx", 0);

  return {
    kind: "pdf2img",
    options: {
      source: files[0]?.path ?? "",
      password: files[0]?.password ?? null,
      scope: toPageScope(asScope(values, "scope")),
      dpi: asNumber(values, "dpi", 150),
      format: asString(values, "format", "png") as ImageFormat,
      quality: asNumber(values, "quality", 85),
      maxEdgePx: maxEdge > 0 ? maxEdge : null,
      zip,
      // 非打包模式下文件名由后端按 `{stem}_p001.png` 规则生成，用户填的名字用不上
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

const pdf2img: FeatureDefinition = {
  id: "pdf2img",
  route: "/pdf2img",
  titleKey: "features.pdf2img.title",
  descKey: "features.pdf2img.desc",
  icon: Images,
  accent: "from-emerald-500 to-teal-500",
  category: "convert",
  order: 20,
  accepts: { ext: [...PDF_EXTENSIONS], multiple: false },
  options: fields(
    scopeField(),
    exportDpiField(),
    imageFormatField(),
    qualityField(),
    maxEdgeField(),
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

export default pdf2img;
