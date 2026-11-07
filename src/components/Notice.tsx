import clsx from "clsx";
import { AlertTriangle, CheckCircle2, Info, X, XCircle } from "lucide-react";
import type { ReactElement, ReactNode } from "react";
import { useTranslation } from "react-i18next";

export type NoticeTone = "info" | "warn" | "error" | "success";

export interface NoticeProps {
  tone?: NoticeTone;
  children: ReactNode;
  /** 右侧操作（如「去查看」） */
  action?: ReactNode;
  onDismiss?(): void;
  className?: string;
}

const TONES: Record<NoticeTone, { box: string; icon: ReactElement }> = {
  info: {
    box: "border-line bg-surface-subtle text-ink-muted",
    icon: <Info size={15} className="text-brand-500" />,
  },
  warn: {
    box: "border-amber-200 bg-amber-50 text-amber-800 dark:border-amber-500/25 dark:bg-amber-500/10 dark:text-amber-200",
    icon: <AlertTriangle size={15} className="text-amber-500" />,
  },
  error: {
    box: "border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-500/25 dark:bg-rose-500/10 dark:text-rose-300",
    icon: <XCircle size={15} className="text-rose-500" />,
  },
  success: {
    box: "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-500/25 dark:bg-emerald-500/10 dark:text-emerald-300",
    icon: <CheckCircle2 size={15} className="text-emerald-500" />,
  },
};

/** 行内提示条：跳过文件、引擎未就绪、设置已保存等轻量反馈。 */
export function Notice({
  tone = "info",
  children,
  action,
  onDismiss,
  className,
}: NoticeProps): ReactElement {
  const { t } = useTranslation();
  const style = TONES[tone];
  return (
    <div
      className={clsx(
        "flex items-start gap-2 rounded-xl border px-3 py-2 text-xs leading-relaxed",
        style.box,
        className,
      )}
      role={tone === "error" ? "alert" : "status"}
    >
      <span className="mt-0.5 shrink-0">{style.icon}</span>
      <span className="min-w-0 flex-1 break-words">{children}</span>
      {action ? <span className="shrink-0">{action}</span> : null}
      {onDismiss ? (
        <button
          type="button"
          onClick={onDismiss}
          className="-mr-1 shrink-0 rounded-md p-0.5 opacity-60 transition-opacity hover:opacity-100"
          aria-label={t("common.close")}
        >
          <X size={13} />
        </button>
      ) : null}
    </div>
  );
}
