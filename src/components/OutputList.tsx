import { FileArchive, FileText, FolderSearch, ImageIcon, ExternalLink } from "lucide-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { formatBytes } from "@/lib/format";
import { openPath, reveal, type OutputFile } from "@/lib/tauri";

export interface OutputListProps {
  outputs: OutputFile[];
  /** 任务抽屉里空间紧张，只显示名字与一个操作 */
  dense?: boolean;
}

/** 任务产出文件列表：可直接打开或在访达中定位。 */
export function OutputList({ outputs, dense = false }: OutputListProps): ReactElement | null {
  const { t } = useTranslation();
  if (outputs.length === 0) return null;

  return (
    <div className="flex flex-col gap-2">
      {!dense && outputs.length > 1 ? (
        <button
          type="button"
          className="btn-ghost h-8 w-full justify-center py-0 text-xs"
          onClick={() => void reveal(outputs[0].path).catch(() => {})}
        >
          <FolderSearch size={14} />
          {t("common.openDir")}（{outputs.length} {t("common.items")}）
        </button>
      ) : null}
      <ul
        className={
          !dense && outputs.length > 4
            ? "flex max-h-60 flex-col gap-1.5 overflow-y-auto pr-0.5"
            : "flex flex-col gap-1.5"
        }
      >
        {outputs.map((output) => (
          <li
            key={output.path}
            className="flex items-center gap-2 rounded-xl border border-line bg-surface-subtle px-3 py-2"
          >
            <KindIcon kind={output.kind} />
            <div className="min-w-0 flex-1">
              <p className="truncate text-xs font-medium text-ink" title={output.path}>
                {output.name}
              </p>
              {!dense ? (
                <p className="text-[11px] text-ink-faint">
                  {formatBytes(output.sizeBytes)}
                  {output.pages ? ` · ${t("common.pageCount", { value: output.pages })}` : ""}
                </p>
              ) : null}
            </div>
            <button
              type="button"
              className="btn-quiet h-7 w-7 rounded-lg px-0 py-0"
              onClick={() => void openPath(output.path).catch(() => {})}
              title={t("common.open")}
              aria-label={t("common.open")}
            >
              <ExternalLink size={14} />
            </button>
            {!dense ? (
              <button
                type="button"
                className="btn-quiet h-7 w-7 rounded-lg px-0 py-0"
                onClick={() => void reveal(output.path).catch(() => {})}
                title={t("common.openDir")}
                aria-label={t("common.openDir")}
              >
                <FolderSearch size={14} />
              </button>
            ) : null}
          </li>
        ))}
      </ul>
    </div>
  );
}

function KindIcon({ kind }: { kind: OutputFile["kind"] }): ReactElement {
  const className = "inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-lg";
  switch (kind) {
    case "pdf":
      return (
        <span className={`${className} bg-rose-50 text-rose-500 dark:bg-rose-500/15`}>
          <FileText size={14} />
        </span>
      );
    case "image":
      return (
        <span className={`${className} bg-sky-50 text-sky-500 dark:bg-sky-500/15`}>
          <ImageIcon size={14} />
        </span>
      );
    case "archive":
      return (
        <span className={`${className} bg-amber-50 text-amber-600 dark:bg-amber-500/15`}>
          <FileArchive size={14} />
        </span>
      );
  }
}
