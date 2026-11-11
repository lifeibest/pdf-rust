import {
  BookOpen,
  Check,
  Copy,
  Download,
  Globe,
  Heart,
  Loader2,
  RefreshCw,
  Sparkles,
  X,
} from "lucide-react";
import { useState, type ReactElement, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";

import { AppIcon } from "@/components/AppIcon";
import { Notice } from "@/components/Notice";
import { Section } from "@/components/Section";
import { checkForUpdate, type UpdateCheckResult } from "@/lib/update";
import { useApp } from "@/store/app";
import { LICENSE_GROUPS, licensesByGroup } from "./licenses";
// 公众号二维码图片（src/assets/gongzhonghao.jpg）。
import wechatQrcode from "@/assets/gongzhonghao.jpg";

/** 官网与 API 文档链接。 */
const WEBSITE_URL = "https://www.pdfai.cn";
const API_DOCS_URL = "https://www.pdfai.cn/docs";

/** 更新日志：日期语言中立，描述走 i18n；新版本往数组头部加。 */
const CHANGELOG = [
  { date: "2026.11.12", textKey: "about.cl20261112c" },
  { date: "2026.11.12", textKey: "about.cl20261112b" },
  { date: "2026.11.12", textKey: "about.cl20261112" },
  { date: "2026.11.11", textKey: "about.cl20261111" },
  { date: "2026.11.10", textKey: "about.cl20261110" },
  { date: "2026.11.08", textKey: "about.cl20261108" },
  { date: "2026.11.01", textKey: "about.cl202611" },
] as const;

/** 「检测更新」的交互状态。 */
type UpdateStatus = "idle" | "checking" | "uptodate" | "available" | "error";

/**
 * 关于页。
 *
 * 展示版号、构建信息、更新日志、开源许可与隐私说明，并提供手动「检测更新」。
 * PDFium 已随安装包内嵌，正常无需用户干预，故不再在此页暴露引擎诊断入口。
 */
export default function AboutPage(): ReactElement {
  const { t, i18n } = useTranslation();
  const info = useApp((state) => state.info);
  const error = useApp((state) => state.error);

  const unknown = t("common.unknown");

  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>("idle");
  const [updateResult, setUpdateResult] = useState<UpdateCheckResult | null>(null);

  // 隐私优先：只有用户主动点击才联网拉取更新清单，无任何自动/后台检测。
  async function handleCheckUpdate(): Promise<void> {
    if (!info) return;
    setUpdateStatus("checking");
    try {
      const result = await checkForUpdate(info.version, info.os, i18n.language);
      setUpdateResult(result);
      setUpdateStatus(result.hasUpdate ? "available" : "uptodate");
    } catch {
      setUpdateResult(null);
      setUpdateStatus("error");
    }
  }

  return (
    <div className="flex flex-col gap-5">
      <header className="card flex items-center gap-4 p-5">
        <AppIcon size={56} />
        <div className="min-w-0 flex-1">
          <h1 className="text-lg font-semibold text-ink">{info?.name ?? t("app.name")}</h1>
          <p className="truncate text-xs text-ink-muted">{t("app.tagline")}</p>
          {/* 官网 / API 文档直接放进头卡，不再单独开一个「链接」模块 */}
          <div className="mt-2.5 flex flex-wrap gap-2">
            <LinkButton href={WEBSITE_URL} icon={<Globe size={15} />}>
              {t("about.website")}
            </LinkButton>
            <LinkButton href={API_DOCS_URL} icon={<BookOpen size={15} />}>
              {t("about.apiDocs")}
            </LinkButton>
          </div>
        </div>
        <div className="flex shrink-0 flex-col items-end gap-2">
          <span className="chip">
            {t("about.version")} {info?.version ?? unknown}
          </span>
          <button
            type="button"
            className="btn-quiet h-7 px-2 py-0 text-xs"
            disabled={updateStatus === "checking" || !info}
            onClick={() => void handleCheckUpdate()}
          >
            {updateStatus === "checking" ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <RefreshCw size={13} />
            )}
            {t(updateStatus === "checking" ? "about.checkingUpdate" : "about.checkUpdate")}
          </button>
        </div>
      </header>

      {error ? <Notice tone="error">{error}</Notice> : null}

      {updateStatus === "error" ? (
        <Notice tone="error" onDismiss={() => setUpdateStatus("idle")}>
          {t("about.updateCheckFailed")}
        </Notice>
      ) : null}

      {updateStatus === "uptodate" ? (
        <Notice tone="success" onDismiss={() => setUpdateStatus("idle")}>
          {t("about.updateUpToDate")}
        </Notice>
      ) : null}

      {updateStatus === "available" && updateResult ? (
        <UpdateAvailableCard result={updateResult} onDismiss={() => setUpdateStatus("idle")} />
      ) : null}

      <div className="grid items-start gap-5 lg:grid-cols-2">
        <div className="flex min-w-0 flex-col gap-5">
          <Section title={t("about.wechat")}>
            <div className="flex flex-col items-center gap-2 py-1">
              <img
                src={wechatQrcode}
                alt={t("about.wechatHint")}
                className="h-40 w-40 rounded-xl border border-line bg-white object-contain p-2"
              />
              <span className="text-xs text-ink-muted">{t("about.wechatHint")}</span>
            </div>
          </Section>

          <Section title={t("about.privacy")}>
            <p className="flex items-start gap-2 text-xs leading-relaxed text-ink-muted">
              <Heart size={14} className="mt-0.5 shrink-0 text-rose-400" />
              {t("about.privacyText")}
            </p>
          </Section>
        </div>

        <div className="flex min-w-0 flex-col gap-5">
          <Section title={t("about.changelog")} collapsible defaultOpen={false}>
            <ul className="flex flex-col gap-3">
              {CHANGELOG.map((entry) => (
                <li key={entry.date} className="flex items-baseline gap-3 text-xs">
                  <span className="shrink-0 font-mono text-[11px] text-brand-600 dark:text-brand-400">
                    {entry.date}
                  </span>
                  <span className="min-w-0 flex-1 leading-relaxed text-ink-muted">
                    {t(entry.textKey)}
                  </span>
                </li>
              ))}
            </ul>
          </Section>

          <Section title={t("about.buildInfo")} collapsible defaultOpen={false}>
            <dl className="flex flex-col gap-3">
              <InfoRow label={t("about.identifier")} value={info?.identifier ?? unknown} copyable />
              <InfoRow label={t("about.tauriVersion")} value={info?.tauriVersion ?? unknown} />
              <InfoRow label={t("about.os")} value={info?.os ?? unknown} />
              <InfoRow label={t("about.arch")} value={info?.arch ?? unknown} />
            </dl>
          </Section>

          <Section
            title={t("about.licenses")}
            hint={t("about.licenseHint")}
            collapsible
            defaultOpen={false}
          >
            <div className="flex flex-col gap-4">
              {LICENSE_GROUPS.map((group) => (
                <div key={group.id} className="flex flex-col gap-1.5">
                  <span className="label mb-0">{t(group.titleKey)}</span>
                  <ul className="flex flex-col gap-1">
                    {licensesByGroup(group.id).map((entry) => (
                      <li key={entry.name} className="flex items-baseline gap-2 text-xs">
                        <button
                          type="button"
                          className="shrink-0 text-brand-600 underline-offset-2 hover:underline dark:text-brand-400"
                          onClick={() => void openUrl(entry.url)}
                        >
                          {entry.name}
                        </button>
                        <span className="shrink-0 font-mono text-[11px] text-ink-faint">
                          {entry.version}
                        </span>
                        <span className="min-w-0 flex-1 truncate text-right text-[11px] text-ink-muted">
                          {entry.license}
                        </span>
                      </li>
                    ))}
                  </ul>
                </div>
              ))}
            </div>
          </Section>

        </div>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------- 版面零件 */

interface InfoRowProps {
  label: string;
  value: string;
  copyable?: boolean;
}

/** 「标签左、值右」的信息行，值过长时截断并把完整路径放进 title。 */
function InfoRow({ label, value, copyable = false }: InfoRowProps) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  async function handleCopy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // 剪贴板不可用（极少数环境）时静默失败，值本身已经显示在界面上
    }
  }

  return (
    <div className="flex items-center gap-3">
      <dt className="w-28 shrink-0 text-xs text-ink-faint">{label}</dt>
      <dd className="min-w-0 flex-1 truncate font-mono text-xs text-ink-muted" title={value}>
        {value}
      </dd>
      {copyable ? (
        <button
          type="button"
          className="btn-quiet h-7 w-7 shrink-0 rounded-lg px-0 py-0"
          onClick={() => void handleCopy()}
          title={t(copied ? "common.copied" : "common.copy")}
          aria-label={t("common.copy")}
        >
          {copied ? <Check size={13} className="text-emerald-500" /> : <Copy size={13} />}
        </button>
      ) : null}
    </div>
  );
}

