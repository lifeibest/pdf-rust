/**
 * 后端契约的 TypeScript 镜像。
 *
 * 这里的每个类型都对应 `src-tauri/src` 中一个 serde 结构，字段名与
 * `#[serde(rename_all = "camelCase")]` 的结果一致。修改后端契约时必须同步这里，
 * 否则 `invoke` 会在运行时才暴露问题（TS 编译期查不出来）。
 */

import { convertFileSrc, invoke } from "@tauri-apps/api/core";

/**
 * 把本地绝对路径转成 asset 协议 URL。
 *
 * 缩略图以 PNG 落在 app cache 目录，用 `<img src>` 直接读取，
 * 避免把图片编码成 base64 塞进 IPC 消息。
 */
export const assetUrl = (path: string) => convertFileSrc(path);

/* ------------------------------------------------------------------ 错误 */

/** `AppError` 的 serde 邻接标签表示：`{ kind, message }`。 */
export type ErrorKind =
  | "pdfiumNotFound"
  | "passwordRequired"
  | "pdf"
  | "io"
  | "image"
  | "archive"
  | "invalidParams"
  | "range"
  | "unsupported"
  | "cancelled"
  | "quotaExceeded"
  | "licenseInvalid"
  | "internal";

export interface AppError {
  kind: ErrorKind;
  /** `Cancelled` 没有 payload，此时为 null */
  message: string | null;
}

export function isAppError(value: unknown): value is AppError {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { kind?: unknown }).kind === "string"
  );
}

/* -------------------------------------------------------------- 通用类型 */

export type PageSize = "a4" | "a3" | "a5" | "letter" | "legal" | "auto";
export type Orientation = "portrait" | "landscape" | "auto";
export type ImageFit = "fit" | "fill" | "stretch" | "native_dpi";
export type ImageFormat = "png" | "jpeg" | "webp";
/**
 * 作用页范围。Rust 侧是 `#[serde(rename_all = "snake_case")]` 的外部标签枚举，
 * 因此 `Spec("1,3")` 在线上是 `{ spec: "1,3" }`，其余变体是裸字符串。
 */
export type PageScope = "all" | "odd" | "even" | { spec: string };

export interface OutputFile {
  path: string;
  name: string;
  kind: "pdf" | "image" | "archive";
  sizeBytes: number;
  pages: number | null;
}

export interface PageInfo {
  index: number;
  widthPt: number;
  heightPt: number;
  rotationDeg: number;
}

export interface DocMetadata {
  title: string | null;
  author: string | null;
  subject: string | null;
  keywords: string | null;
  creator: string | null;
  producer: string | null;
  creationDate: string | null;
  modificationDate: string | null;
}

export interface DocInfo {
  path: string;
  fileName: string;
  fileSize: number;
  pageCount: number;
  encrypted: boolean;
  passwordFailed: boolean;
  pdfVersion: string | null;
  metadata: DocMetadata;
  pages: PageInfo[];
}

export interface ThumbOut {
  page: number;
  path: string;
  width: number;
  height: number;
  rotationDeg: number;
}

/* ---------------------------------------------------------------- 任务 */

export type TaskStatus = "running" | "finished" | "failed" | "cancelled";

export interface TaskRecord {
  id: string;
  feature: string;
  status: TaskStatus;
  current: number;
  total: number;
  message: string;
  outputs: OutputFile[];
  error: string | null;
  errorKind: ErrorKind | null;
  startedAt: number;
  elapsedMs: number;
}

export interface ProgressPayload {
  taskId: string;
  feature: string;
  current: number;
  total: number;
  message: string;
}

export interface LogPayload {
  taskId: string;
  level: string;
  message: string;
}

export interface FinishedPayload {
  taskId: string;
  feature: string;
  outputs: OutputFile[];
  elapsedMs: number;
}

export interface FailedPayload {
  taskId: string;
  feature: string;
  error: AppError;
  elapsedMs: number;
}

export interface CleanupReport {
  thumbBytes: number;
  tempDirs: number;
  tempBytes: number;
}

/** `TaskRequest`：`kind` 与功能 id 一致，`options` 为对应功能的选项结构。 */
export type TaskRequest =
  | { kind: "merge"; options: MergeOptions }
  | { kind: "split"; options: SplitOptions }
  | { kind: "rotate"; options: RotateOptions }
  | { kind: "pdf2img"; options: ExportImagesOptions }
  | { kind: "img2pdf"; options: ImagesToPdfOptions };

export interface MergeSource {
  path: string;
  password: string | null;
}

export interface MergeOptions {
  sources: MergeSource[];
  pageSize: PageSize;
  orientation: Orientation;
  imageFit: ImageFit;
  marginMm: number;
  imageDpi: number;
  flattenAnnotations: boolean;
  outputName: string | null;
}

export type SplitMode =
  | { mode: "every_n"; size: number }
  | { mode: "each_page" }
  | { mode: "by_ranges"; spec: string }
  | { mode: "extract"; spec: string };

export interface SplitOptions {
  source: string;
  password: string | null;
  mode: SplitMode;
  zip: boolean;
  outputName: string | null;
}

export interface RotateOptions {
  source: string;
  password: string | null;
  degrees: number;
  scope: PageScope;
  inPlace: boolean;
  outputName: string | null;
}

export interface ExportImagesOptions {
  source: string;
  password: string | null;
  scope: PageScope;
  dpi: number;
  format: ImageFormat;
  quality: number;
  maxEdgePx: number | null;
  zip: boolean;
  outputName: string | null;
}

export interface ImagesToPdfOptions {
  sources: string[];
  pageSize: PageSize;
  orientation: Orientation;
  imageFit: ImageFit;
  marginMm: number;
  imageDpi: number;
  outputName: string | null;
}

