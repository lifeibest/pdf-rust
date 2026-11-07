import { RotateCw } from "lucide-react";
import { lazy } from "react";

import { PDF_EXTENSIONS } from "@/features/accepts";
import { fields, scopeField } from "@/features/options";
import type {
  BuildContext,
  FeatureDefinition,
  OptionValues,
  SourceFile,
} from "@/features/types";
import { asBoolean, asScope, asString } from "@/features/types";
import { toPageScope, validateScope } from "@/lib/pages";
import type { TaskRequest } from "@/lib/tauri";

function buildRequest(
  files: SourceFile[],
  values: OptionValues,
  context: BuildContext,
): TaskRequest {
  const inPlace = asBoolean(values, "inPlace", false);
  // 下拉项的值是字符串，这里显式转成后端需要的 i32
  const degrees = Number.parseInt(asString(values, "degrees", "90"), 10);

  return {
    kind: "rotate",
    options: {
      source: files[0]?.path ?? "",
      password: files[0]?.password ?? null,
      degrees: Number.isFinite(degrees) ? degrees : 90,
      scope: toPageScope(asScope(values, "scope")),
      inPlace,
      // 原地模式直接覆盖源文件，输出名没有意义
      outputName: inPlace ? null : context.outputName,
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

const rotate: FeatureDefinition = {
  id: "rotate",
  route: "/rotate",
  titleKey: "features.rotate.title",
  descKey: "features.rotate.desc",
  icon: RotateCw,
  accent: "from-rose-500 to-pink-500",
  category: "organize",
  order: 30,
  accepts: { ext: [...PDF_EXTENSIONS], multiple: false },
  options: fields(
    {
      type: "select",
      key: "degrees",
      labelKey: "options.degrees",
      // 选项值仍是字符串，`asNumber` 负责转换；后端只接受 90 的整数倍
      default: "90",
      options: [
        { value: "90", labelKey: "options.degreesValue.90" },
        { value: "180", labelKey: "options.degreesValue.180" },
        { value: "270", labelKey: "options.degreesValue.270" },
      ],
    },
    scopeField(),
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

export default rotate;

/** 供旋转预览网格使用的角度取值，与上方 select 选项一一对应。 */
export const ROTATION_CHOICES = [90, 180, 270] as const;
