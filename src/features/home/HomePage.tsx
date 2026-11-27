import { ArrowRight, Crown, Infinity as InfinityIcon, Laptop, Search, ShieldCheck, Sparkles, X } from "lucide-react";
import { Link } from "react-router-dom";
import { useEffect, useMemo, useRef, useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { EmptyState } from "@/components/EmptyState";
import { Notice } from "@/components/Notice";
import { AppIcon } from "@/components/AppIcon";
import { formatRelativeTime } from "@/lib/format";
import { CATEGORIES, FEATURES, featuresByCategory, featureById } from "@/features/registry";
import type { FeatureDefinition, FeatureId } from "@/features/types";
import { useApp } from "@/store/app";
import { useQuota } from "@/store/quota";
import { useTasks } from "@/store/tasks";

/** 首页只展示最近若干条任务，完整记录在任务中心。 */
const RECENT_LIMIT = 5;

/** 常见办公口语搜索词，帮助普通用户用大白话秒搜功能。 */
const SEARCH_KEYWORDS: Partial<Record<FeatureId, string>> = {
  merge: "合并 拼接 合在一起 多份 照片 merge combine",
  invoice: "发票 报销 打印 a4 a5 拼版 invoice",
  idcard: "身份证 证件 护照 银行卡 复印 正反面 水印 id card",
  organize: "整理 删页 删除 排序 调顺序 重排 旋转 organize reorder delete",
  split: "拆分 分割 提取 拆开 单页 split extract",
  rotate: "旋转 歪了 倒了 方向 顺时针 逆时针 rotate",
  rmblank: "空白页 去白页 删除空白 扫描件 blank",
  watermark: "水印 防盗 合同 样本 文字 watermark",
  compress: "压缩 变小 瘦身 体积 邮件 上传 compress shrink",
  pdf2img: "转图片 长图 拼长图 微信 截图 png jpg webp image long",
  extractimg: "提取图片 抠图 原图 导出图片 extract",
  img2pdf: "图片转 照片 苹果 heic heif 拍照 漂白 黑白 扫描 photo scan",
  wechat: "微信 公众号 文章 链接 网页 wechat article",
};

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
  const info = useApp((state) => state.info);
  const engineReady = useApp((state) => state.engineReady);
  const vip = useQuota((state) => state.license?.active ?? false);
  const tasks = useTasks((state) => state.tasks);
  const setDrawerOpen = useTasks((state) => state.setDrawerOpen);

  const [query, setQuery] = useState("");
  const searchRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        searchRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const normalizedQuery = query.trim().toLowerCase();
  const filteredByCategory = useMemo(() => {
    return CATEGORIES.map((category) => {
      const list = featuresByCategory(category.id).filter((feature) => {
        if (!normalizedQuery) return true;
        const haystack = [
          t(feature.titleKey),
          t(feature.descKey),
          feature.id,
          SEARCH_KEYWORDS[feature.id] ?? "",
        ]
          .join(" ")
          .toLowerCase();
        return haystack.includes(normalizedQuery);
      });
      return { category, features: list };
    }).filter((group) => group.features.length > 0);
  }, [normalizedQuery, t]);

  const recent = tasks.slice(0, RECENT_LIMIT);

  return (
    <div className="flex flex-col gap-8">
      <header className="flex items-center gap-4">
        <AppIcon size={48} />
        <div className="min-w-0">
          <h1 className="text-xl font-semibold tracking-tight text-ink">{t("home.greeting")}</h1>
          <p className="mt-0.5 text-sm text-ink-muted">{t("home.subtitle")}</p>
          <div className="mt-2 flex flex-wrap gap-2">
            {vip ? (
              <span className="chip border-amber-300/60 bg-amber-50 text-amber-700 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300">
                <Crown size={13} />
                {t("activate.navVip")}
              </span>
            ) : null}
            {ADVANTAGES.map((advantage) => (
              <span key={advantage.labelKey} className="chip">
                <advantage.icon size={13} className="text-brand-600" />
                {t(advantage.labelKey)}
              </span>
            ))}
          </div>
        </div>
      </header>

      {info !== null && !engineReady ? <Notice tone="warn">{t("home.engineWarning")}</Notice> : null}

      <div className="relative">
        <Search
          size={15}
          className="pointer-events-none absolute left-3.5 top-1/2 -translate-y-1/2 text-ink-faint"
        />
        <input
          ref={searchRef}
          type="text"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t("home.searchPlaceholder")}
          className="field pl-9 pr-9 text-sm"
        />
        {query.length > 0 ? (
          <button
            type="button"
            onClick={() => setQuery("")}
            className="btn-quiet absolute right-1.5 top-1/2 h-7 w-7 -translate-y-1/2 rounded-lg px-0 py-0"
            aria-label={t("common.clear")}
          >
            <X size={14} />
          </button>
        ) : null}
      </div>

      <div className="flex flex-col gap-6">
        {filteredByCategory.length === 0 ? (
          <div className="card">
            <EmptyState
              icon={Search}
              title={t("home.searchEmpty")}
              hint={t("home.searchEmptyHint")}
            />
          </div>
        ) : (
          filteredByCategory.map(({ category, features }) => (
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
          ))
        )}
      </div>

      {!vip ? (
        <Link
          to="/activate"
          className="card flex items-center gap-4 border-amber-300/60 bg-gradient-to-br from-amber-50 to-orange-50 p-4 transition-all duration-150 hover:-translate-y-0.5 hover:shadow-card-hover dark:border-amber-500/25 dark:from-amber-500/10 dark:to-orange-500/10"
        >
          <span className="inline-flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl bg-gradient-to-br from-amber-400 to-orange-500 text-white">
            <Crown size={20} />
          </span>
          <span className="min-w-0 flex-1">
            <span className="block truncate text-sm font-medium text-ink">
              {t("features.activate.title")}
            </span>
            <span className="mt-0.5 block text-xs leading-relaxed text-ink-muted">
              {t("features.activate.desc")}
            </span>
          </span>
          <ArrowRight size={16} className="shrink-0 text-amber-600 dark:text-amber-400" />
        </Link>
      ) : null}

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
