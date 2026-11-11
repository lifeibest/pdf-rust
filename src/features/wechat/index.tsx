import { Ban, CheckCircle2, Link2, Loader2, MousePointerClick, Play, XCircle } from "lucide-react";
import { useCallback, useMemo, useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { Notice } from "@/components/Notice";
import { OutputList } from "@/components/OutputList";
import { OutputPicker } from "@/components/OutputPicker";
import { Section } from "@/components/Section";
import type { FeaturePageProps } from "@/features/types";
import { useOutputPicker } from "@/hooks/useOutputPicker";
import { splitError } from "@/lib/errors";
import {
  cancelWechatExport,
  confirmWechatExport,
  exportWechatPdf,
  isAppError,
  type AppError,
} from "@/lib/tauri";
import { useQuota } from "@/store/quota";
import { useTasks } from "@/store/tasks";

/** 轻量客户端校验：仅接受微信文章链接，非法时按钮旁给出提示，不必等后端往返。 */
function isValidWechatUrl(raw: string): boolean {
  const value = raw.trim();
  return value.startsWith("https://mp.weixin.qq.com/");
}

/**
 * 微信公众号文章导出 PDF 页（custom 模式）。
 *
 * 不走 `useFeatureController`：来源是 URL 而非文件列表，运行经专用命令
 * `export_wechat_pdf`。进度 / 结果 / 取消复用全局任务事件（`store/tasks`）与产出列表组件。
 */
export default function WechatPage({ feature }: FeaturePageProps): ReactElement {
  const { t } = useTranslation();
  const output = useOutputPicker({ enabled: true });

  const [url, setUrl] = useState("");
  const [taskId, setTaskId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [localError, setLocalError] = useState<AppError | null>(null);
  const [manual, setManual] = useState(false);
  const [confirmed, setConfirmed] = useState(false);

  const task = useTasks((state) =>
    taskId ? state.tasks.find((item) => item.id === taskId) ?? null : null,
  );

  const trimmed = url.trim();
  const valid = isValidWechatUrl(trimmed);
  const running = busy && task?.status !== "finished" && task?.status !== "failed" && task?.status !== "cancelled";
  const canRun = valid && !running;

  const hint = useMemo(() => {
    if (trimmed.length === 0) return t("wechat.needUrl");
    if (!valid) return t("wechat.invalidUrl");
    return null;
  }, [trimmed.length, valid, t]);

  const startRun = useCallback(async (manualMode: boolean) => {
    if (!canRun) return;
    const dir = await output.resolve();
    // 「每次询问」模式下用户取消了目录选择，中止而非悄悄写默认目录
    if (!dir) return;

    const id = crypto.randomUUID();
    setTaskId(id);
    setLocalError(null);
    setConfirmed(false);
    setManual(manualMode);
    setBusy(true);

    try {
      await exportWechatPdf(id, {
        url: trimmed,
        outputName: output.name.trim().length > 0 ? output.name.trim() : null,
        manual: manualMode,
      }, dir);
      output.commit(dir);
    } catch (error) {
      const normalized: AppError = isAppError(error)
        ? error
        : { kind: "internal", message: String(error) };
      // 配额用尽走全局激活引导，不作为普通失败展示
      if (normalized.kind === "quotaExceeded") {
        useQuota.getState().openModal();
      } else {
        setLocalError(normalized);
      }
    } finally {
      setBusy(false);
    }
  }, [canRun, output, trimmed]);

  const onRun = useCallback(() => {
    void startRun(manual);
  }, [startRun, manual]);

  const onConfirm = useCallback(async () => {
    if (!taskId) return;
    await confirmWechatExport(taskId);
    setConfirmed(true);
  }, [taskId]);

  const onCancel = useCallback(async () => {
    if (taskId) await cancelWechatExport(taskId);
  }, [taskId]);

  // 错误优先级：本次调用的本地错误（创建任务前失败）> 任务事件里的失败
  const failKind = localError
    ? localError.kind
    : task?.status === "failed"
      ? task.errorKind
      : null;
  const failMessage = localError
    ? localError.message
    : task?.status === "failed"
      ? task.error
      : null;
  const failure = failKind || failMessage ? splitError(failKind, failMessage, t) : null;
  // 微信功能里的 unsupported 专指「读不到正文」，用更贴切标题替换通用的「不支持的文件类型」
  const failureTitle = failKind === "unsupported" ? t("wechat.blockedTitle") : failure?.title ?? null;
  const showRetryManual = failKind === "unsupported" && !manual;

  const showSuccess = !busy && task?.status === "finished";
  const showCancelled = !busy && task?.status === "cancelled";

  return (
    <div className="flex flex-col gap-5">
      <p className="text-sm text-ink-muted">{t(feature.descKey)}</p>

      <div className="grid items-start gap-5 xl:grid-cols-[minmax(0,1fr)_21rem]">
        <div className="flex min-w-0 flex-col gap-5">
          <Section title={t("wechat.article")} hint={t("wechat.articleHint")}>
            <div className="flex flex-col gap-1.5">
              <label className="label mb-0" htmlFor="wechat-url">
                {t("wechat.urlLabel")}
              </label>
              <div className="flex items-center gap-2">
                <span className="field flex min-w-0 flex-1 items-center gap-2 px-2.5">
                  <Link2 size={15} className="shrink-0 text-ink-faint" />
                  <input
                    id="wechat-url"
                    type="url"
                    className="min-w-0 flex-1 bg-transparent text-xs text-ink outline-none"
                    value={url}
                    placeholder={t("wechat.urlPlaceholder")}
                    disabled={running}
                    onChange={(event) => setUrl(event.target.value)}
                  />
                </span>
              </div>
            </div>
          </Section>

          <Notice tone="info">{t("wechat.limitHint")}</Notice>
        </div>

        <aside className="flex min-w-0 flex-col gap-5">
          <Section title={t("feature.output")}>
            <OutputPicker api={output} disabled={running} />
          </Section>

          <div className="flex flex-col gap-3">
            <label className="flex cursor-pointer items-start gap-2 text-xs text-ink-muted select-none">
              <input
                type="checkbox"
                className="mt-0.5 accent-brand-600"
                checked={manual}
                disabled={running}
                onChange={(event) => setManual(event.target.checked)}
              />
              <span className="min-w-0 flex-1">
                <span className="font-medium text-ink">{t("wechat.manual")}</span>
                <span className="mt-0.5 block leading-relaxed">{t("wechat.manualHint")}</span>
              </span>
            </label>

            {running ? (
              <div className="card flex flex-col gap-2 p-4">
                <div className="flex items-center gap-2 text-sm text-ink">
                  <Loader2 size={16} className="animate-spin text-brand-500" />
                  <span className="flex-1 truncate">
                    {task?.message || t("wechat.printing")}
                  </span>
                </div>
                {manual ? (
                  <div className="flex flex-col gap-2 rounded-xl bg-surface-subtle p-3">
                    <p className="text-xs leading-relaxed text-ink-muted">{t("wechat.manualWaiting")}</p>
                    {confirmed ? (
                      <span className="flex items-center gap-1.5 text-xs text-ink-faint">
                        <CheckCircle2 size={13} />
                        {t("wechat.confirmed")}
                      </span>
                    ) : (
                      <button type="button" className="btn-primary self-start" onClick={() => void onConfirm()}>
                        <MousePointerClick size={15} />
                        {t("wechat.confirm")}
                      </button>
                    )}
                  </div>
                ) : null}
                <button type="button" className="btn-ghost self-start" onClick={() => void onCancel()}>
                  <Ban size={15} />
                  {t("common.cancel")}
                </button>
              </div>
            ) : null}

            {showSuccess ? (
              <div className="flex flex-col gap-3">
                <p className="flex items-center gap-2 text-sm text-emerald-600 dark:text-emerald-400">
                  <CheckCircle2 size={16} />
                  {t("wechat.done")}
                </p>
                <OutputList outputs={task?.outputs ?? []} />
              </div>
            ) : null}

            {showCancelled ? (
              <p className="flex items-center gap-2 text-xs text-ink-faint">
                <Ban size={14} />
                {t("feature.runCancelled")}
              </p>
            ) : null}

            {!running && failure ? (
              <div className="flex flex-col gap-2">
                <div className="flex items-start gap-2 rounded-2xl border border-rose-200 bg-rose-50 px-4 py-3 dark:border-rose-500/25 dark:bg-rose-500/10">
                  <XCircle size={16} className="mt-0.5 shrink-0 text-rose-500" />
                  <div className="min-w-0 flex-1">
                    <p className="text-sm font-medium text-rose-700 dark:text-rose-300">{failureTitle}</p>
                    {failure.detail ? (
                      <p className="mt-0.5 break-words text-xs text-rose-600/85 dark:text-rose-200/80">
                        {failure.detail}
                      </p>
                    ) : null}
                  </div>
                </div>
                {showRetryManual ? (
                  <button type="button" className="btn-ghost self-start" onClick={() => void startRun(true)}>
                    <MousePointerClick size={15} />
                    {t("wechat.retryManual")}
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
                  {t("wechat.export")}
                </button>
                {!canRun && hint ? <span className="text-xs text-ink-faint">{hint}</span> : null}
              </div>
            ) : null}
          </div>
        </aside>
      </div>
    </div>
  );
}
