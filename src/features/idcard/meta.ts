import { CreditCard } from "lucide-react";

import { IMAGE_EXTENSIONS } from "@/features/accepts";
import { fields } from "@/features/options";
import type {
  BuildContext,
  FeatureDefinition,
  OptionValues,
  SourceFile,
} from "@/features/types";
import { asNumber, asString } from "@/features/types";
import type { ImageEnhance, TaskRequest } from "@/lib/tauri";

function buildRequest(
  files: SourceFile[],
  values: OptionValues,
  context: BuildContext,
): TaskRequest {
  const wm = asString(values, "watermarkText", "").trim();
  return {
    kind: "idcard",
    options: {
      sources: files.map((file) => file.path),
      enhance: asString(values, "enhance", "none") as ImageEnhance,
      watermarkText: wm.length > 0 ? wm : null,
      watermarkOpacity: asNumber(values, "watermarkOpacity", 25) / 100,
      outputName: context.outputName,
    },
  };
}

function validate(files: SourceFile[]): string | null {
  if (files.length === 0) return "feature.needFiles";
  if (files.length > 2) return "idcard.tooMany";
  return null;
}

const idcard: FeatureDefinition = {
  id: "idcard",
  route: "/idcard",
  titleKey: "features.idcard.title",
  descKey: "features.idcard.desc",
  icon: CreditCard,
  accent: "from-indigo-500 to-sky-500",
  category: "organize",
  order: 18,
  accepts: { ext: [...IMAGE_EXTENSIONS], multiple: true },
  options: fields(
    {
      type: "segmented",
      key: "enhance",
      labelKey: "options.enhance",
      default: "none",
      options: [
        {
          value: "none",
          labelKey: "options.enhanceValue.none",
          hintKey: "options.enhanceHint.none",
        },
        {
          value: "bleach",
          labelKey: "options.enhanceValue.bleach",
          hintKey: "options.enhanceHint.bleach",
        },
        {
          value: "bw",
          labelKey: "options.enhanceValue.bw",
          hintKey: "options.enhanceHint.bw",
        },
      ],
    },
    {
      type: "text",
      key: "watermarkText",
      labelKey: "options.idcardWatermark",
      hintKey: "options.idcardWatermarkHint",
      placeholderKey: "options.idcardWatermarkPlaceholder",
      default: "",
    },
    {
      type: "number",
      key: "watermarkOpacity",
      labelKey: "options.watermarkOpacity",
      min: 5,
      max: 100,
      step: 5,
      default: 25,
      unitKey: "options.unit.percent",
    },
  ),
  needsOutputDir: true,
  sortEnabled: true,
  thumbEnabled: false,
  mixedSources: false,
  buildRequest,
  validate,
};

export default idcard;
