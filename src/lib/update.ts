/**
 * 客户端更新检测（手动触发）。
 *
 * 隐私优先：不做任何自动/后台联网，只有用户在「关于」页主动点击「检测更新」时，
 * 才会去拉取一份托管在 OSS 上的静态清单 JSON，与当前版本比对后给出提示。
 *
 * 清单结构见仓库根目录 `update-manifest.json`（上传到 OSS 的源文件）。
 */

/** OSS 上的更新清单地址（静态 JSON，已开启 CORS: *）。 */
export const UPDATE_MANIFEST_URL = "https://static.pdfai.cn/pdfai_update.json";

/** 单个平台的安装包信息。 */
export interface UpdateDownload {
  label: string;
  url: string;
  fileName: string;
  size: number | null;
  sha256: string | null;
}

/** OSS 清单原始结构（字段尽量宽松，容忍线上手改）。 */
export interface UpdateManifest {
  schema?: number;
  productName?: string;
  identifier?: string;
  latestVersion: string;
  releaseDate?: string;
  minSupportedVersion?: string;
  mandatory?: boolean;
  releasePageUrl?: string;
  releaseNotes?: Record<string, string[]>;
  downloads?: Record<string, UpdateDownload>;
}

/** 一次检测后交给 UI 直接消费的结果。 */
export interface UpdateCheckResult {
  hasUpdate: boolean;
  /** 是否为强制更新：mandatory 为真，或当前版本低于 minSupportedVersion。 */
  mandatory: boolean;
  currentVersion: string;
  latestVersion: string;
  releaseDate?: string;
  releasePageUrl?: string;
  /** 已按当前语言挑好的更新要点。 */
  notes: string[];
  /** 已按当前操作系统挑好的下载项；linux 等无对应包时为 undefined。 */
  download?: UpdateDownload;
}

/**
 * 语义化版本比较：a<b 返回 -1，a>b 返回 1，相等返回 0。
 * 只按 `major.minor.patch` 数字逐段比较，缺省段按 0 处理。
 */
export function compareVersions(a: string, b: string): number {
  const pa = a.split(".").map((part) => Number.parseInt(part, 10) || 0);
  const pb = b.split(".").map((part) => Number.parseInt(part, 10) || 0);
  const len = Math.max(pa.length, pb.length);
  for (let i = 0; i < len; i += 1) {
    const x = pa[i] ?? 0;
    const y = pb[i] ?? 0;
    if (x !== y) return x < y ? -1 : 1;
  }
  return 0;
}

/**
 * 拉取清单并比对版本。
 *
 * @param currentVersion 当前客户端版本（`info.version`）
 * @param os             当前操作系统（`info.os`，取值 macos/windows/linux）
 * @param lang           当前界面语言（zh-CN/en-US），用于挑选更新要点
 */
export async function checkForUpdate(
  currentVersion: string,
  os: string,
  lang: string,
): Promise<UpdateCheckResult> {
  // no-store：每次都取最新清单，避免命中 webview 缓存导致「检测不到新版本」
  const response = await fetch(UPDATE_MANIFEST_URL, { cache: "no-store" });
  if (!response.ok) {
    throw new Error(`更新清单请求失败：HTTP ${response.status}`);
  }
  const manifest = (await response.json()) as UpdateManifest;

  if (!manifest.latestVersion) {
    throw new Error("更新清单缺少 latestVersion 字段");
  }

  const hasUpdate = compareVersions(currentVersion, manifest.latestVersion) < 0;
  const belowMinSupported = manifest.minSupportedVersion
    ? compareVersions(currentVersion, manifest.minSupportedVersion) < 0
    : false;
  const mandatory = Boolean(manifest.mandatory) || belowMinSupported;

  const notes =
    manifest.releaseNotes?.[lang] ?? manifest.releaseNotes?.["zh-CN"] ?? [];
  const download = manifest.downloads?.[os];

  return {
    hasUpdate,
    mandatory,
    currentVersion,
    latestVersion: manifest.latestVersion,
    releaseDate: manifest.releaseDate,
    releasePageUrl: manifest.releasePageUrl,
    notes,
    download,
  };
}
