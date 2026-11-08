import {
  BookOpen,
  Check,
  Copy,
  FolderSearch,
  Globe,
  Heart,
  Loader2,
  RefreshCw,
} from "lucide-react";
import { useState, type ReactElement, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";

import { AppIcon } from "@/components/AppIcon";
import { Notice } from "@/components/Notice";
import { Section } from "@/components/Section";
import { shortenPath } from "@/lib/paths";
import { reveal } from "@/lib/tauri";
import { useApp } from "@/store/app";
import { LICENSE_GROUPS, licensesByGroup } from "./licenses";
// 公众号二维码图片（src/assets/gongzhonghao.jpg）。
import wechatQrcode from "@/assets/gongzhonghao.jpg";

/** 官网与 API 文档链接。 */
const WEBSITE_URL = "https://www.pdfai.cn";
const API_DOCS_URL = "https://www.pdfai.cn/docs";

/**
 * 关于页。
 *
 * 除了常规的版号与许可清单，这里承担一个实际职责：PDFium 是运行时动态加载的，
 * 缺失时应用其余部分仍可用，因此把「搜索过哪些路径、怎么补上、补完点重新检测」
 * 全部摊在这一页，用户不需要看日志也不需要重启。
 */
export default function AboutPage(): ReactElement {
  const { t } = useTranslation();
  const info = useApp((state) => state.info);
  const loading = useApp((state) => state.loading);
  const error = useApp((state) => state.error);
  const engineReady = useApp((state) => state.engineReady);
  const recheckEngine = useApp((state) => state.recheckEngine);

  const pdfium = info?.pdfium ?? null;
  const unknown = t("common.unknown");

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
        <span className="chip shrink-0">
          {t("about.version")} {info?.version ?? unknown}
        </span>
      </header>

      {error ? <Notice tone="error">{error}</Notice> : null}

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

          <Section title={t("about.paths")}>
            <dl className="flex flex-col gap-3">
              <InfoRow
                label={t("about.resourceDir")}
                value={info?.resourceDir ?? unknown}
                revealable={Boolean(info?.resourceDir)}
              />
              <InfoRow
                label={t("about.cacheDir")}
                value={info?.cacheDir ?? unknown}
                revealable={Boolean(info?.cacheDir)}
              />
              <InfoRow
                label={t("about.configDir")}
                value={info?.configDir ?? unknown}
                revealable={Boolean(info?.configDir)}
              />
              <InfoRow
                label={t("about.defaultOutputDir")}
                value={info?.defaultOutputDir ?? unknown}
                revealable={Boolean(info?.defaultOutputDir)}
              />
            </dl>
          </Section>

          <Section title={t("about.privacy")}>
            <p className="flex items-start gap-2 text-xs leading-relaxed text-ink-muted">
              <Heart size={14} className="mt-0.5 shrink-0 text-rose-400" />
              {t("about.privacyText")}
            </p>
          </Section>
        </div>

        <div className="flex min-w-0 flex-col gap-5">
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

          <Section
            title={t("about.engine")}
            collapsible
            defaultOpen={false}
            action={
              <button
                type="button"
                className="btn-quiet h-7 px-2 py-0 text-xs"
                disabled={loading}
                onClick={() => void recheckEngine()}
              >
                {loading ? (
                  <Loader2 size={13} className="animate-spin" />
                ) : (
                  <RefreshCw size={13} />
                )}
                {t("about.engineReload")}
              </button>
            }
          >
            <div className="flex flex-col gap-3">
              <Notice tone={engineReady ? "success" : "error"}>
                {t(engineReady ? "about.engineReady" : "about.engineMissing")}
              </Notice>

              <dl className="flex flex-col gap-3">
                <InfoRow label={t("about.engineTag")} value={pdfium?.expectedTag ?? unknown} />
                <InfoRow label={t("about.engineSource")} value={pdfium?.source ?? unknown} />
                <InfoRow
                  label={t("about.enginePath")}
                  value={pdfium?.libraryPath ?? unknown}
                  copyable={Boolean(pdfium?.libraryPath)}
                />
              </dl>

              {pdfium?.error ? <Notice tone="warn">{pdfium.error}</Notice> : null}

              {pdfium && pdfium.searched.length > 0 ? (
                <div className="flex flex-col gap-1.5">
                  <span className="label mb-0">{t("about.engineSearched")}</span>
                  <ul className="flex flex-col gap-1">
                    {pdfium.searched.map((path) => (
                      <li
                        key={path}
                        className="truncate rounded-lg bg-surface-subtle px-2 py-1 font-mono text-[11px] text-ink-muted"
                        title={path}
                      >
                        {shortenPath(path, 48)}
                      </li>
                    ))}
                  </ul>
                  <p className="text-[11px] leading-relaxed text-ink-faint">
                    {t("about.engineHint")}
                  </p>
                </div>
              ) : null}
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
  revealable?: boolean;
}

/** 「标签左、值右」的信息行，值过长时截断并把完整路径放进 title。 */
function InfoRow({ label, value, copyable = false, revealable = false }: InfoRowProps) {
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
      {revealable ? (
        <button
          type="button"
          className="btn-quiet h-7 w-7 shrink-0 rounded-lg px-0 py-0"
          onClick={() => void reveal(value).catch(() => null)}
          title={t("common.openDir")}
          aria-label={t("common.openDir")}
        >
          <FolderSearch size={13} />
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
