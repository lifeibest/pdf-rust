import { Stamp } from "lucide-react";

import { PDF_EXTENSIONS } from "@/features/accepts";
import { fields, scopeField } from "@/features/options";
import type {
  BuildContext,
  FeatureDefinition,
  OptionValues,
  SourceFile,
} from "@/features/types";
import { asNumber, asScope, asString } from "@/features/types";
import { toPageScope, validateScope } from "@/lib/pages";
import type { TaskRequest, WatermarkColor, WatermarkLayout } from "@/lib/tauri";

function buildRequest(
  files: SourceFile[],
  values: OptionValues,
  context: BuildContext,
): TaskRequest {
  return {
    kind: "watermark",
    options: {
      source: files[0]?.path ?? "",
      password: files[0]?.password ?? null,
      text: asString(values, "text", "").trim(),
      layout: asString(values, "layout", "tile") as WatermarkLayout,
      color: asString(values, "color", "gray") as WatermarkColor,
      fontSize: asNumber(values, "fontSize", 36),
      opacity: asNumber(values, "opacity", 22) / 100,
      angle: asNumber(values, "angle", -35),
      scope: toPageScope(asScope(values, "scope")),
      outputName: context.outputName,
    },
  };
}

function validate(files: SourceFile[], values: OptionValues): string | null {
  if (files.length === 0) return "feature.needOneFile";
  if (asString(values, "text", "").trim().length === 0) {
    return "watermark.needText";
  }
  const total = files[0].doc?.pageCount ?? 0;
  if (total === 0) return null;
  const reason = validateScope(asScope(values, "scope"), total);
  return reason ? `errors.rangeReason.${reason}` : null;
}

const watermark: FeatureDefinition = {
  id: "watermark",
  route: "/watermark",
  titleKey: "features.watermark.title",
  descKey: "features.watermark.desc",
  icon: Stamp,
  accent: "from-cyan-500 to-blue-500",
  category: "organize",
  order: 38,
  accepts: { ext: [...PDF_EXTENSIONS], multiple: false },
  options: fields(
    {
      type: "text",
      key: "text",
      labelKey: "options.watermarkText",
      placeholderKey: "options.watermarkTextPlaceholder",
      default: "",
    },
    {
      type: "segmented",
      key: "layout",
      labelKey: "options.watermarkLayout",
      default: "tile",
      options: [
        {
          value: "tile",
          labelKey: "options.watermarkLayoutValue.tile",
          hintKey: "options.watermarkLayoutHint.tile",
        },
        {
          value: "center",
          labelKey: "options.watermarkLayoutValue.center",
          hintKey: "options.watermarkLayoutHint.center",
        },
      ],
    },
    {
      type: "select",
      key: "color",
      labelKey: "options.watermarkColor",
      default: "gray",
      options: [
        { value: "gray", labelKey: "options.watermarkColorValue.gray" },
        { value: "red", labelKey: "options.watermarkColorValue.red" },
        { value: "blue", labelKey: "options.watermarkColorValue.blue" },
        { value: "black", labelKey: "options.watermarkColorValue.black" },
      ],
    },
    {
      type: "number",
      key: "fontSize",
      labelKey: "options.watermarkFontSize",
      min: 12,
      max: 120,
      step: 2,
      default: 36,
      unitKey: "options.unit.px",
    },
    {
      type: "number",
      key: "opacity",
      labelKey: "options.watermarkOpacity",
      hintKey: "options.watermarkOpacityHint",
      min: 5,
      max: 100,
      step: 5,
      default: 22,
      unitKey: "options.unit.percent",
    },
    {
      type: "number",
      key: "angle",
      labelKey: "options.watermarkAngle",
      min: -90,
      max: 90,
      step: 5,
      default: -35,
      unitKey: "options.unit.deg",
    },
    scopeField(),
  ),
  needsOutputDir: true,
  sortEnabled: false,
  thumbEnabled: true,
  mixedSources: false,
  buildRequest,
  validate,
};

export default watermark;
