import type { LucideIcon } from "lucide-react";
import type { ComponentType, LazyExoticComponent } from "react";

import type { DocInfo, TaskRequest } from "@/lib/tauri";
import { DEFAULT_SCOPE, type ScopeSelection } from "@/lib/pages";

/**
 * 功能 id。必须与后端 `commands/tasks.rs` 中 `TaskRequest` 的 serde rename 一致，
 * 因为前端直接把 `feature.id` 当作 `kind` 发给 `run_task`，中间没有映射表。
 */
export type FeatureId =
  | "merge"
  | "split"
  | "rotate"
  | "pdf2img"
  | "img2pdf"
  | "invoice"
  | "wechat";

/** 不属于处理功能的系统页面。 */
export type SystemPageId = "activate" | "settings" | "about";

export type PageId = FeatureId | SystemPageId | "home";

export type FeatureCategory = "convert" | "organize";

/* ------------------------------------------------------------ 选项表单 */

/** 表单值：由 `OptionsForm` 依据字段 schema 收集。 */
export type OptionValues = Record<string, string | number | boolean | ScopeSelection>;

interface FieldBase {
  /** 收集到 `OptionValues` 里的键名，也是 `buildRequest` 读取用的键 */
  key: string;
  labelKey: string;
  hintKey?: string;
  /**
   * 条件显示：仅当 `values[showIf.key] === showIf.value` 时渲染。
   * 用于「拆分方式 = 每 N 页」时才显示「每组页数」这类联动。
   */
  showIf?: { key: string; value: string };
}

export interface SelectField extends FieldBase {
  type: "select";
  options: { value: string; labelKey: string }[];
  default: string;
}

export interface NumberField extends FieldBase {
  type: "number";
  min: number;
  max: number;
  step?: number;
  default: number;
  unitKey?: string;
}

export interface SwitchField extends FieldBase {
  type: "switch";
  default: boolean;
}

export interface TextField extends FieldBase {
  type: "text";
  default: string;
  placeholderKey?: string;
}

/**
 * 页范围字段。收集到的值是 `ScopeSelection`（`all/odd/even/spec` + 表达式），
 * 由 `toPageScope()` 转成后端 `PageScope`。
 */
export interface PageRangeField extends FieldBase {
  type: "pageRange";
  default: ScopeSelection;
  /** `kind === "spec"` 时输入框的占位文案 */
  placeholderKey?: string;
}

export type OptionField =
  | SelectField
  | NumberField
  | SwitchField
  | TextField
  | PageRangeField;

/** 依据 schema 计算表单初值。 */
export function defaultValues(fields: OptionField[]): OptionValues {
  const values: OptionValues = {};
  for (const field of fields) {
    values[field.key] =
      field.type === "pageRange" ? { ...field.default } : field.default;
  }
  return values;
}

/* ------------------------------------------- 表单值读取（供 buildRequest 使用） */

export function asString(values: OptionValues, key: string, fallback = ""): string {
  const value = values[key];
  return typeof value === "string" ? value : fallback;
}

export function asNumber(values: OptionValues, key: string, fallback = 0): number {
  const value = values[key];
  if (typeof value === "number" && Number.isFinite(value)) return value;
  return fallback;
}

export function asBoolean(values: OptionValues, key: string, fallback = false): boolean {
  const value = values[key];
  return typeof value === "boolean" ? value : fallback;
}

export function asScope(values: OptionValues, key: string): ScopeSelection {
  const value = values[key];
  return typeof value === "object" && value !== null && "kind" in value
    ? (value as ScopeSelection)
    : { ...DEFAULT_SCOPE };
}

/** 空字符串归一为 `null`，与后端 `Option<String>` 的语义对齐。 */
export function asOptionalName(values: OptionValues, key: string): string | null {
  const value = asString(values, key).trim();
  return value.length === 0 ? null : value;
}

/* -------------------------------------------------------------- 源文件 */

export interface SourceFile {
  /** 列表内的稳定标识（路径可能重复，例如同一文件被添加两次） */
  id: string;
  path: string;
  name: string;
  size: number;
  /** 仅 PDF：探测到的文档信息 */
  doc?: DocInfo | null;
  /** 仅加密 PDF：用户输入的打开密码 */
  password?: string | null;
}

/* ------------------------------------------------------------ 功能定义 */

export interface FeaturePageProps {
  feature: FeatureDefinition;
}

export interface BuildContext {
  /** 已选输出目录，`null` 表示交给后端默认目录 */
  outDir: string | null;
  /** `OutputPicker` 里填写的输出文件名，空时为 `null`（后端自动命名） */
  outputName: string | null;
}

export interface FeatureDefinition {
  id: FeatureId;
  /** 路由路径，如 `/merge` */
  route: string;
  titleKey: string;
  descKey: string;
  icon: LucideIcon;
  /** 卡片渐变色，形如 `from-indigo-500 to-violet-500` */
  accent: string;
  category: FeatureCategory;
  /** 同分类内的排序，越小越靠前 */
  order: number;

  /** 允许拖入 / 选择的文件类型 */
  accepts: { ext: string[]; multiple: boolean };

  /** 由 `OptionsForm` 自动渲染的选项 */
  options: OptionField[];

  /** 是否需要输出目录（所有产出文件的功能都为 true） */
  needsOutputDir: boolean;
  /**
   * 运行管线：
   * - `"task"`（默认）：经 `GenericFeaturePage` / `useFeatureController` 走统一 `run_task`；
   * - `"custom"`：自带页面直接调用专用命令（如微信导出走 `export_wechat_pdf`），
   *   不进入 task 管线，此时 `buildRequest` 可缺省。
   */
  mode?: "task" | "custom";
  /** 是否显示拖拽排序（多来源功能） */
  sortEnabled: boolean;
  /** 是否加载页面缩略图 */
  thumbEnabled: boolean;
  /** 是否允许在同一个列表里混入多种来源（如 PDF + 图片） */
  mixedSources: boolean;

  /**
   * 把界面状态翻译为后端 `TaskRequest`。
   *
   * 有了它，`GenericFeaturePage` 才能真正跑通任意功能，
   * 后续新增简单功能只写 `meta.ts` 即可，零页面代码。
   *
   * `mode: "custom"` 的功能不经 task 管线，可省略本字段。
   */
  buildRequest?(
    files: SourceFile[],
    values: OptionValues,
    context: BuildContext,
  ): TaskRequest;

  /** 提交前校验，返回 i18n 错误 key；合法时返回 null。 */
  validate?(files: SourceFile[], values: OptionValues): string | null;

  /** 自定义页面；缺省时落到 `GenericFeaturePage`。 */
  Page?: LazyExoticComponent<ComponentType<FeaturePageProps>>;
}

/** 系统页面（设置 / 关于）在注册表中的轻量描述。 */
export interface SystemPageDefinition {
  id: SystemPageId;
  route: string;
  titleKey: string;
  descKey: string;
  icon: LucideIcon;
  order: number;
  Page: LazyExoticComponent<ComponentType>;
}
