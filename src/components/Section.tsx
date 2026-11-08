import clsx from "clsx";
import { ChevronRight } from "lucide-react";
import { useState, type ReactElement, type ReactNode } from "react";

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
  /** 可折叠：标题变成开关，点击展开 / 收起内容 */
  collapsible?: boolean;
  /** 折叠区块的初始状态，默认展开 */
  defaultOpen?: boolean;
}

/** 带标题的卡片区块，功能页与设置页共用的版面单元。 */
export function Section({
  title,
  hint,
  action,
  children,
  className,
  flush = false,
  collapsible = false,
  defaultOpen = true,
}: SectionProps): ReactElement {
  const [open, setOpen] = useState(defaultOpen);
  // 非折叠区块始终展开
  const expanded = collapsible ? open : true;

  return (
    <section className={clsx("card flex flex-col overflow-hidden", className)}>
      <header
        className={clsx(
          "flex items-center gap-2 px-4 py-3",
          expanded && "border-b border-line",
        )}
      >
        {collapsible ? (
          <button
            type="button"
            onClick={() => setOpen((value) => !value)}
            aria-expanded={open}
            className="-mx-1 flex flex-1 items-center gap-1.5 rounded-lg px-1 text-left transition-colors hover:bg-surface-subtle"
          >
            <ChevronRight
              size={15}
              className={clsx(
                "shrink-0 text-ink-faint transition-transform",
                open && "rotate-90",
              )}
            />
            <h2 className="flex-1 text-sm font-semibold text-ink">{title}</h2>
            {hint ? <span className="text-[11px] text-ink-faint">{hint}</span> : null}
          </button>
        ) : (
          <>
            <h2 className="flex-1 text-sm font-semibold text-ink">{title}</h2>
            {hint ? <span className="text-[11px] text-ink-faint">{hint}</span> : null}
          </>
        )}
        {action}
      </header>
      {expanded ? <div className={clsx(flush ? "p-0" : "p-4")}>{children}</div> : null}
    </section>
  );
}
