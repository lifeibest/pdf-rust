import clsx from "clsx";
import type { ReactElement, ReactNode } from "react";

export interface SectionProps {
  title: string;
  /** 标题右侧的辅助说明 */
  hint?: string;
  /** 标题右侧的操作区（如「清空」按钮） */
  action?: ReactNode;
  children: ReactNode;
  className?: string;
  /** 内容区不留内边距（列表自带间距时用） */
  flush?: boolean;
}

/** 带标题的卡片区块，功能页与设置页共用的版面单元。 */
export function Section({
  title,
  hint,
  action,
  children,
  className,
  flush = false,
}: SectionProps): ReactElement {
  return (
    <section className={clsx("card flex flex-col overflow-hidden", className)}>
      <header className="flex items-center gap-2 border-b border-line px-4 py-3">
        <h2 className="flex-1 text-sm font-semibold text-ink">{title}</h2>
        {hint ? <span className="text-[11px] text-ink-faint">{hint}</span> : null}
        {action}
      </header>
      <div className={clsx(flush ? "p-0" : "p-4")}>{children}</div>
    </section>
  );
}
