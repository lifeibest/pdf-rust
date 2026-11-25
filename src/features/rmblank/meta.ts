import { Eraser } from "lucide-react";
import { lazy } from "react";

import { PDF_EXTENSIONS } from "@/features/accepts";
import { fields } from "@/features/options";
import type {
  BuildContext,
  FeatureDefinition,
  OptionValues,
  SourceFile,
} from "@/features/types";
import { asBoolean, asString } from "@/features/types";
import type { TaskRequest } from "@/lib/tauri";

const BLANK_PRESET_THRESHOLD: Record<string, number> = {
  strict: 0.2,
  standard: 0.5,
  loose: 1.5,
};

function buildRequest(
  files: SourceFile[],
  values: OptionValues,
  context: BuildContext,
): TaskRequest {
  const inPlace = asBoolean(values, "inPlace", false);
  const preset = asString(values, "blankPreset", "standard");
  const threshold = BLANK_PRESET_THRESHOLD[preset] ?? 0.5;

  return {
    kind: "rmblank",
    options: {
      source: files[0]?.path ?? "",
      password: files[0]?.password ?? null,
      threshold,
      inPlace,
      // 原地模式直接覆盖源文件，输出名没有意义
      outputName: inPlace ? null : context.outputName,
    },
  };
}

function validate(files: SourceFile[]): string | null {
  return files.length === 0 ? "feature.needOneFile" : null;
}

const rmblank: FeatureDefinition = {
  id: "rmblank",
  route: "/rmblank",
  titleKey: "features.rmblank.title",
  descKey: "features.rmblank.desc",
  icon: Eraser,
  accent: "from-lime-500 to-green-500",
  category: "organize",
  order: 35,
  accepts: { ext: [...PDF_EXTENSIONS], multiple: false },
  options: fields(
    {
      type: "segmented",
      key: "blankPreset",
      labelKey: "options.blankPreset",
      default: "standard",
      options: [
        {
          value: "strict",
          labelKey: "options.blankPresetValue.strict",
          hintKey: "options.blankPresetHint.strict",
        },
        {
          value: "standard",
          labelKey: "options.blankPresetValue.standard",
          hintKey: "options.blankPresetHint.standard",
        },
        {
          value: "loose",
          labelKey: "options.blankPresetValue.loose",
          hintKey: "options.blankPresetHint.loose",
        },
      ],
    },
    {
      type: "switch",
      key: "inPlace",
      labelKey: "options.inPlace",
      hintKey: "options.inPlaceHint",
      default: false,
    },
  ),
  needsOutputDir: true,
  sortEnabled: false,
  thumbEnabled: true,
  mixedSources: false,
  buildRequest,
  validate,
  Page: lazy(() => import("./index")),
};

export default rmblank;