interface LinkButtonProps {
  href: string;
  icon: ReactNode;
  children: ReactNode;
}

/** 外部链接按钮：统一走 opener 插件，交给系统浏览器处理。 */
function LinkButton({ href, icon, children }: LinkButtonProps): ReactElement {
  return (
    <button
      type="button"
      className="btn-ghost hover:border-brand-400 hover:text-brand-600"
      onClick={() => void openUrl(href)}
    >
      {icon}
      {children}
    </button>
  );
}

interface UpdateAvailableCardProps {
  result: UpdateCheckResult;
  onDismiss: () => void;
}

/** 「发现新版本」卡片：版本跃迁 + 更新要点 + 下载/查看完整说明入口。 */
function UpdateAvailableCard({ result, onDismiss }: UpdateAvailableCardProps): ReactElement {
  const { t } = useTranslation();
  const downloadUrl = result.download?.url;
  const pageUrl = result.releasePageUrl;

  return (
    <div className="card flex flex-col gap-3 border-brand-400/40 p-4">
      <div className="flex items-center gap-3">
        <span className="inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-brand-500 to-violet-500 text-white">
          <Sparkles size={17} />
        </span>
        <div className="min-w-0 flex-1">
          <p className="flex flex-wrap items-center gap-2 text-sm font-medium text-ink">
            {t("about.updateAvailable")}
            {result.mandatory ? (
              <span className="rounded-full bg-rose-500/15 px-2 py-0.5 text-[11px] font-medium text-rose-500">
                {t("about.updateMandatory")}
              </span>
            ) : null}
          </p>
          <p className="mt-0.5 text-xs text-ink-muted">
            {result.currentVersion} → {result.latestVersion}
            {result.releaseDate
              ? ` · ${t("about.updateReleaseDate", { date: result.releaseDate })}`
              : ""}
          </p>
        </div>
        {result.mandatory ? null : (
          <button
            type="button"
            className="btn-quiet h-7 w-7 shrink-0 rounded-lg px-0 py-0"
            onClick={onDismiss}
            aria-label={t("common.close")}
            title={t("common.close")}
          >
            <X size={15} />
          </button>
        )}
      </div>

      {result.notes.length > 0 ? (
        <ul className="flex flex-col gap-1">
          {result.notes.map((note) => (
            <li key={note} className="flex gap-2 text-xs leading-relaxed text-ink-muted">
              <span className="mt-1 shrink-0 text-brand-500">•</span>
              <span className="min-w-0">{note}</span>
            </li>
          ))}
        </ul>
      ) : null}

      <div className="flex flex-wrap gap-2">
        {downloadUrl ? (
          <button type="button" className="btn-primary" onClick={() => void openUrl(downloadUrl)}>
            <Download size={15} />
            {t("about.updateDownload")}
          </button>
        ) : null}
        {pageUrl ? (
          <button type="button" className="btn-ghost" onClick={() => void openUrl(pageUrl)}>
            <Globe size={15} />
            {t("about.updateViewAll")}
          </button>
        ) : null}
      </div>
    </div>
  );
}
