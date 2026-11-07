/** 展示层的格式化工具：体积、耗时、相对时间。全部为纯函数。 */

import type { Translate } from "@/i18n";

const UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

/** 字节数转可读体积，保留一位小数（B 与整数不带小数）。 */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";

  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const text = unit === 0 || Number.isInteger(value) ? String(Math.round(value)) : value.toFixed(1);
  return `${text} ${UNITS[unit]}`;
}

/**
 * 毫秒转「1.2 秒 / 3 分 05 秒」形式，用于任务耗时。
 *
 * 单位词走 i18n，因此需要把 `t` 传进来；体积单位（B/KB/…）是国际通用的，不翻译。
 */
export function formatDuration(ms: number, t: Translate): string {
  if (!Number.isFinite(ms) || ms < 0) return t("common.durationSeconds", { value: "0.0" });
  if (ms < 1000) return t("common.durationMs", { value: ms });

  const totalSeconds = Math.round(ms / 1000);
  if (totalSeconds < 60) {
    return t("common.durationSeconds", { value: (ms / 1000).toFixed(1) });
  }

  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes < 60) {
    return t("common.durationMinutesSeconds", {
      minutes,
      seconds: String(seconds).padStart(2, "0"),
    });
  }

  const hours = Math.floor(minutes / 60);
  return t("common.durationHoursMinutes", { hours, minutes: minutes % 60 });
}

/** Unix 毫秒时间戳转「刚刚 / 5 分钟前 / 3 天前 / 具体日期」。 */
export function formatRelativeTime(
  timestampMs: number,
  t: Translate,
  now = Date.now(),
): string {
  const diff = now - timestampMs;
  if (diff < 60_000) return t("common.timeJustNow");

  const minutes = Math.floor(diff / 60_000);
  if (minutes < 60) return t("common.timeMinutes", { value: minutes });

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return t("common.timeHours", { value: hours });

  const days = Math.floor(hours / 24);
  if (days < 7) return t("common.timeDays", { value: days });

  return new Date(timestampMs).toLocaleDateString(undefined, {
    month: "2-digit",
    day: "2-digit",
  });
}

/** 进度百分比，`total` 为 0 时返回 0，避免出现 NaN。 */
export function percent(current: number, total: number): number {
  if (total <= 0) return 0;
  return Math.max(0, Math.min(100, Math.round((current / total) * 100)));
}
