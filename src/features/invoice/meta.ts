import { Receipt } from "lucide-react";

import { PDF_AND_IMAGE_EXTENSIONS } from "@/features/accepts";
import {
  exportDpiField,
  fields,
  marginField,
  orientationField,
} from "@/features/options";
import type {
  BuildContext,
  FeatureDefinition,
  OptionValues,
  SelectField,
  SourceFile,
} from "@/features/types";
import { asNumber, asString } from "@/features/types";
import type { Orientation, TaskRequest } from "@/lib/tauri";

/** 每张 A4 上排布的发票张数。仅 1 / 2 / 4 三档能铺满网格，其余值后端会向 4 归一。 */
function perSheetField(): SelectField {
  return {
    type: "select",
    key: "perSheet",
    labelKey: "options.perSheet",
    hintKey: "options.perSheetHint",
    default: "2",
    options: [
      { value: "1", labelKey: "options.perSheetValue.one" },
      { value: "2", labelKey: "options.perSheetValue.two" },
      { value: "4", labelKey: "options.perSheetValue.four" },
    ],
  };
}

function buildRequest(
  files: SourceFile[],
  values: OptionValues,
  context: BuildContext,
): TaskRequest {
  return {
    kind: "invoice",
    options: {
      // 顺序即拼版顺序，直接沿用列表当前顺序；PDF 的每一页都算一张发票
      sources: files.map((file) => ({
        path: file.path,
        password: file.password ?? null,
      })),
      perSheet: Number(asString(values, "perSheet", "2")),
      orientation: asString(values, "orientation", "auto") as Orientation,
      marginMm: asNumber(values, "marginMm", 5),
      dpi: asNumber(values, "dpi", 200),
      outputName: context.outputName,
    },
  };
}

function validate(files: SourceFile[]): string | null {
  return files.length === 0 ? "feature.needFiles" : null;
}

const invoice: FeatureDefinition = {
  id: "invoice",
  route: "/invoice",
  titleKey: "features.invoice.title",
  descKey: "features.invoice.desc",
  icon: Receipt,
  accent: "from-purple-500 to-fuchsia-500",
  category: "organize",
  order: 15,
  accepts: { ext: [...PDF_AND_IMAGE_EXTENSIONS], multiple: true },
  options: fields(
    perSheetField(),
    orientationField(),
    // 默认留 5mm，既避开打印机物理边距，也充当两张发票之间的裁切间隙
    { ...marginField(), default: 5 },
    { ...exportDpiField(), default: 200 },
  ),
  needsOutputDir: true,
  sortEnabled: true,
  thumbEnabled: false,
  mixedSources: true,
  buildRequest,
  validate,
};

export default invoice;
