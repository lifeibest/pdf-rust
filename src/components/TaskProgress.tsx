import clsx from "clsx";
import { CheckCircle2, Loader2, Play, XCircle, Ban } from "lucide-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { formatDuration, percent } from "@/lib/format";
import { splitError } from "@/lib/errors";
import { OutputList } from "./OutputList";
import type { TaskRunner } from "@/hooks/useTaskRunner";
import type { FeatureDefinition } from "@/features/types";

export interface TaskProgressProps {
  feature: FeatureDefinition;
  runner: TaskRunner;
  /** 文件与选项是否已满足运行条件 */
  canRun: boolean;
  /** `canRun` 为假时显示在按钮旁的原因 */
  hint?: string;
  onRun(): void | Promise<void>;
  /** 完成后「再处理一批」，通常用来清空来源列表 */
  onRunAnother?(): void;
}

/**
 * 运行按钮 + 进度 + 结果。
 *
 * 所有状态都从 `useTaskRunner` 读出，因此任何功能页只要接上 runner
 * 就自动获得一致的进度条、错误提示与产出列表。
 */
export function TaskProgress({
  feature,
  runner,
  canRun,
  hint,
  onRun,
  onRunAnother,
}: TaskProgressProps): ReactElement {
  const { t } = useTranslation();
  const { status, running, current, total, message, outputs, errorKind, errorMessage, elapsedMs } =
    runner;

  const ratio = percent(current, total);
  const error = splitError(errorKind, errorMessage, t);

  return (
    <div className="flex flex-col gap-3">
      {running ? (
        <div className="card flex flex-col gap-2 p-4">
          <div className="flex items-center gap-2 text-sm text-ink">
            <Loader2 size={16} className="animate-spin text-brand-500" />
            <span className="flex-1 truncate">{message || t("common.running")}</span>
            <span className="shrink-0 text-xs tabular-nums text-ink-muted">
              {total > 0 ? `${current}${t("common.ofTotal", { total })}` : ""}
              {total > 0 ? ` · ${ratio}%` : ""}
            </span>
          </div>

          <div className="h-1.5 w-full overflow-hidden rounded-full bg-surface-subtle">
            <div
              className={clsx(
                "h-full rounded-full bg-gradient-to-r transition-[width] duration-200",
                feature.accent,
              )}
              // 后端在开始阶段可能还没算出 total，给个不定进度条避免看起来卡死
              style={{ width: total > 0 ? `${Math.max(ratio, 2)}%` : "35%" }}
            />
          </div>

          <button type="button" className="btn-ghost self-start" onClick={() => void runner.cancel()}>
            <Ban size={15} />
            {t("common.cancel")}
          </button>
        </div>
      ) : null}

      {!running && status === "failed" ? (
        <div className="flex items-start gap-2 rounded-2xl border border-rose-200 bg-rose-50 px-4 py-3 dark:border-rose-500/25 dark:bg-rose-500/10">
          <XCircle size={16} className="mt-0.5 shrink-0 text-rose-500" />
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium text-rose-700 dark:text-rose-300">{error.title}</p>
            {error.detail ? (
              <p className="mt-0.5 break-words text-xs text-rose-600/85 dark:text-rose-200/80">
                {error.detail}
              </p>
            ) : null}
          </div>
        </div>
      ) : null}

      {!running && status === "cancelled" ? (
        <p className="flex items-center gap-2 text-xs text-ink-faint">
          <Ban size={14} />
          {t("feature.runCancelled")}
        </p>
      ) : null}

      {!running && status === "finished" ? (
        <div className="flex flex-col gap-3">
          <p className="flex items-center gap-2 text-sm text-emerald-600 dark:text-emerald-400">
            <CheckCircle2 size={16} />
            {t("feature.runSuccess", { count: outputs.length })}
            {elapsedMs > 0 ? (
              <span className="text-xs text-ink-faint">
                {t("tasks.elapsed", { duration: formatDuration(elapsedMs, t) })}
              </span>
            ) : null}
          </p>
          <OutputList outputs={outputs} />
          {onRunAnother ? (
            <button type="button" className="btn-ghost self-start" onClick={onRunAnother}>
              {t("feature.runAnother")}
            </button>
          ) : null}
        </div>
      ) : null}

      {!running ? (
        <div className="flex items-center gap-3">
          <button
            type="button"
            className="btn-primary min-w-[9rem]"
            disabled={!canRun}
            onClick={() => void onRun()}
          >
            <Play size={16} />
            {t("common.run")}
          </button>
          {!canRun && hint ? <span className="text-xs text-ink-faint">{hint}</span> : null}
        </div>
      ) : null}
    </div>
  );
}
