import { useCallback, useState } from "react";

import {
  isAppError,
  runTask,
  type AppError,
  type ErrorKind,
  type OutputFile,
  type TaskRecord,
  type TaskRequest,
  type TaskStatus,
} from "@/lib/tauri";
import { useTasks } from "@/store/tasks";
import type { FeatureId } from "@/features/types";

export type RunnerStatus = TaskStatus | "idle";

export interface StartOptions {
  /** 输出目录，`null` 时由后端回退到默认目录 */
  outDir?: string | null;
  /** 加密文档的打开密码 */
  password?: string | null;
  /** 结束时是否发系统通知 */
  notify?: boolean | null;
}

export interface TaskRunner {
  /** 当前跟踪的任务；任务尚未创建时为 null */
  task: TaskRecord | null;
  status: RunnerStatus;
  running: boolean;
  current: number;
  total: number;
  message: string;
  outputs: OutputFile[];
  errorKind: ErrorKind | null;
  errorMessage: string | null;
  elapsedMs: number;
  start(request: TaskRequest, options?: StartOptions): Promise<TaskRecord | null>;
  cancel(): Promise<void>;
  reset(): void;
}

/**
 * 统一任务运行器。
 *
 * 进度、完成、失败全部来自后端事件（写入 `store/tasks.ts`），本 hook 只负责
 * 「这次运行对应哪条记录」的绑定，因此所有功能页共用同一套进度与错误处理。
 *
 * 绑定的难点：`taskId` 由后端生成，要等 `run_task` 返回才知道，而进度事件在
 * 返回之前就开始到达。因此运行期间先按「同功能、开始时间不早于本次调用」匹配，
 * 拿到返回值后再切换为按 id 精确匹配。
 */
export function useTaskRunner(feature: FeatureId): TaskRunner {
  const [taskId, setTaskId] = useState<string | null>(null);
  /** 本次调用的时间下界，用于在 id 未知时定位记录 */
  const [since, setSince] = useState<number | null>(null);
  /** `run_task` 在创建任务之前就失败（如输出目录不可写）时的本地错误 */
  const [localError, setLocalError] = useState<AppError | null>(null);

  const task = useTasks((state) => {
    if (taskId) return state.tasks.find((item) => item.id === taskId) ?? null;
    if (since === null) return null;
    return (
      state.tasks.find(
        (item) =>
          item.feature === feature &&
          item.status === "running" &&
          item.startedAt >= since,
      ) ?? null
    );
  });

  const start = useCallback(
    async (request: TaskRequest, options: StartOptions = {}) => {
      setLocalError(null);
      setTaskId(null);
      // 留 5 秒余量：前端时钟与后端记录的时间戳可能有细微偏差
      setSince(Date.now() - 5_000);

      try {
        const record = await runTask(
          request,
          options.outDir ?? null,
          options.password ?? null,
          options.notify ?? null,
        );
        setTaskId(record.id);
        return record;
      } catch (error) {
        const normalized: AppError = isAppError(error)
          ? error
          : { kind: "internal", message: String(error) };
        setLocalError(normalized);
        return null;
      } finally {
        setSince(null);
      }
    },
    [],
  );

  const cancel = useCallback(async () => {
    const id = taskId ?? task?.id;
    if (id) await useTasks.getState().cancel(id);
  }, [taskId, task?.id]);

  const reset = useCallback(() => {
    setTaskId(null);
    setSince(null);
    setLocalError(null);
  }, []);

  const errorKind = task?.errorKind ?? localError?.kind ?? null;
  const errorMessage = task?.error ?? localError?.message ?? null;
  const status: RunnerStatus = task
    ? task.status
    : localError
      ? "failed"
      : "idle";

  return {
    task,
    status,
    running: status === "running",
    current: task?.current ?? 0,
    total: task?.total ?? 0,
    message: task?.message ?? "",
    outputs: task?.outputs ?? [],
    errorKind,
    errorMessage,
    elapsedMs: task?.elapsedMs ?? 0,
    start,
    cancel,
    reset,
  };
}
