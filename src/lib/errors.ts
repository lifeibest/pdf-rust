import type { Translate } from "@/i18n";
import { isAppError, type ErrorKind } from "./tauri";

/** `AppError.kind` → i18n key。key 与 `errors.*` 段落一一对应。 */
export function errorKeyOf(kind: ErrorKind | null | undefined): string {
  if (!kind) return "errors.unknown";
  return `errors.${kind}`;
}

export interface ErrorText {
  /** 分类标题，如「PDF 处理失败」 */
  title: string;
  /** 后端给出的具体原因；`cancelled` 等无 payload 的错误为 null */
  detail: string | null;
}

/**
 * 把后端错误拆成「标题 + 具体原因」两段，交给界面分行渲染。
 *
 * 标题走 i18n，避免把英文技术信息直接甩给用户；detail 来自 Rust 侧的
 * `AppError` message，本身已经是中文。
 */
export function splitError(
  kind: ErrorKind | null | undefined,
  message: string | null | undefined,
  t: Translate,
): ErrorText {
  const title = t(errorKeyOf(kind));
  const detail = message && message.length > 0 ? message : null;
  return { title, detail: kind === "cancelled" ? null : detail };
}

/** 需要用户补密码才能继续的错误。 */
export function isPasswordError(kind: ErrorKind | null | undefined): boolean {
  return kind === "passwordRequired";
}

/** 引擎缺失属于可自愈的致命错误，界面应引导用户去「关于」页重新检测。 */
export function isEngineError(kind: ErrorKind | null | undefined): boolean {
  return kind === "pdfiumNotFound";
}

/** 配额用尽：界面应弹出激活引导，而不是当作普通失败。 */
export function isQuotaError(kind: ErrorKind | null | undefined): boolean {
  return kind === "quotaExceeded";
}

/**
 * 从任意抛出的值里取出可读文本。
 *
 * `invoke` 失败时可能是后端序列化的 `AppError`、也可能是字符串化的 Rust panic，
 * 展示层需要一个统一的兼容入口。
 */
export function messageOf(error: unknown): string {
  if (typeof error === "string") return error;
  // `Cancelled` 没有 message，不能因此退化成 "[object Object]"
  if (isAppError(error)) return error.message ?? "";
  if (typeof error === "object" && error !== null && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string") return message;
  }
  return String(error);
}
