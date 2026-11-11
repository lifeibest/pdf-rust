import { Shrink } from "lucide-react";

import { PDF_EXTENSIONS } from "@/features/accepts";
import { fields } from "@/features/options";
import type {
  BuildContext,
  FeatureDefinition,
  OptionValues,
  SourceFile,
} from "@/features/types";
import type { TaskRequest } from "@/lib/tauri";

/**
 * 三档压缩预设：用户只选「程度」，分辨率 / JPEG 质量这类引擎参数
 * 由前端映射，后端契约保持不变。
 */
const LEVEL_PRESETS: Record<string, { dpi: number; quality: number }> = {
  low: { dpi: 220, quality: 85 },
  medium: { dpi: 150, quality: 70 },
  high: { dpi: 100, quality: 50 },
};

function buildRequest(
  files: SourceFile[],
  values: OptionValues,
  context: BuildContext,
): TaskRequest {
  const level = typeof values.level === "string" ? values.level : "medium";
  const preset = LEVEL_PRESETS[level] ?? LEVEL_PRESETS.medium;
  return {
    kind: "compress",
    options: {
      source: files[0]?.path ?? "",
      password: files[0]?.password ?? null,
      dpi: preset.dpi,
      quality: preset.quality,
      outputName: context.outputName,
    },
  };
}

function validate(files: SourceFile[]): string | null {
  return files.length === 0 ? "feature.needOneFile" : null;
}

const compress: FeatureDefinition = {
  id: "compress",
  route: "/compress",
  titleKey: "features.compress.title",
  descKey: "features.compress.desc",
  icon: Shrink,
  accent: "from-cyan-500 to-blue-500",
  category: "organize",
  order: 40,
  accepts: { ext: [...PDF_EXTENSIONS], multiple: false },
  options: fields({
    type: "segmented",
    key: "level",
    labelKey: "options.compressLevel",
    default: "medium",
    options: [
      {
        value: "low",
        labelKey: "options.compressLevelValue.low",
        hintKey: "options.compressLevelHint.low",
      },
      {
        value: "medium",
        labelKey: "options.compressLevelValue.medium",
        hintKey: "options.compressLevelHint.medium",
      },
      {
        value: "high",
        labelKey: "options.compressLevelValue.high",
        hintKey: "options.compressLevelHint.high",
      },
    ],
  }),
  needsOutputDir: true,
  sortEnabled: false,
  thumbEnabled: true,
  mixedSources: false,
  buildRequest,
  validate,
};

export default compress;