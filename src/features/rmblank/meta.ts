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
import { asBoolean, asNumber } from "@/features/types";
import type { TaskRequest } from "@/lib/tauri";

function buildRequest(
  files: SourceFile[],
  values: OptionValues,
  context: BuildContext,
): TaskRequest {
  const inPlace = asBoolean(values, "inPlace", false);

  return {
    kind: "rmblank",
    options: {
      source: files[0]?.path ?? "",
      password: files[0]?.password ?? null,
      threshold: asNumber(values, "threshold", 0.5),
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
      type: "number",
      key: "threshold",
      labelKey: "options.threshold",
      hintKey: "options.thresholdHint",
      min: 0.01,
      max: 50,
      step: 0.1,
      default: 0.5,
      unitKey: "options.unit.percent",
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