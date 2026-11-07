/**
 * 页码范围表达式的前端镜像，语法与 `src-tauri/src/pdf/ranges.rs` 保持一致：
 *
 * - `all` / `*`：全部页
 * - `odd` / `even`：奇数页 / 偶数页
 * - `first` / `last`：首页 / 末页
 * - `first:N` / `last:N`：前 N 页 / 后 N 页
 * - `1,3-5,8`：显式页码与区间，写反的区间（`5-3`）自动纠正
 *
 * 前端只用来做即时校验与「将影响 N 页」预览；真正的展开仍以后端为准。
 */

import type { PageScope } from "./tauri";

export type ScopeKind = "all" | "odd" | "even" | "spec";

export interface ScopeSelection {
  kind: ScopeKind;
  /** 仅 `kind === "spec"` 时使用 */
  spec: string;
}

export const DEFAULT_SCOPE: ScopeSelection = { kind: "all", spec: "" };

/** 把界面选择转换为后端 `PageScope` 的 serde 表示。 */
export function toPageScope(selection: ScopeSelection): PageScope {
  switch (selection.kind) {
    case "all":
      return "all";
    case "odd":
      return "odd";
    case "even":
      return "even";
    case "spec": {
      const spec = selection.spec.trim();
      // 空表达式退化为全部页，避免后端直接报 Range 错误
      return spec.length === 0 ? "all" : { spec };
    }
  }
}

export function scopeFromPageScope(scope: PageScope): ScopeSelection {
  if (typeof scope === "string") {
    if (scope === "all" || scope === "odd" || scope === "even") {
      return { kind: scope, spec: "" };
    }
    return { kind: "spec", spec: scope };
  }
  return { kind: "spec", spec: scope.spec };
}

/** 解析结果：成功时给出展开后的 1-based 页码（按输入顺序去重）。 */
export type RangeResult = { ok: true; pages: number[] } | { ok: false; reason: string };

/** 展开页码范围表达式；`total` 为文档总页数。 */
export function expandSpec(spec: string, total: number): RangeResult {
  const normalized = spec.trim();
  if (total <= 0) return { ok: false, reason: "emptyDocument" };
  if (normalized.length === 0) return { ok: false, reason: "empty" };

  const keyword = expandKeyword(normalized.toLowerCase(), total);
  if (keyword) {
    return keyword.length === 0
      ? { ok: false, reason: "noMatch" }
      : { ok: true, pages: keyword };
  }

  const pages: number[] = [];
  for (const rawToken of normalized.split(",")) {
    const token = rawToken.trim();
    if (token.length === 0) return { ok: false, reason: "emptyToken" };

    const group = expandToken(token, total);
    if (!group.ok) return group;
    if (group.pages.length === 0) return { ok: false, reason: "noMatch" };
    pages.push(...group.pages);
  }

  return { ok: true, pages: dedupe(pages) };
}

/** 按界面选择计算受影响页数，用于按钮旁的提示文案。 */
export function countScope(selection: ScopeSelection, total: number): number {
  if (total <= 0) return 0;
  switch (selection.kind) {
    case "all":
      return total;
    case "odd":
      return Math.ceil(total / 2);
    case "even":
      return Math.floor(total / 2);
    case "spec": {
      const result = expandSpec(selection.spec, total);
      return result.ok ? result.pages.length : 0;
    }
  }
}

/** 校验界面选择是否可提交，返回错误原因 key（合法时为 null）。 */
export function validateScope(
  selection: ScopeSelection,
  total: number,
): string | null {
  if (selection.kind !== "spec") return null;
  const result = expandSpec(selection.spec, total);
  return result.ok ? null : result.reason;
}

/* ---------------------------------------------------------------- 内部 */

function expandToken(token: string, total: number): RangeResult {
  const lower = token.toLowerCase();

  const first = parsePrefixedCount(lower, "first:");
  if (first !== null) {
    return first.ok ? { ok: true, pages: takeFromStart(first.value, total) } : first;
  }
  const last = parsePrefixedCount(lower, "last:");
  if (last !== null) {
    return last.ok ? { ok: true, pages: takeFromEnd(last.value, total) } : last;
  }

  const dash = token.indexOf("-");
  if (dash > 0) {
    const start = Number.parseInt(token.slice(0, dash).trim(), 10);
    const end = Number.parseInt(token.slice(dash + 1).trim(), 10);
    if (!Number.isInteger(start) || !Number.isInteger(end)) {
      return { ok: false, reason: "invalidNumber" };
    }
    const [low, high] = start <= end ? [start, end] : [end, start];
    if (low < 1 || high > total) return { ok: false, reason: "outOfRange" };
    return { ok: true, pages: range(low, high) };
  }

  const single = Number.parseInt(token, 10);
  if (!Number.isInteger(single)) return { ok: false, reason: "invalidNumber" };
  if (single < 1 || single > total) return { ok: false, reason: "outOfRange" };
  return { ok: true, pages: [single] };
}

function parsePrefixedCount(
  lower: string,
  prefix: string,
): { ok: true; value: number } | { ok: false; reason: string } | null {
  if (!lower.startsWith(prefix)) return null;
  const value = Number.parseInt(lower.slice(prefix.length).trim(), 10);
  if (!Number.isInteger(value) || value <= 0) {
    return { ok: false, reason: "invalidNumber" };
  }
  return { ok: true, value };
}

function expandKeyword(lower: string, total: number): number[] | null {
  switch (lower) {
    case "all":
    case "*":
      return range(1, total);
    case "odd":
      return range(1, total).filter((page) => page % 2 === 1);
    case "even":
      return range(1, total).filter((page) => page % 2 === 0);
    case "first":
      return [1];
    case "last":
      return [total];
    default:
      return null;
  }
}

function range(start: number, end: number): number[] {
  const out: number[] = [];
  for (let page = start; page <= end; page += 1) out.push(page);
  return out;
}

function takeFromStart(count: number, total: number): number[] {
  return range(1, Math.min(count, total));
}

function takeFromEnd(count: number, total: number): number[] {
  return range(Math.max(1, total - count + 1), total);
}

function dedupe(pages: number[]): number[] {
  const seen = new Set<number>();
  return pages.filter((page) => {
    if (seen.has(page)) return false;
    seen.add(page);
    return true;
  });
}
