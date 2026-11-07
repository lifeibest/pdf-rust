import { FolderOpen, FolderX } from "lucide-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { shortenPath } from "@/lib/paths";
import type { OutputPickerApi } from "@/hooks/useOutputPicker";

export interface OutputPickerProps {
  api: OutputPickerApi;
  /** 单来源功能（如旋转的原地模式）可以隐藏目录选择 */
  showDir?: boolean;
  /** 输出文件名由后端按规则生成时可以隐藏输入框 */
  showName?: boolean;
  disabled?: boolean;
}

/** 输出目录与文件名选择。 */
export function OutputPicker({
  api,
  showDir = true,
  showName = true,
  disabled = false,
}: OutputPickerProps): ReactElement | null {
  const { t } = useTranslation();
  if (!showDir && !showName) return null;

  return (
    <div className="flex flex-col gap-4">
      {showDir ? (
        <div className="flex flex-col gap-1.5">
          <span className="label mb-0">{t("common.outputDir")}</span>
          <div className="flex items-center gap-2">
            <span
              className="field flex min-w-0 flex-1 items-center gap-2 truncate text-xs text-ink-muted"
              title={api.dir ?? undefined}
            >
              {api.dir ? (
                <>
                  <FolderOpen size={14} className="shrink-0 text-ink-faint" />
                  <span className="truncate">{shortenPath(api.dir, 32)}</span>
                </>
              ) : (
                <span className="text-ink-faint">{t("common.defaultDir")}</span>
              )}
            </span>
            <button
              type="button"
              className="btn-ghost shrink-0"
              onClick={() => void api.chooseDir()}
              disabled={disabled}
            >
              {t("common.choose")}
            </button>
            {api.dir ? (
              <button
                type="button"
                className="btn-quiet h-9 w-9 shrink-0 rounded-xl px-0"
                onClick={() => api.setDir(null)}
                disabled={disabled}
                title={t("common.defaultDir")}
                aria-label={t("common.defaultDir")}
              >
                <FolderX size={15} />
              </button>
            ) : null}
          </div>
        </div>
      ) : null}

      {showName ? (
        <div className="flex flex-col gap-1.5">
          <label className="label mb-0" htmlFor="output-name">
            {t("common.outputName")}
          </label>
          <input
            id="output-name"
            type="text"
            className="field text-xs"
            value={api.name}
            placeholder={t("common.outputNamePlaceholder")}
            disabled={disabled}
            onChange={(event) => api.setName(event.target.value)}
          />
        </div>
      ) : null}
    </div>
  );
}
