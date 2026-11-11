/**
 * 激活码短码兑换（激活时一次性联网）。
 *
 * 短码方案（与后端 `/v1/license/redeem`、签发工具 issue_license.py 契约一致）：
 * - 用户收到的是 8 位短码（如 K7Q2-9F4M），而非很长的签名 token；
 * - 激活时用短码 + 本机设备指纹 POST 后端，后端换回签名 token 的同时登记本机设备、
 *   判定是否超过授权台数：未超限才返回 token；超限则不返回 token（硬拦截，无法激活）；
 * - 客户端拿到 token 后交给 Rust 侧离线验签保存。兑换只发生在「激活那一刻」，之后全程离线。
 *
 * 为什么走后端而不是直接读 OSS：客户端 CSP 的 connect-src 只放行 pdf-api.pdfai.cn，
 * 且浏览器直连 OSS 静态域名会受 CORS 限制；后端用已鉴权 oss2 读对象，无 CORS 问题。
 */

import { deviceFingerprint } from "@/lib/tauri";

/** 后端兑换接口（CSP connect-src 已放行 pdf-api.pdfai.cn）。 */
export const LICENSE_REDEEM_URL = "https://pdf-api.pdfai.cn/v1/license/redeem";

/** 单次请求超时，避免激活时请求挂起。 */
const REQUEST_TIMEOUT_MS = 8_000;

/** 兑换成功：拿到签名 token 与当前设备计数。 */
export interface RedeemSuccess {
  ok: true;
  token: string;
  devices: number;
  cap: number;
}

/** 兑换失败：短码无效/不存在、超过授权台数、或网络/后端不可达。 */
export interface RedeemFailure {
  ok: false;
  reason: "notFound" | "network" | "overLimit";
  /** 仅 overLimit 时有效，用于提示文案 */
  devices?: number;
  cap?: number;
}

export type RedeemOutcome = RedeemSuccess | RedeemFailure;

/**
 * 判断输入是否为「完整签名 token」：形如 base64url.base64url，含点号。
 * 短码只含 Crockford base32 字符与可选连字符，不含点号。
 */
export function isFullToken(input: string): boolean {
  return input.includes(".");
}

interface RedeemBody {
  code: number;
  code_msg?: string;
  token?: string | null;
  status?: string;
  devices?: number;
  cap?: number;
}

/**
 * 用短码兑换签名 token：随请求带上本机设备指纹，后端登记设备并做授权台数硬校验。
 * 不抛异常，统一返回 RedeemOutcome，便于调用方区分「超限拒绝」「码无效」「网络问题」。
 */
export async function redeemShortCode(shortCode: string): Promise<RedeemOutcome> {
  const controller = new AbortController();
  const timer = window.setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
  try {
    const fingerprint = await deviceFingerprint();
    const response = await fetch(LICENSE_REDEEM_URL, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ code: shortCode.trim(), ...fingerprint }),
      signal: controller.signal,
      cache: "no-store",
    });
    // 后端始终返回 200，用 body.code 区分结果；非 2xx 视为后端/网络异常。
    if (!response.ok) return { ok: false, reason: "network" };
    const data = (await response.json()) as RedeemBody;
    if (data.code === 0 && data.token) {
      return { ok: true, token: data.token, devices: data.devices ?? 0, cap: data.cap ?? 0 };
    }
    // 429=超过授权台数上限（硬拦截）；400/404=短码无效或不存在；500=OSS/后端故障归为无效。
    if (data.code === 429) {
      return { ok: false, reason: "overLimit", devices: data.devices ?? 0, cap: data.cap ?? 0 };
    }
    return { ok: false, reason: "notFound" };
  } catch {
    // 网络中断/超时/解析失败等，统一为 network。
    return { ok: false, reason: "network" };
  } finally {
    window.clearTimeout(timer);
  }
}
