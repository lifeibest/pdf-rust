import { useCallback, useMemo, useRef, useState } from "react";

import { pickFiles, probeDocument, statPaths } from "@/lib/tauri";
import { basename, extname } from "@/lib/paths";
import type { FeatureDefinition, SourceFile } from "@/features/types";

export interface AddOptions {
  /** true 时替换整个列表（单来源功能、全局拖拽预填都用它） */
  replace?: boolean;
}

export interface SourceFilesApi {
  files: SourceFile[];
  /** 正在读取文件信息 / 探测文档 */
  busy: boolean;
  /** 因类型不符被跳过的文件名，供界面提示 */
  rejected: string[];
  /** 至少有一个加密文档还没填对密码 */
  needsPassword: boolean;
  add(paths: string[], options?: AddOptions): Promise<void>;
  /** 打开系统文件选择框 */
  pick(): Promise<void>;
  remove(id: string): void;
  clear(): void;
  reorder(activeId: string, overId: string): void;
  setPassword(id: string, password: string): Promise<void>;
  dismissRejected(): void;
}

let sequence = 0;

function nextId(path: string): string {
  sequence += 1;
  return `${path}::${sequence}`;
}

/**
 * 功能页的来源文件列表。
 *
 * 统一处理三件事：按 `feature.accepts` 过滤扩展名、批量补齐体积信息、
 * 对 PDF 逐个探测页数与加密状态。各功能页因此不必重复这套逻辑。
 */
export function useSourceFiles(feature: FeatureDefinition): SourceFilesApi {
  const [files, setFiles] = useState<SourceFile[]>([]);
  const [busy, setBusy] = useState(false);
  const [rejected, setRejected] = useState<string[]>([]);
  /** 防止异步探测回来时列表已被清空 / 替换 */
  const generation = useRef(0);

  const allowed = useMemo(() => new Set(feature.accepts.ext), [feature]);

  const add = useCallback(
    async (paths: string[], options: AddOptions = {}) => {
      const unique = [...new Set(paths)].filter((path) => path.length > 0);
      if (unique.length === 0) return;

      const accepted: string[] = [];
      const skipped: string[] = [];
      for (const path of unique) {
        if (allowed.has(extname(path))) accepted.push(path);
        else skipped.push(basename(path));
      }
      setRejected(skipped);

      // 单来源功能只保留最后一个，符合「选一个 PDF 来处理」的直觉
      const kept = feature.accepts.multiple
        ? accepted
        : accepted.slice(-1);
      if (kept.length === 0) return;

      const token = ++generation.current;
      setBusy(true);
      try {
        const stats = await statPaths(kept).catch(() => null);
        const statOf = new Map((stats ?? []).map((item) => [item.path, item]));

        const built: SourceFile[] = [];
        for (const path of kept) {
          const stat = statOf.get(path);
          const item: SourceFile = {
            id: nextId(path),
            path,
            name: stat?.name ?? basename(path),
            size: stat?.sizeBytes ?? 0,
            doc: null,
            password: null,
          };
          if (stat?.kind === "pdf" || (!stat && extname(path) === "pdf")) {
            item.doc = await probeQuietly(path, null);
          }
          built.push(item);
        }

        if (token !== generation.current) return;
        setFiles((current) => {
          if (options.replace) return built;
          if (!feature.accepts.multiple) return built.slice(-1);
          return [...current, ...built];
        });
      } finally {
        if (token === generation.current) setBusy(false);
      }
    },
    [allowed, feature.accepts.multiple],
  );

  const pick = useCallback(async () => {
    const result = await pickFiles({
      extensions: feature.accepts.ext,
      multiple: feature.accepts.multiple,
    }).catch(() => null);
    if (!result || result.cancelled || result.paths.length === 0) return;
    // 选择框本身就是「重新选一批」，单来源功能替换、多来源功能追加
    await add(result.paths, { replace: !feature.accepts.multiple });
  }, [add, feature.accepts.ext, feature.accepts.multiple]);

  const remove = useCallback((id: string) => {
    setFiles((current) => current.filter((item) => item.id !== id));
  }, []);

  const clear = useCallback(() => {
    generation.current += 1;
    setBusy(false);
    setRejected([]);
    setFiles([]);
  }, []);

  const reorder = useCallback((activeId: string, overId: string) => {
    setFiles((current) => {
      const from = current.findIndex((item) => item.id === activeId);
      const to = current.findIndex((item) => item.id === overId);
      if (from < 0 || to < 0 || from === to) return current;
      const next = [...current];
      const [moved] = next.splice(from, 1);
      next.splice(to, 0, moved);
      return next;
    });
  }, []);

  const setPassword = useCallback(async (id: string, password: string) => {
    const target = files.find((item) => item.id === id);
    if (!target) return;
    const doc = await probeQuietly(target.path, password);
    setFiles((current) =>
      current.map((item) =>
        item.id === id ? { ...item, password, doc: doc ?? item.doc } : item,
      ),
    );
  }, [files]);

  const dismissRejected = useCallback(() => setRejected([]), []);

  const needsPassword = files.some(
    (item) => item.doc?.encrypted === true && item.doc.pageCount === 0,
  );

  return {
    files,
    busy,
    rejected,
    needsPassword,
    add,
    pick,
    remove,
    clear,
    reorder,
    setPassword,
    dismissRejected,
  };
}

/**
 * 探测文档信息，失败时返回 null。
 *
 * 引擎未就绪、文件被占用、密码错误都会走到这里；列表不该因为一项探测失败
 * 而整批加不进来，界面会用「未知页数」而不是报错来呈现。
 */
async function probeQuietly(
  path: string,
  password: string | null,
): Promise<SourceFile["doc"]> {
  return probeDocument(path, password).catch(() => null);
}
