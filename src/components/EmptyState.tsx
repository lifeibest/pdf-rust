import clsx from "clsx";
import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

export interface EmptyStateProps {
  icon?: LucideIcon;
  title: string;
  hint?: string;
  action?: ReactNode;
  className?: string;
}

/** 列表 / 面板为空时的占位块。 */
export function EmptyState({ icon: Icon, title, hint, action, className }: EmptyStateProps) {
  return (
    <div
      className={clsx(
        "flex flex-col items-center justify-center gap-2 rounded-2xl px-6 py-10 text-center",
        className,
      )}
    >
      {Icon ? (
        <span className="mb-1 inline-flex h-11 w-11 items-center justify-center rounded-2xl bg-surface-subtle text-ink-faint">
          <Icon size={20} />
        </span>
      ) : null}
      <p className="text-sm font-medium text-ink-muted">{title}</p>
      {hint ? <p className="max-w-sm text-xs text-ink-faint">{hint}</p> : null}
      {action ? <div className="mt-3">{action}</div> : null}
    </div>
  );
}
