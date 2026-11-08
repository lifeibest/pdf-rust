import { ArrowRight, Infinity as InfinityIcon, Laptop, ShieldCheck, Sparkles } from "lucide-react";
import { Link } from "react-router-dom";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { EmptyState } from "@/components/EmptyState";
import { Notice } from "@/components/Notice";
import { AppIcon } from "@/components/AppIcon";
import { formatRelativeTime } from "@/lib/format";
import { CATEGORIES, FEATURES, featuresByCategory, featureById } from "@/features/registry";
import type { FeatureDefinition } from "@/features/types";
import { useApp } from "@/store/app";
import { useTasks } from "@/store/tasks";

/** 首页只展示最近若干条任务，完整记录在任务中心。 */
const RECENT_LIMIT = 5;

/** 顶部优势标签：突出本地、隐私、无大小限制三大卖点。 */
const ADVANTAGES = [
  { icon: ShieldCheck, labelKey: "home.advPrivacy" },
  { icon: InfinityIcon, labelKey: "home.advUnlimited" },
  { icon: Laptop, labelKey: "home.advLocal" },
] as const;

/**
 * 首页。
 *
 * 卡片与分组同样由注册表生成；「最近任务」直接复用任务 store，
 * 因此这里没有任何自己的状态。
 */
export default function HomePage(): ReactElement {
  const { t } = useTranslation();
  const engineReady = useApp((state) => state.engineReady);
  const tasks = useTasks((state) => state.tasks);
  const setDrawerOpen = useTasks((state) => state.setDrawerOpen);

  const recent = tasks.slice(0, RECENT_LIMIT);

  return (
    <div className="flex flex-col gap-8">
      <header className="flex items-center gap-4">
        <AppIcon size={48} />
        <div className="min-w-0">
          <h1 className="text-xl font-semibold tracking-tight text-ink">{t("home.greeting")}</h1>
          <p className="mt-0.5 text-sm text-ink-muted">{t("home.subtitle")}</p>
          <div className="mt-2 flex flex-wrap gap-2">
            {ADVANTAGES.map((advantage) => (
              <span key={advantage.labelKey} className="chip">
                <advantage.icon size={13} className="text-brand-600" />
                {t(advantage.labelKey)}
              </span>
            ))}
          </div>
        </div>
      </header>

      {!engineReady ? <Notice tone="warn">{t("home.engineWarning")}</Notice> : null}

      <div className="flex flex-col gap-6">
        {CATEGORIES.map((category) => {
          const features = featuresByCategory(category.id);
          if (features.length === 0) return null;
          return (
            <section key={category.id} className="flex flex-col gap-3">
              <h2 className="text-xs font-semibold uppercase tracking-wider text-ink-faint">
                {t(category.titleKey)}
              </h2>
              <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
                {features.map((feature) => (
                  <FeatureCard key={feature.id} feature={feature} />
                ))}
              </div>
            </section>
          );
        })}
      </div>

      <section className="flex flex-col gap-3">
        <div className="flex items-center gap-2">
          <h2 className="flex-1 text-xs font-semibold uppercase tracking-wider text-ink-faint">
            {t("tasks.recent")}
          </h2>
          {tasks.length > 0 ? (
            <button
              type="button"
              className="btn-quiet h-7 px-2 py-0 text-xs"
              onClick={() => setDrawerOpen(true)}
            >
              {t("nav.taskCenter")}
              <ArrowRight size={13} />
            </button>
          ) : null}
        </div>

        {recent.length === 0 ? (
          <div className="card">
            <EmptyState icon={Sparkles} title={t("tasks.empty")} hint={t("tasks.emptyHint")} />
          </div>
        ) : (
          <ul className="card divide-y divide-line">
            {recent.map((task) => {
              const definition = featureById(task.feature);
              const title = definition ? t(definition.titleKey) : task.feature;
              return (
                <li key={task.id}>
                  <button
                    type="button"
                    className="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-surface-subtle"
                    onClick={() => setDrawerOpen(true)}
                  >
                    <span
                      className={`inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br text-white ${
                        definition?.accent ?? "from-brand-500 to-violet-500"
                      }`}
                    >
                      {definition ? <definition.icon size={15} /> : <Sparkles size={15} />}
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm font-medium text-ink">{title}</span>
                      <span className="block truncate text-xs text-ink-faint">
                        {t(`tasks.${task.status}`)}
                        {task.outputs.length > 0
                          ? ` · ${t("feature.runSuccess", { count: task.outputs.length })}`
                          : ""}
                      </span>
                    </span>
                    <span className="shrink-0 text-xs text-ink-faint">
                      {formatRelativeTime(task.startedAt, t)}
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </section>

      <p className="pb-2 text-center text-xs text-ink-faint">
        {t("drop.globalHint")} · {FEATURES.length} {t("common.items")}
      </p>
    </div>
  );
}

/** 单个功能卡片。 */
function FeatureCard({ feature }: { feature: FeatureDefinition }): ReactElement {
  const { t } = useTranslation();
  const Icon = feature.icon;

  return (
    <Link
      to={feature.route}
      className="card flex items-start gap-3 p-4 transition-all duration-150 hover:-translate-y-0.5 hover:shadow-card-hover"
    >
      <span
        className={`inline-flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl bg-gradient-to-br text-white ${feature.accent}`}
      >
        <Icon size={20} />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-sm font-medium text-ink">{t(feature.titleKey)}</span>
        <span className="mt-0.5 block text-xs leading-relaxed text-ink-muted">
          {t(feature.descKey)}
        </span>
      </span>
    </Link>
  );
}
