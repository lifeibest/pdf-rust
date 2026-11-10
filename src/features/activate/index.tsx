import { BadgeCheck, Crown, KeyRound, QrCode, RefreshCw } from "lucide-react";
import { useEffect, useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { ActivationForm } from "@/components/ActivationForm";
import { Section } from "@/components/Section";
import { useQuota } from "@/store/quota";
// 公众号二维码图片（src/assets/gongzhonghao.jpg），与关于页共用。
import wechatQrcode from "@/assets/gongzhonghao.jpg";

/** 获取激活码的步骤（文案走 i18n）。 */
const STEPS = ["activate.step1", "activate.step2", "activate.step3"] as const;

/**
 * 「获取激活码 / VIP 会员」页。
 *
 * 按授权状态分流，避免已激活用户还看到一堆获取引导：
 * - 未激活：获取流程（关注公众号 → 付费 → 录入激活码）+ 今日免费额度。
 * - 已激活：VIP 会员卡 + 会员信息 + 今日用量，隐藏二维码 / 步骤 / 录入框，
 *   仅保留一个低调的「解除激活」入口。
 */
export default function ActivatePage(): ReactElement {
  const license = useQuota((state) => state.license);
  const refresh = useQuota((state) => state.refresh);

  // 进入页面即拉取一次授权与配额快照，避免首屏为空。
  useEffect(() => {
    void refresh();
  }, [refresh]);

  return license?.active ? <VipView /> : <FreeView />;
}

/* ------------------------------------------------------------------ VIP */

function VipView(): ReactElement {
  const { t } = useTranslation();
  const license = useQuota((state) => state.license);
  const quota = useQuota((state) => state.quota);
  const loading = useQuota((state) => state.loading);
  const refresh = useQuota((state) => state.refresh);
  const deactivate = useQuota((state) => state.deactivate);

  // 「重新激活」展开录入框（非破坏性）；「解除激活」需二次确认（破坏性）。
  const [reactivating, setReactivating] = useState(false);
  const [confirming, setConfirming] = useState(false);

  const plan = t(planKey(license?.plan ?? null));

  async function handleDeactivate(): Promise<void> {
    setConfirming(false);
    await deactivate();
  }

  return (
    <div className="flex flex-col gap-5">
      {/* VIP 会员卡：沿用品牌靛紫渐变，与首页卡片 / 功能图标同色系 */}
      <div className="relative overflow-hidden rounded-2xl border border-brand-400/40 bg-gradient-to-br from-brand-500 via-brand-600 to-violet-600 p-6 text-white shadow-lg dark:border-brand-500/25">
        <div className="pointer-events-none absolute -right-12 -top-12 h-44 w-44 rounded-full bg-white/15 blur-2xl" />
        <div className="relative flex items-start gap-4">
          <span className="inline-flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl bg-white/20 backdrop-blur-sm">
            <Crown size={28} />
          </span>
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <h1 className="text-xl font-bold tracking-tight">{t("activate.vipTitle")}</h1>
              <span className="rounded-full bg-white/25 px-2.5 py-0.5 text-[11px] font-semibold">
                {plan}
              </span>
            </div>
            <p className="mt-1.5 text-sm leading-relaxed text-white/90">
              {t("activate.vipSubtitle")}
            </p>
          </div>
        </div>
      </div>

      <div className="grid items-start gap-5 lg:grid-cols-2">
        <div className="flex min-w-0 flex-col gap-5">
          <Section title={t("activate.memberTitle")}>
            <div className="flex flex-col gap-2.5">
              <Row label={t("activate.memberPlan")} value={plan} />
              <Row
                label={t("activate.memberQuota")}
                value={
                  quota?.unlimited || license?.quotaUnlimited
                    ? t("activate.unlimited")
                    : String(quota?.limit ?? license?.quotaPerDay ?? 0)
                }
              />
              <Row
                label={t("activate.memberExp")}
                value={license?.exp != null ? formatExp(license.exp) : t("activate.permanent")}
              />
              {license?.licenseId ? (
                <Row label={t("activate.memberId")} value={license.licenseId} />
              ) : null}
            </div>
          </Section>

          <Section title={t("activate.manageTitle")}>
            <div className="flex flex-col gap-3">
              <p className="text-xs leading-relaxed text-ink-muted">{t("activate.manageHint")}</p>

              {/* 重新激活：展开录入框，粘贴新码即覆盖当前授权，不会丢失会员状态 */}
              {reactivating ? (
                <ActivationForm />
              ) : (
                <button
                  type="button"
                  className="btn-ghost self-start px-3 py-1 text-xs"
                  onClick={() => setReactivating(true)}
                >
                  <KeyRound size={13} />
                  {t("activate.reactivate")}
                </button>
              )}

              {/* 解除激活：破坏性操作，低调展示 + 二次确认，避免误点直接掉回免费版 */}
              <div className="border-t border-line pt-2.5">
                {confirming ? (
                  <div className="flex flex-col gap-2">
                    <p className="text-xs leading-relaxed text-ink-muted">
                      {t("activate.deactivateConfirm")}
                    </p>
                    <div className="flex gap-2">
                      <button
                        type="button"
                        className="btn-ghost border-rose-200 px-3 py-1 text-xs text-rose-600 hover:bg-rose-50 dark:border-rose-500/30 dark:text-rose-400 dark:hover:bg-rose-500/10"
                        disabled={loading}
                        onClick={() => void handleDeactivate()}
                      >
                        {t("activate.deactivateConfirmYes")}
                      </button>
                      <button
                        type="button"
                        className="btn-quiet px-3 py-1 text-xs"
                        onClick={() => setConfirming(false)}
                      >
                        {t("common.cancel")}
                      </button>
                    </div>
                  </div>
                ) : (
                  <button
                    type="button"
                    className="btn-quiet self-start px-2 py-0 text-xs text-ink-faint hover:text-rose-500"
                    onClick={() => setConfirming(true)}
                  >
                    {t("activate.deactivate")}
                  </button>
                )}
              </div>
            </div>
          </Section>
        </div>

        <Section title={t("activate.quotaTitle")}>
          <div className="flex flex-col gap-2.5">
            <Row label={t("activate.quotaUsed")} value={String(quota?.used ?? 0)} />
            <Row
              label={t("activate.quotaLimit")}
              value={quota?.unlimited ? t("activate.unlimited") : String(quota?.limit ?? 0)}
            />
            <Row
              label={t("activate.quotaRemaining")}
              value={quota?.unlimited ? t("activate.unlimited") : String(quota?.remaining ?? 0)}
            />
            <button
              type="button"
              className="btn-quiet mt-1 self-start px-2 py-0 text-xs"
              disabled={loading}
              onClick={() => void refresh()}
            >
              <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
              {t("activate.refresh")}
            </button>
          </div>
        </Section>
      </div>
    </div>
  );
}

/* ----------------------------------------------------------------- 免费 */

function FreeView(): ReactElement {
  const { t } = useTranslation();
  const quota = useQuota((state) => state.quota);
  const loading = useQuota((state) => state.loading);
  const refresh = useQuota((state) => state.refresh);

  return (
    <div className="flex flex-col gap-5">
      <header className="card flex items-center gap-4 p-5">
        <span className="inline-flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl bg-gradient-to-br from-brand-500 to-violet-500 text-white">
          <BadgeCheck size={22} />
        </span>
        <div className="min-w-0 flex-1">
          <h1 className="text-lg font-semibold text-ink">{t("activate.title")}</h1>
          <p className="mt-0.5 text-xs leading-relaxed text-ink-muted">{t("activate.subtitle")}</p>
        </div>
        <span className="chip shrink-0">{t("activate.statusFree", { limit: quota?.limit ?? 0 })}</span>
      </header>

      <div className="grid items-start gap-5 lg:grid-cols-2">
        <Section title={t("activate.howTitle")}>
          <div className="flex flex-col gap-4">
            <ol className="flex flex-col gap-2.5">
              {STEPS.map((key, index) => (
                <li
                  key={key}
                  className="flex items-start gap-2.5 text-xs leading-relaxed text-ink-muted"
                >
                  <span className="mt-0.5 inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-brand-500/15 text-[10px] font-semibold text-brand-600 dark:text-brand-400">
                    {index + 1}
                  </span>
                  <span className="min-w-0 flex-1">{t(key)}</span>
                </li>
              ))}
            </ol>
            <div className="flex flex-col items-center gap-2 rounded-xl border border-line bg-surface-subtle py-3">
              <img
                src={wechatQrcode}
                alt={t("activate.qrHint")}
                className="h-36 w-36 rounded-xl border border-line bg-white object-contain p-2"
              />
              <span className="flex items-center gap-1.5 text-xs text-ink-muted">
                <QrCode size={13} />
                {t("activate.qrHint")}
              </span>
            </div>
          </div>
        </Section>

        <div className="flex min-w-0 flex-col gap-5">
          <Section title={t("activate.quotaTitle")}>
            <div className="flex flex-col gap-2.5">
              <Row label={t("activate.quotaUsed")} value={String(quota?.used ?? 0)} />
              <Row label={t("activate.quotaLimit")} value={String(quota?.limit ?? 0)} />
              <Row label={t("activate.quotaRemaining")} value={String(quota?.remaining ?? 0)} />
              <button
                type="button"
                className="btn-quiet mt-1 self-start px-2 py-0 text-xs"
                disabled={loading}
                onClick={() => void refresh()}
              >
                <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
                {t("activate.refresh")}
              </button>
            </div>
          </Section>

          <Section title={t("activate.inputTitle")}>
            <ActivationForm />
          </Section>
        </div>
      </div>
    </div>
  );
}

/* ---------------------------------------------------------------- 辅助 */

function planKey(plan: string | null): string {
  return plan === "follow" ? "activate.planFollow" : "activate.planPaid";
}

function formatExp(exp: number): string {
  return new Date(exp * 1000).toLocaleDateString();
}

interface RowProps {
  label: string;
  value: string;
}

/** 一行「标签 — 值」，会员信息与用量共用。 */
function Row({ label, value }: RowProps): ReactElement {
  return (
    <div className="flex items-center justify-between gap-3 text-xs">
      <span className="text-ink-faint">{label}</span>
      <span className="truncate font-mono text-ink-muted">{value}</span>
    </div>
  );
}
