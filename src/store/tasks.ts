import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import {
  cancelTask,
  clearTasks,
  EVENT,
  listTasks,
  type FailedPayload,
  type FinishedPayload,
  type ProgressPayload,
  type TaskRecord,
} from "@/lib/tauri";

interface TasksState {
  /** 按开始时间倒序，与后端 `list_tasks` 的顺序一致 */
  tasks: TaskRecord[];
  /** 事件监听是否已建立，避免 StrictMode / HMR 下重复订阅 */
  listening: boolean;
  drawerOpen: boolean;

  bootstrap(): Promise<void>;
  cancel(id: string): Promise<void>;
  clearFinished(): Promise<void>;
  setDrawerOpen(open: boolean): void;
}

/** 监听在应用生命周期内常驻；保存句柄只为 HMR 时能干净地拆掉。 */
let activeListeners: UnlistenFn[] = [];

export const useTasks = create<TasksState>((set, get) => ({
  tasks: [],
  listening: false,
  drawerOpen: false,

  async bootstrap() {
    if (get().listening) return;
    // 先标记，避免并发调用（StrictMode 会连续触发两次 effect）重复订阅
    set({ listening: true });

    const progress = await listen<ProgressPayload>(EVENT.progress, ({ payload }) => {
      upsertTask(set, payload.taskId, payload.feature, () => ({
        status: "running",
        current: payload.current,
        total: payload.total,
        message: payload.message,
      }));
    });

    const finished = await listen<FinishedPayload>(EVENT.finished, ({ payload }) => {
      const count = payload.outputs.length;
      upsertTask(set, payload.taskId, payload.feature, (task) => ({
        status: "finished",
        current: Math.max(task.current, count),
        total: Math.max(task.total, count),
        message: "",
        outputs: payload.outputs,
        elapsedMs: payload.elapsedMs,
        summary: payload.summary,
        error: null,
        errorKind: null,
      }));
    });

    const failed = await listen<FailedPayload>(EVENT.failed, ({ payload }) => {
      upsertTask(set, payload.taskId, payload.feature, () => ({
        // 用户取消也走 failed 事件，这里按 kind 还原为 cancelled
        status: payload.error.kind === "cancelled" ? "cancelled" : "failed",
        message: "",
        error: payload.error.message ?? "",
        errorKind: payload.error.kind,
        elapsedMs: payload.elapsedMs,
      }));
    });

    activeListeners = [progress, finished, failed];

    // 监听建立后再拉快照，避免快照请求在途期间的事件被丢掉
    const snapshot = await listTasks().catch(() => [] as TaskRecord[]);
    set((state) => ({ tasks: mergeSnapshot(snapshot, state.tasks) }));
  },

  async cancel(id) {
    await cancelTask(id).catch(() => false);
    // 后端随后会发 failed(cancelled) 事件，这里先本地置位让按钮立刻失去意义
    upsertTask(set, id, "", () => ({ status: "cancelled", message: "" }));
  },

  async clearFinished() {
    await clearTasks().catch(() => 0);
    set((state) => ({ tasks: state.tasks.filter((task) => task.status === "running") }));
  },

  setDrawerOpen(open) {
    set({ drawerOpen: open });
  },
}));

type Setter = (
  partial: Partial<TasksState> | ((state: TasksState) => Partial<TasksState>),
) => void;

/**
 * 更新某个任务；不存在时新建一条占位记录。
 *
 * `list_tasks` 快照只在启动时拉一次，之后新开的任务全靠事件驱动，
 * 因此事件必须能把记录「补」进列表，否则任务中心永远看不到本次运行。
 */
function upsertTask(
  set: Setter,
  id: string,
  feature: string,
  mutate: (task: TaskRecord) => Partial<TaskRecord>,
) {
  set((state) => {
    const existing = state.tasks.find((task) => task.id === id);
    if (existing) {
      return {
        tasks: state.tasks.map((task) =>
          task.id === id ? { ...task, ...mutate(task) } : task,
        ),
      };
    }

    const skeleton: TaskRecord = {
      id,
      feature,
      status: "running",
      current: 0,
      total: 0,
      message: "",
      outputs: [],
      error: null,
      errorKind: null,
      startedAt: Date.now(),
      elapsedMs: 0,
      summary: null,
    };
    const created: TaskRecord = { ...skeleton, ...mutate(skeleton) };

    // feature 为空说明是本地取消一条不存在的任务，没必要为它建记录
    if (created.feature.length === 0) return {};
    return { tasks: [created, ...state.tasks] };
  });
}

/**
 * 合并后端快照与本地记录。
 *
 * 同 id 时保留进度更靠前的那一份：快照请求在途中任务可能已经推进甚至完成。
 * 只存在于本地、快照里没有的任务，说明后端已把它清理掉，直接丢弃。
 */
function mergeSnapshot(snapshot: TaskRecord[], local: TaskRecord[]): TaskRecord[] {
  if (local.length === 0) return snapshot;

  const localById = new Map(local.map((task) => [task.id, task]));
  const merged = snapshot.map((task) => pickFresher(task, localById.get(task.id)));
  return [...merged].sort((left, right) => right.startedAt - left.startedAt);
}

function pickFresher(snapshotTask: TaskRecord, localTask?: TaskRecord): TaskRecord {
  if (!localTask) return snapshotTask;
  return progressOf(localTask) > progressOf(snapshotTask) ? localTask : snapshotTask;
}

/** 终态记录一律视为「比运行中更新」，其次比较已处理数。 */
function progressOf(task: TaskRecord): number {
  return task.status === "running" ? task.current : 1_000_000 + task.current;
}

/** 当前是否有任务在跑，用于侧边栏角标。 */
export function hasRunningTasks(): boolean {
  return useTasks.getState().tasks.some((task) => task.status === "running");
}

/** HMR 时拆掉旧监听，避免同一事件被处理两次。 */
export function disposeTaskListeners() {
  activeListeners.forEach((unlisten) => unlisten());
  activeListeners = [];
}
