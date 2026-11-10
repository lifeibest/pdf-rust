/**
 * VIP 激活静默上报（best-effort，绝不影响核心功能）。
 *
 * 设计约束（与「离线优先、隐私优先」定位一致）：
 * - 激活与所有 PDF 处理纯本地，本模块只在**有网时**旁路发送一条防伪记录；
 * - 上报内容只含哈希（codeHash/deviceId）与设备元信息，不含激活码/机器码原文；
 * - 全程静默：失败不弹窗、不报错、不阻塞，只入队等待下次联网重试；
 * - 后端不可用 / 无网 / 超时一律视为失败待重试，应用功能零影响（fail-open）。
 */

import { vipReportPayload, type VipReportPayload } from "@/lib/tauri";

/** 后端 VIP 上报接口。 */
export const VIP_REPORT_URL = "https://pdf-api.pdfai.cn/v1/vip";

/** 单次请求超时，避免请求挂起。 */
const REQUEST_TIMEOUT_MS = 6_000;
/** 重试退避基数与上限（30s 起，最长 30min）。 */
const BACKOFF_BASE_MS = 30_000;
const BACKOFF_MAX_MS = 30 * 60_000;
/** 周期性补报间隔（覆盖「网络恢复但没触发 online 事件」的情况）。 */
const FLUSH_INTERVAL_MS = 30 * 60_000;

const QUEUE_KEY = "pdfai.vipReportQueue";

interface PendingReport {
  /** codeHash:deviceId，用于同设备同码去重 */
  key: string;
  payload: VipReportPayload;
  /** 入队时间（毫秒） */
  ts: number;
  attempts: number;
  nextRetryAt: number;
}

function loadQueue(): PendingReport[] {
  try {
    const raw = localStorage.getItem(QUEUE_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as PendingReport[]) : [];
  } catch {
    return [];
  }
}

function saveQueue(queue: PendingReport[]): void {
  try {
    localStorage.setItem(QUEUE_KEY, JSON.stringify(queue));
  } catch {
    // 存储不可用时放弃持久化（内存中仍保留本次），静默
  }
}

/** 并发保护：同一时刻只允许一个 flush 在跑。 */
let flushing = false;

/**
 * 把当前激活信息入队（同设备同码去重）并尝试发送。
 * 未激活时 Rust 侧返回 null，直接跳过。任何异常都静默吞掉。
 */
export async function scheduleVipReport(): Promise<void> {
  try {
    const payload = await vipReportPayload();
    if (!payload) return;
    const key = `${payload.codeHash}:${payload.deviceId}`;
    const queue = loadQueue();
    if (!queue.some((item) => item.key === key)) {
      queue.push({ key, payload, ts: Date.now(), attempts: 0, nextRetryAt: 0 });
      saveQueue(queue);
    }
    void flushVipReport();
  } catch {
    // 静默：上报失败绝不影响应用
  }
}

/** 发送队列中到期的记录：成功移除，失败按指数退避排队。 */
export async function flushVipReport(): Promise<void> {
  if (flushing) return;
  flushing = true;
  try {
    const queue = loadQueue();
    if (queue.length === 0) return;
    const now = Date.now();
    const remaining: PendingReport[] = [];
    for (const item of queue) {
      if (item.nextRetryAt > now) {
        remaining.push(item);
        continue;
      }
      const ok = await sendOnce(item);
      if (ok) continue;
      item.attempts += 1;
      item.nextRetryAt = Date.now() + backoff(item.attempts);
      remaining.push(item);
    }
    saveQueue(remaining);
  } catch {
    // 静默
  } finally {
    flushing = false;
  }
}

/** 单条发送；无网/超时/后端不可达一律返回 false 待重试。 */
async function sendOnce(item: PendingReport): Promise<boolean> {
  const controller = new AbortController();
  const timer = window.setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
  try {
    const response = await fetch(VIP_REPORT_URL, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ ...item.payload, ts: item.ts }),
      signal: controller.signal,
      cache: "no-store",
    });
    return response.ok;
  } catch {
    return false;
  } finally {
    window.clearTimeout(timer);
  }
}

function backoff(attempts: number): number {
  return Math.min(BACKOFF_MAX_MS, BACKOFF_BASE_MS * 2 ** Math.min(attempts, 10));
}

let schedulerStarted = false;

/** 安装调度：online 事件 + 周期补报。幂等，只装一次。 */
export function startVipReportScheduler(): void {
  if (schedulerStarted) return;
  schedulerStarted = true;
  window.addEventListener("online", () => void flushVipReport());
  window.setInterval(() => void flushVipReport(), FLUSH_INTERVAL_MS);
}
