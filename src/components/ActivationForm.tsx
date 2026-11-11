import { CheckCircle2, KeyRound, Loader2 } from "lucide-react";
import { useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { Notice } from "@/components/Notice";
import { useQuota } from "@/store/quota";

interface ActivationFormProps {
  /** 激活成功后的额外回调（如关闭弹窗） */
  onActivated?: () => void;
}

/** 把 store 里的错误哨兵值/后端原文统一成可读文案。 */
function errorText(error: string | null, t: (key: string) => string): string | null {
  if (!error) return null;
  // 超限单独用 warn 提示条渲染（需插值 devices/cap），此处不重复作为错误文案。
  if (error === "overLimit") return null;
  if (error === "empty") return t("activate.errEmpty");
  if (error === "licenseInvalid") return t("activate.errInvalid");
  if (error === "redeemNotFound") return t("activate.errRedeemNotFound");
  if (error === "redeemNetwork") return t("activate.errRedeemNetwork");
  return error;
}

/**
 * 激活码录入表单：输入 → 离线验签 → 落盘。
 *
 * 被「配额用尽弹窗」与「获取激活码页」共用，保证两处行为一致。
 */
export function ActivationForm({ onActivated }: ActivationFormProps): ReactElement {
  const { t } = useTranslation();
  const [code, setCode] = useState("");
  const [done, setDone] = useState(false);

  const loading = useQuota((state) => state.loading);
  const error = useQuota((state) => state.error);
  const overLimit = useQuota((state) => state.overLimit);
  const activate = useQuota((state) => state.activate);

  async function handleSubmit(): Promise<void> {
    setDone(false);
    const ok = await activate(code);
    if (ok) {
      setDone(true);
      setCode("");
      onActivated?.();
    }
  }

  const message = errorText(error, t);

  return (
    <div className="flex flex-col gap-3">
      <label className="label" htmlFor="activation-code">
        {t("activate.codeLabel")}
      </label>
      <div className="flex gap-2">
        <input
          id="activation-code"
          className="field min-w-0 flex-1 border-brand-300/70 bg-brand-500/[0.06] font-mono text-xs shadow-[inset_0_1px_2px_rgba(0,0,0,0.03)] dark:border-brand-500/40 dark:bg-brand-500/10"
          placeholder={t("activate.codePlaceholder")}
          value={code}
          spellCheck={false}
          autoComplete="off"
          onChange={(event) => {
            // 短码自动转大写方便抄录；含点号的是完整签名 token（base64url 大小写敏感），保持原样。
            const next = event.target.value.includes(".")
              ? event.target.value
              : event.target.value.toUpperCase();
            setCode(next);
            setDone(false);
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !loading) void handleSubmit();
          }}
        />
        <button
          type="button"
          className="btn-primary shrink-0"
          disabled={loading || code.trim().length === 0}
          onClick={() => void handleSubmit()}
        >
          {loading ? <Loader2 size={15} className="animate-spin" /> : <KeyRound size={15} />}
          {t("activate.activate")}
        </button>
      </div>

      {done ? (
        <Notice tone="success">
          <span className="flex items-center gap-2">
            <CheckCircle2 size={14} />
            {t("activate.success")}
          </span>
        </Notice>
      ) : null}

      {error === "overLimit" && overLimit ? (
        <Notice tone="warn">
          {t("activate.errOverLimit", { devices: overLimit.devices, cap: overLimit.cap })}
        </Notice>
      ) : null}

      {message ? <Notice tone="error">{message}</Notice> : null}
    </div>
  );
}