/* -------------------------------------------------------------- 应用信息 */

export interface PdfiumManifest {
  tag: string | null;
  asset: string | null;
  sha256: string | null;
  bytes: number | null;
  downloadedAt: string | null;
}

export interface EngineStatus {
  ready: boolean;
  source: string | null;
  libraryPath: string | null;
  expectedTag: string;
  manifest: PdfiumManifest | null;
  error: string | null;
  searched: string[];
}

export interface AppInfo {
  name: string;
  version: string;
  identifier: string;
  tauriVersion: string;
  os: string;
  arch: string;
  resourceDir: string | null;
  cacheDir: string | null;
  configDir: string | null;
  defaultOutputDir: string | null;
  pdfium: EngineStatus;
  imageExtensions: string[];
}

/* ------------------------------------------------------------ 授权配额 */

/** 激活码授权状态，字段与 Rust `license::LicenseStatus` 对齐。 */
export interface LicenseStatus {
  active: boolean;
  plan: string | null;
  quotaUnlimited: boolean;
  quotaPerDay: number | null;
  exp: number | null;
  licenseId: string | null;
  /** token 绑定的机器码（未绑定为 null） */
  machine: string | null;
  /** 本机机器码，申请绑定码时提供给签发方 */
  machineCode: string;
  /** 未激活 / 校验失败原因 */
  reason: string | null;
}

/** 今日配额快照，字段与 Rust `usage::QuotaStatus` 对齐。 */
export interface QuotaStatus {
  licensed: boolean;
  unlimited: boolean;
  limit: number | null;
  used: number;
  remaining: number | null;
  allowed: boolean;
  date: string;
}

/** VIP 静默上报 payload，字段与 Rust `commands::license::VipReportPayload` 对齐。 */
export interface VipReportPayload {
  codeHash: string;
  deviceId: string;
  deviceIdReliable: boolean;
  plan: string;
  os: string;
  arch: string;
  appVersion: string;
}

/* ------------------------------------------------------------------ 文件 */

export interface PickFilesArgs {
  title?: string | null;
  extensions?: string[];
  filterName?: string | null;
  multiple?: boolean | null;
  directory?: string | null;
}

export interface PickedFiles {
  paths: string[];
  cancelled: boolean;
}

/** `stat_paths` 返回的轻量元信息。 */
export interface PathStat {
  path: string;
  name: string;
  exists: boolean;
  isDir: boolean;
  sizeBytes: number;
  kind: "pdf" | "image" | "unknown";
}

/* ------------------------------------------------------------- 命令封装 */

export const runTask = (
  request: TaskRequest,
  outDir?: string | null,
  password?: string | null,
  notify?: boolean | null,
) =>
  invoke<TaskRecord>("run_task", {
    request,
    outDir: outDir ?? null,
    password: password ?? null,
    notify: notify ?? null,
  });

export const cancelTask = (taskId: string) =>
  invoke<boolean>("cancel_task", { taskId });

export const listTasks = () => invoke<TaskRecord[]>("list_tasks");

export const clearTasks = () => invoke<number>("clear_tasks");

export const clearTemp = () => invoke<CleanupReport>("clear_temp");

export const probeDocument = (
  path: string,
  password?: string | null,
  includePages = true,
) =>
  invoke<DocInfo>("probe_document", {
    path,
    password: password ?? null,
    includePages,
  });

export const renderThumbnails = (
  path: string,
  password?: string | null,
  pages?: number[] | null,
  maxPx?: number | null,
) =>
  invoke<ThumbOut[]>("render_thumbnails", {
    path,
    password: password ?? null,
    pages: pages ?? null,
    maxPx: maxPx ?? null,
  });

export const clearThumbCache = () => invoke<number>("clear_thumb_cache");

export const pickFiles = (args: PickFilesArgs = {}) =>
  invoke<PickedFiles>("pick_files", { args });

export const pickDir = (title?: string | null) =>
  invoke<PickedFiles>("pick_dir", { title: title ?? null });

export const pickSave = (
  defaultName?: string | null,
  directory?: string | null,
) =>
  invoke<PickedFiles>("pick_save", {
    defaultName: defaultName ?? null,
    directory: directory ?? null,
  });

export const reveal = (path: string) => invoke<void>("reveal", { path });

/** 用系统默认应用打开文件；目录则在访达中打开。 */
export const openPath = (path: string) => invoke<void>("open_path", { path });

export const classifyPath = (path: string) =>
  invoke<"pdf" | "image" | "unknown">("classify_path", { path });

export const ensureDir = (path: string) => invoke<string>("ensure_dir", { path });

/** 批量取路径元信息；不存在的条目会返回 `exists: false` 而不是报错。 */
export const statPaths = (paths: string[]) =>
  invoke<PathStat[]>("stat_paths", { paths });

export const appInfo = () => invoke<AppInfo>("app_info");

export const pdfiumStatus = () => invoke<EngineStatus>("pdfium_status");

export const reloadPdfium = () => invoke<EngineStatus>("reload_pdfium");

/* -------------------------------------------------------------- 授权配额 */

export const licenseStatus = () => invoke<LicenseStatus>("license_status");

export const activateLicense = (code: string) =>
  invoke<LicenseStatus>("activate_license", { code });

export const deactivateLicense = () => invoke<LicenseStatus>("deactivate_license");

export const quotaStatus = () => invoke<QuotaStatus>("quota_status");

/** 生成静默上报 payload；未激活返回 null。纯本地计算，不联网。 */
export const vipReportPayload = () => invoke<VipReportPayload | null>("vip_report_payload");

/* -------------------------------------------------------------- 事件名 */

export const EVENT = {
  progress: "task:progress",
  log: "task:log",
  finished: "task:finished",
  failed: "task:failed",
} as const;
