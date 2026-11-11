import clsx from "clsx";
import { Ban, CheckCircle2, ListChecks, Loader2, Trash2, X, XCircle } from "lucide-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { formatDuration, formatRelativeTime, percent } from "@/lib/format";
import { splitError } from "@/lib/errors";
import { featureById } from "@/features/registry";
import { useTasks } from "@/store/tasks";
import { EmptyState } from "./EmptyState";
import { OutputList } from "./OutputList";
import type { TaskRecord, TaskStatus } from "@/lib/tauri";

/**
 * 任务中心抽屉。
 *
 * 挂在布局层，因此从任何页面都能看到进行中的任务；数据全部来自
 * `store/tasks.ts`（后端事件驱动），组件本身不持有任何任务状态。
 */
export function TaskDrawer(): ReactElement | null {
  const { t } = useTranslation();
  const open = useTasks((state) => state.drawerOpen);
  const tasks = useTasks((state) => state.tasks);
  const setOpen = useTasks((state) => state.setDrawerOpen);
  const cancel = useTasks((state) => state.cancel);
  const clearFinished = useTasks((state) => state.clearFinished);

  if (!open) return null;

  const hasFinished = tasks.some((task) => task.status !== "running");

  return (
    <section
      className="fixed bottom-4 right-4 z-40 flex max-h-[70vh] w-[22rem] flex-col overflow-hidden rounded-2xl border border-line bg-surface shadow-panel animate-slide-up"
      aria-label={t("tasks.title")}
    >
      <header className="flex items-center gap-2 border-b border-line px-4 py-3">
        <ListChecks size={16} className="text-brand-500" />
        <h2 className="flex-1 text-sm font-semibold text-ink">{t("tasks.title")}</h2>
        {hasFinished ? (
          <button
            type="button"
            className="btn-quiet h-7 rounded-lg px-2 py-0 text-xs"
            onClick={() => void clearFinished()}
            title={t("tasks.clearFinished")}
          >
            <Trash2 size={13} />
            {t("tasks.clearFinished")}
          </button>
        ) : null}
        <button
          type="button"
          className="btn-quiet h-7 w-7 rounded-lg px-0 py-0"
          onClick={() => setOpen(false)}
          aria-label={t("common.close")}
          title={t("common.close")}
        >
          <X size={15} />
        </button>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto p-3">
        {tasks.length === 0 ? (
          <EmptyState icon={ListChecks} title={t("tasks.empty")} hint={t("tasks.emptyHint")} />
        ) : (
          <ul className="flex flex-col gap-2">
            {tasks.map((task) => (
              <TaskRow key={task.id} task={task} onCancel={cancel} />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

interface TaskRowProps {
  task: TaskRecord;
  onCancel(id: string): Promise<void>;
}

function TaskRow({ task, onCancel }: TaskRowProps): ReactElement {
  const { t } = useTranslation();
  const definition = featureById(task.feature);
  const title = definition ? t(definition.titleKey) : task.feature;
  const error = splitError(task.errorKind, task.error, t);

  return (
    <li className="flex flex-col gap-2 rounded-xl border border-line bg-surface-subtle p-3">
      <div className="flex items-center gap-2">
        <StatusIcon status={task.status} />
        <span className="min-w-0 flex-1 truncate text-xs font-medium text-ink">{title}</span>
        <StatusChip status={task.status} />
      </div>

      {task.status === "running" ? (
        <>
          <div className="h-1 w-full overflow-hidden rounded-full bg-line">
            <div
              className="h-full rounded-full bg-brand-500 transition-[width] duration-200"
              style={{
                width: task.total > 0 ? `${Math.max(percent(task.current, task.total), 2)}%` : "35%",
              }}
            />
          </div>
          <div className="flex items-center gap-2">
            <p className="flex-1 truncate text-[11px] text-ink-faint">
              {task.message ||
                (task.total > 0 ? `${task.current} / ${task.total}` : t("common.running"))}
            </p>
            <button
              type="button"
              className="btn-quiet h-6 rounded-md px-2 py-0 text-[11px]"
              onClick={() => void onCancel(task.id)}
            >
              <Ban size={12} />
              {t("tasks.cancel")}
            </button>
          </div>
        </>
      ) : null}

      {task.status === "failed" ? (
        <p className="break-words text-[11px] text-rose-500">
          {error.title}
          {error.detail ? ` · ${error.detail}` : ""}
        </p>
      ) : null}

      {task.status === "finished" && task.summary ? (
        <p className="break-words text-[11px] font-medium text-ink-muted">{task.summary}</p>
      ) : null}

      {task.status === "finished" ? <OutputList outputs={task.outputs} dense /> : null}

      {task.status !== "running" ? (
        <p className="text-[11px] text-ink-faint">
          {formatRelativeTime(task.startedAt, t)}
          {task.elapsedMs > 0
            ? ` · ${t("tasks.elapsed", { duration: formatDuration(task.elapsedMs, t) })}`
            : ""}
        </p>
      ) : null}
    </li>
  );
}

function StatusIcon({ status }: { status: TaskStatus }): ReactElement {
  switch (status) {
    case "running":
      return <Loader2 size={14} className="shrink-0 animate-spin text-brand-500" />;
    case "finished":
      return <CheckCircle2 size={14} className="shrink-0 text-emerald-500" />;
    case "cancelled":
      return <Ban size={14} className="shrink-0 text-ink-faint" />;
    case "failed":
      return <XCircle size={14} className="shrink-0 text-rose-500" />;
  }
}

function StatusChip({ status }: { status: TaskStatus }): ReactElement {
  const { t } = useTranslation();
  const label = t(`tasks.${status}`);
  return (
    <span
      className={clsx(
        "shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium",
        status === "running" && "bg-brand-500/12 text-brand-600 dark:text-brand-300",
        status === "finished" && "bg-emerald-500/12 text-emerald-600 dark:text-emerald-400",
        status === "failed" && "bg-rose-500/12 text-rose-600 dark:text-rose-400",
        status === "cancelled" && "bg-surface text-ink-faint",
      )}
    >
      {label}
    </span>
  );
}
