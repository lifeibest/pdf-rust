import { Combine } from "lucide-react";
import { lazy } from "react";

import { PDF_AND_IMAGE_EXTENSIONS } from "@/features/accepts";
import {
  fields,
  imageDpiField,
  imageFitField,
  marginField,
  orientationField,
  pageSizeField,
} from "@/features/options";
import type {
  BuildContext,
  FeatureDefinition,
  OptionValues,
  SourceFile,
} from "@/features/types";
import { asBoolean, asNumber, asString } from "@/features/types";
import type {
  ImageFit,
  Orientation,
  PageSize,
  TaskRequest,
} from "@/lib/tauri";

function buildRequest(
  files: SourceFile[],
  values: OptionValues,
  context: BuildContext,
): TaskRequest {
  return {
    kind: "merge",
    options: {
      // 顺序即输出页面顺序，因此这里直接沿用列表当前顺序
      sources: files.map((file) => ({
        path: file.path,
        password: file.password ?? null,
      })),
      pageSize: asString(values, "pageSize", "a4") as PageSize,
      orientation: asString(values, "orientation", "auto") as Orientation,
      imageFit: asString(values, "imageFit", "fit") as ImageFit,
      marginMm: asNumber(values, "marginMm", 0),
      imageDpi: asNumber(values, "imageDpi", 96),
      flattenAnnotations: asBoolean(values, "flattenAnnotations", false),
      outputName: context.outputName,
    },
  };
}

function validate(files: SourceFile[]): string | null {
  return files.length === 0 ? "feature.needFiles" : null;
}

const merge: FeatureDefinition = {
  id: "merge",
  route: "/merge",
  titleKey: "features.merge.title",
  descKey: "features.merge.desc",
  icon: Combine,
  accent: "from-indigo-500 to-violet-500",
  category: "organize",
  order: 10,
  accepts: { ext: [...PDF_AND_IMAGE_EXTENSIONS], multiple: true },
  options: fields(
    pageSizeField(),
    orientationField(),
    imageFitField(),
    marginField(),
    imageDpiField(),
    {
      type: "switch",
      key: "flattenAnnotations",
      labelKey: "options.flattenAnnotations",
      hintKey: "options.flattenHint",
      default: false,
    },
  ),
  needsOutputDir: true,
  sortEnabled: true,
  thumbEnabled: true,
  mixedSources: true,
  buildRequest,
  validate,
  Page: lazy(() => import("./index")),
};

export default merge;
