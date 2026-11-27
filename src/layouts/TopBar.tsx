import clsx from "clsx";
import { ChevronRight, Crown, ListChecks, Moon, Sun } from "lucide-react";
import { Link, useLocation } from "react-router-dom";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { featureById, systemPageById } from "@/features/registry";
import { useSettings } from "@/store/settings";
import { useTasks } from "@/store/tasks";
import { useQuota } from "@/store/quota";
import type { Translate } from "@/i18n";

/** 内容区顶栏：面包屑 + 今日配额 + 主题切换 + 任务中心入口。整条都可拖动窗口。 */
export function TopBar(): ReactElement {
  const { t } = useTranslation();
  const location = useLocation();
  const running = useTasks((state) => state.tasks.filter((task) => task.status === "running").length);
  const drawerOpen = useTasks((state) => state.drawerOpen);
  const setDrawerOpen = useTasks((state) => state.setDrawerOpen);
  const vip = useQuota((state) => state.license?.active ?? false);
  const quota = useQuota((state) => state.quota);
  const theme = useSettings((state) => state.settings.theme);
  const patchSettings = useSettings((state) => state.patch);

  const isDark =
    theme === "dark" ||
    (theme === "system" &&
      typeof window !== "undefined" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches);

  const toggleTheme = () => {
    patchSettings({ theme: isDark ? "light" : "dark" });
  };

  const current = titleOf(location.pathname, t, vip);

  return (
    <header
      data-tauri-drag-region
      className="flex h-14 shrink-0 items-center gap-2 border-b border-line bg-surface px-5"
    >
      <nav className="flex min-w-0 items-center gap-1.5 text-sm" aria-label="breadcrumb">
        <Link to="/" className="shrink-0 text-ink-faint transition-colors hover:text-ink">
          {t("nav.home")}
        </Link>
        {current ? (
          <>
            <ChevronRight size={14} className="shrink-0 text-ink-faint" />
            <span className="truncate font-medium text-ink">{current}</span>
          </>
        ) : null}
      </nav>

      {/* 中间的空白弹性区也标为拖拽区：顶栏最容易被抓住的地方 */}
      <div className="flex-1" data-tauri-drag-region />

      {vip ? (
        <Link
          to="/activate"
          className="inline-flex h-8 items-center gap-1 rounded-full border border-amber-300/60 bg-amber-50 px-2.5 text-xs font-medium text-amber-700 transition-colors hover:bg-amber-100/80 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300"
        >
          <Crown size={13} />
          <span>{t("activate.navVip")}</span>
        </Link>
      ) : quota && quota.limit != null ? (
        <Link
          to="/activate"
          className="hidden h-8 items-center gap-1.5 rounded-full border border-line bg-surface-subtle px-2.5 text-xs text-ink-muted transition-colors hover:border-brand-300 hover:text-ink sm:inline-flex"
          title={t("features.activate.desc")}
        >
          <span>
            {t("nav.quotaLeft", {
              remaining: quota.remaining ?? 0,
              limit: quota.limit,
            })}
          </span>
        </Link>
      ) : null}

      <button
        type="button"
        onClick={toggleTheme}
        className="btn-quiet h-9 w-9 rounded-xl px-0 py-0"
        title={isDark ? t("settings.themeLight") : t("settings.themeDark")}
        aria-label={isDark ? t("settings.themeLight") : t("settings.themeDark")}
      >
        {isDark ? <Sun size={16} /> : <Moon size={16} />}
      </button>

      <button
        type="button"
        onClick={() => setDrawerOpen(!drawerOpen)}
        className={clsx(
          "btn-quiet relative h-9 rounded-xl px-3",
          drawerOpen && "bg-surface-subtle text-ink",
        )}
        title={t("nav.taskCenter")}
      >
        <ListChecks size={16} />
        <span className="hidden sm:inline">{t("nav.taskCenter")}</span>
        {running > 0 ? (
          <span className="absolute -right-0.5 -top-0.5 inline-flex h-4 min-w-[1rem] items-center justify-center rounded-full bg-brand-500 px-1 text-[10px] font-semibold text-white">
            {running}
          </span>
        ) : null}
      </button>
    </header>
  );
}

/** 依据路径推导面包屑末级标题；首页返回空串（不显示第二级）。 */
function titleOf(pathname: string, t: Translate, vip: boolean): string {
  const id = pathname.replace(/^\/+/, "").split("/")[0];
  if (!id) return "";
  const feature = featureById(id);
  if (feature) return t(feature.titleKey);
  const page = systemPageById(id);
  if (page) {
    // 已激活时该页面包屑也同步为「VIP 会员」。
    return page.id === "activate" && vip ? t("activate.navVip") : t(page.titleKey);
  }
  return "";
}
