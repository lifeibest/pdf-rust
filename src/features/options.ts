/**
 * 复用的选项字段工厂。
 *
 * 合并与图片转 PDF 共享同一组排版选项，拆分与 PDF 转图片共享页范围与打包选项，
 * 集中在这里定义可保证「同一个概念在所有功能里表现一致」。
 */

import { DEFAULT_SCOPE } from "@/lib/pages";
import type {
  NumberField,
  OptionField,
  PageRangeField,
  SelectField,
  SwitchField,
} from "./types";

export function pageSizeField(showIf?: SelectField["showIf"]): SelectField {
  return {
    type: "select",
    key: "pageSize",
    labelKey: "options.pageSize",
    showIf,
    default: "a4",
    options: [
      { value: "a4", labelKey: "options.size.a4" },
      { value: "a3", labelKey: "options.size.a3" },
      { value: "a5", labelKey: "options.size.a5" },
      { value: "letter", labelKey: "options.size.letter" },
      { value: "legal", labelKey: "options.size.legal" },
      { value: "auto", labelKey: "options.size.auto" },
    ],
  };
}

export function orientationField(showIf?: SelectField["showIf"]): SelectField {
  return {
    type: "select",
    key: "orientation",
    labelKey: "options.orientation",
    showIf,
    default: "auto",
    options: [
      { value: "portrait", labelKey: "options.orientationValue.portrait" },
      { value: "landscape", labelKey: "options.orientationValue.landscape" },
      { value: "auto", labelKey: "options.orientationValue.auto" },
    ],
  };
}

export function imageFitField(showIf?: SelectField["showIf"]): SelectField {
  return {
    type: "select",
    key: "imageFit",
    labelKey: "options.imageFit",
    showIf,
    default: "fit",
    options: [
      { value: "fit", labelKey: "options.fitValue.fit" },
      { value: "fill", labelKey: "options.fitValue.fill" },
      { value: "stretch", labelKey: "options.fitValue.stretch" },
      { value: "native_dpi", labelKey: "options.fitValue.native_dpi" },
    ],
  };
}

export function marginField(): NumberField {
  return {
    type: "number",
    key: "marginMm",
    labelKey: "options.marginMm",
    min: 0,
    max: 60,
    step: 0.5,
    default: 0,
    unitKey: "options.unit.mm",
  };
}

/** 图片像素到点的换算基准，`auto` 纸张与 `native_dpi` 排版都依赖它。 */
export function imageDpiField(): NumberField {
  return {
    type: "number",
    key: "imageDpi",
    labelKey: "options.imageDpi",
    min: 24,
    max: 1200,
    step: 1,
    default: 96,
  };
}

export function exportDpiField(): NumberField {
  return {
    type: "number",
    key: "dpi",
    labelKey: "options.dpi",
    min: 24,
    max: 1200,
    step: 1,
    default: 150,
  };
}

export function imageFormatField(): SelectField {
  return {
    type: "select",
    key: "format",
    labelKey: "options.format",
    default: "png",
    options: [
      { value: "png", labelKey: "options.formatValue.png" },
      { value: "jpeg", labelKey: "options.formatValue.jpeg" },
      { value: "webp", labelKey: "options.formatValue.webp" },
    ],
  };
}

/** JPEG 质量；WebP 走无损编码，因此仅在格式为 jpeg 时显示。 */
export function qualityField(): NumberField {
  return {
    type: "number",
    key: "quality",
    labelKey: "options.quality",
    min: 1,
    max: 100,
    step: 1,
    default: 85,
    showIf: { key: "format", value: "jpeg" },
  };
}

/** 最长边上限，0 表示沿用后端默认值（`DEFAULT_MAX_EDGE_PX`）。 */
export function maxEdgeField(): NumberField {
  return {
    type: "number",
    key: "maxEdgePx",
    labelKey: "options.maxEdgePx",
    hintKey: "options.maxEdgeHint",
    min: 0,
    max: 20_000,
    step: 100,
    default: 0,
    unitKey: "options.unit.px",
  };
}

export function zipField(): SwitchField {
  return {
    type: "switch",
    key: "zip",
    labelKey: "options.zip",
    hintKey: "options.zipHint",
    default: false,
  };
}

export function scopeField(defaultKind: PageRangeField["default"]["kind"] = "all"): PageRangeField {
  return {
    type: "pageRange",
    key: "scope",
    labelKey: "options.scope",
    placeholderKey: "options.specPlaceholder",
    default: { ...DEFAULT_SCOPE, kind: defaultKind },
  };
}

/** 把若干字段拼成有序列表，顺带过滤掉 `undefined`，便于按条件拼装。 */
export function fields(...items: (OptionField | undefined)[]): OptionField[] {
  return items.filter((item): item is OptionField => item !== undefined);
}
