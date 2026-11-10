/**
 * ★ 功能注册表 —— 整个前端的扩展核心。
 *
 * 路由、侧边栏分组、首页卡片、全局拖拽匹配全部由这里导出的数据生成。
 * 新增一个功能：写 `features/<id>/meta.ts`，然后在下方 `FEATURES` 数组里加一行，
 * 不需要改动任何布局或路由代码。
 */

import type { FeatureCategory, FeatureDefinition, SystemPageDefinition } from "./types";

import activate from "./activate/meta";
import img2pdf from "./img2pdf/meta";
import invoice from "./invoice/meta";
import merge from "./merge/meta";
import pdf2img from "./pdf2img/meta";
import rotate from "./rotate/meta";
import split from "./split/meta";
import about from "./about/meta";
import settings from "./settings/meta";

/** 处理类功能。`order` 决定同分类内的先后。 */
export const FEATURES: FeatureDefinition[] = [
  merge,
  invoice,
  split,
  rotate,
  pdf2img,
  img2pdf,
].sort((left, right) => left.order - right.order || left.id.localeCompare(right.id));

/** 获取激活码 / 设置 / 关于，固定在侧边栏底部。 */
export const SYSTEM_PAGES: SystemPageDefinition[] = [activate, settings, about].sort(
  (left, right) => left.order - right.order,
);

/** 侧边栏分组顺序与标题 key。 */
export const CATEGORIES: { id: FeatureCategory; titleKey: string }[] = [
  { id: "convert", titleKey: "nav.convert" },
  { id: "organize", titleKey: "nav.organize" },
];

export function featureById(id: string): FeatureDefinition | undefined {
  return FEATURES.find((feature) => feature.id === id);
}

export function systemPageById(id: string): SystemPageDefinition | undefined {
  return SYSTEM_PAGES.find((page) => page.id === id);
}

export function featuresByCategory(category: FeatureCategory): FeatureDefinition[] {
  return FEATURES.filter((feature) => feature.category === category);
}

/**
 * 按拖入文件的扩展名匹配可用功能。
 *
 * 规则：功能的 `accepts.ext` 必须覆盖全部拖入类型；结果按「接受范围越窄越靠前」
 * 排序，因此调用方可以直接取第一个作为最可能的意图，多于一个时弹出选择器。
 */
export function matchFeatures(extensions: string[]): FeatureDefinition[] {
  if (extensions.length === 0) return [];

  const unique = [...new Set(extensions.map((ext) => ext.toLowerCase()))];
  return FEATURES.filter((feature) => {
    const allowed = new Set(feature.accepts.ext);
    return unique.every((ext) => allowed.has(ext));
  }).sort(
    (left, right) =>
      left.accepts.ext.length - right.accepts.ext.length || left.order - right.order,
  );
}

/** 所有功能接受过的扩展名合集，供全局拖拽做第一层过滤。 */
export const ACCEPTED_EXTENSIONS: string[] = [
  ...new Set(FEATURES.flatMap((feature) => feature.accepts.ext)),
];
