import { Sparkles, X } from "lucide-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";

import { ActivationForm } from "@/components/ActivationForm";
import { useQuota } from "@/store/quota";

/**
 * 配额用尽时的全局激活引导弹窗。
 *
 * 挂在 AppLayout 层，与当前路由无关：任何功能页触发 `quotaExceeded` 都会打开它，
 * 用户可就地粘贴激活码，或跳去「获取激活码」页了解关注公众号 / 付费流程。
 */
export function ActivationModal(): ReactElement | null {
  const { t } = useTranslation();
  const navigate = useNavigate();

  const open = useQuota((state) => state.modalOpen);
  const quota = useQuota((state) => state.quota);
  const closeModal = useQuota((state) => state.closeModal);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/45 p-6">
      <div className="card flex w-full max-w-md flex-col gap-4 p-5">
        <div className="flex items-start gap-3">
          <span className="inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-brand-500 to-violet-500 text-white">
            <Sparkles size={17} />
          </span>
          <div className="min-w-0 flex-1">
            <p className="text-sm font-semibold text-ink">{t("activate.modalTitle")}</p>
            <p className="mt-1 text-xs leading-relaxed text-ink-muted">
              {quota?.limit != null
                ? t("activate.modalDescLimited", { limit: quota.limit, used: quota.used })
                : t("activate.modalDesc")}
            </p>
          </div>
          <button
            type="button"
            className="btn-quiet h-7 w-7 shrink-0 rounded-lg px-0 py-0"
            onClick={closeModal}
            aria-label={t("common.close")}
            title={t("common.close")}
          >
            <X size={15} />
          </button>
        </div>

        <ActivationForm onActivated={closeModal} />

        <button
          type="button"
          className="btn-ghost"
          onClick={() => {
            closeModal();
            navigate("/activate");
          }}
        >
          {t("activate.goActivate")}
        </button>
      </div>
    </div>
  );
}
