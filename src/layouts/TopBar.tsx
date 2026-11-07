import clsx from "clsx";
import { ChevronRight, ListChecks } from "lucide-react";
import { Link, useLocation } from "react-router-dom";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { featureById, systemPageById } from "@/features/registry";
import { useTasks } from "@/store/tasks";
import type { Translate } from "@/i18n";

/** 内容区顶栏：面包屑 + 任务中心入口。整条都可拖动窗口。 */
export function TopBar(): ReactElement {
  const { t } = useTranslation();
  const location = useLocation();
  const running = useTasks((state) => state.tasks.filter((task) => task.status === "running").length);
  const drawerOpen = useTasks((state) => state.drawerOpen);
  const setDrawerOpen = useTasks((state) => state.setDrawerOpen);

  const current = titleOf(location.pathname, t);

  return (
    <header className="drag-region flex h-14 shrink-0 items-center gap-3 border-b border-line bg-surface px-5">
      <nav className="no-drag flex min-w-0 items-center gap-1.5 text-sm" aria-label="breadcrumb">
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

      <div className="flex-1" />

      <button
        type="button"
        onClick={() => setDrawerOpen(!drawerOpen)}
        className={clsx(
          "no-drag btn-quiet relative h-9 rounded-xl px-3",
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
function titleOf(pathname: string, t: Translate): string {
  const id = pathname.replace(/^\/+/, "").split("/")[0];
  if (!id) return "";
  const feature = featureById(id);
  if (feature) return t(feature.titleKey);
  const page = systemPageById(id);
  if (page) return t(page.titleKey);
  return "";
}
