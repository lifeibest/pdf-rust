import { ImagePlus } from "lucide-react";
import { lazy } from "react";

import { IMAGE_EXTENSIONS } from "@/features/accepts";
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
import { asNumber, asString } from "@/features/types";
import type { ImageFit, Orientation, PageSize, TaskRequest } from "@/lib/tauri";

function buildRequest(
  files: SourceFile[],
  values: OptionValues,
  context: BuildContext,
): TaskRequest {
  return {
    kind: "img2pdf",
    options: {
      sources: files.map((file) => file.path),
      pageSize: asString(values, "pageSize", "a4") as PageSize,
      orientation: asString(values, "orientation", "auto") as Orientation,
      imageFit: asString(values, "imageFit", "fit") as ImageFit,
      marginMm: asNumber(values, "marginMm", 0),
      imageDpi: asNumber(values, "imageDpi", 96),
      outputName: context.outputName,
    },
  };
}

function validate(files: SourceFile[]): string | null {
  return files.length === 0 ? "feature.needFiles" : null;
}

const img2pdf: FeatureDefinition = {
  id: "img2pdf",
  route: "/img2pdf",
  titleKey: "features.img2pdf.title",
  descKey: "features.img2pdf.desc",
  icon: ImagePlus,
  accent: "from-sky-500 to-cyan-500",
  category: "convert",
  order: 10,
  accepts: { ext: [...IMAGE_EXTENSIONS], multiple: true },
  options: fields(
    pageSizeField(),
    orientationField(),
    imageFitField(),
    marginField(),
    imageDpiField(),
  ),
  needsOutputDir: true,
  sortEnabled: true,
  thumbEnabled: false,
  mixedSources: false,
  buildRequest,
  validate,
  Page: lazy(() => import("./index")),
};

export default img2pdf;
