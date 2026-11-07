import clsx from "clsx";
import { Home } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { NavLink } from "react-router-dom";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { AppIcon } from "@/components/AppIcon";
import { CATEGORIES, SYSTEM_PAGES, featuresByCategory } from "@/features/registry";

/**
 * 左侧固定侧边栏。
 *
 * 分组、顺序、图标、文案全部来自 `features/registry.ts`，
 * 因此新增功能时这里会自动多出一项，不需要改布局代码。
 */
export function Sidebar(): ReactElement {
  const { t } = useTranslation();

  return (
    <nav className="flex h-full w-60 shrink-0 flex-col border-r border-line bg-surface">
      {/* 顶部留白用来避开 macOS 的红绿灯按钮，同时充当窗口拖拽区 */}
      <div className="drag-region flex h-14 shrink-0 items-end gap-2.5 px-4 pb-2">
        <AppIcon size={26} className="no-drag rounded-lg" />
        <span className="no-drag truncate text-sm font-semibold tracking-tight text-ink">
          {t("app.name")}
        </span>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-3 py-3">
        <SidebarLink to="/" icon={Home} label={t("nav.home")} end />

        {CATEGORIES.map((category) => {
          const features = featuresByCategory(category.id);
          if (features.length === 0) return null;
          return (
            <div key={category.id} className="mt-5 flex flex-col gap-1">
              <p className="px-2 pb-1 text-[11px] font-semibold uppercase tracking-wider text-ink-faint">
                {t(category.titleKey)}
              </p>
              {features.map((feature) => (
                <SidebarLink
                  key={feature.id}
                  to={feature.route}
                  icon={feature.icon}
                  label={t(feature.titleKey)}
                  accent={feature.accent}
                />
              ))}
            </div>
          );
        })}
      </div>

      <div className="shrink-0 border-t border-line px-3 py-3">
        {SYSTEM_PAGES.map((page) => (
          <SidebarLink key={page.id} to={page.route} icon={page.icon} label={t(page.titleKey)} />
        ))}
      </div>
    </nav>
  );
}

interface SidebarLinkProps {
  to: string;
  icon: LucideIcon;
  label: string;
  /** 功能项的渐变色，选中时用于图标底色 */
  accent?: string;
  end?: boolean;
}

function SidebarLink({ to, icon: Icon, label, accent, end = false }: SidebarLinkProps): ReactElement {
  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) =>
        clsx(
          "group flex items-center gap-2.5 rounded-xl px-2.5 py-2 text-sm transition-colors",
          isActive
            ? "bg-surface-subtle font-medium text-ink"
            : "text-ink-muted hover:bg-surface-subtle/60 hover:text-ink",
        )
      }
    >
      {({ isActive }) => (
        <>
          <span
            className={clsx(
              "inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-lg",
              isActive && accent
                ? `bg-gradient-to-br text-white ${accent}`
                : "text-ink-faint group-hover:text-ink-muted",
            )}
          >
            <Icon size={15} />
          </span>
          <span className="truncate">{label}</span>
        </>
      )}
    </NavLink>
  );
}
