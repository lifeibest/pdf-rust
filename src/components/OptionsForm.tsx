import clsx from "clsx";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { countScope, validateScope, type ScopeSelection } from "@/lib/pages";
import type { NumberField, OptionField, OptionValues, PageRangeField, SegmentedField, SelectField, SwitchField, TextField } from "@/features/types";

export interface OptionsFormProps {
  fields: OptionField[];
  values: OptionValues;
  onChange(key: string, value: OptionValues[string]): void;
  /** 当前文档总页数；页范围字段用它做即时校验与「将影响 N 页」提示 */
  pageCount?: number;
  disabled?: boolean;
}

/**
 * 依据 `FeatureDefinition.options` 自动渲染选项表单。
 *
 * 新增一种控件只要在这里补一个分支，所有功能页立刻可用 —— 这是
 * 「新增功能只写 meta.ts」能成立的前提之一。
 */
export function OptionsForm({
  fields,
  values,
  onChange,
  pageCount = 0,
  disabled = false,
}: OptionsFormProps): ReactElement | null {
  const { t } = useTranslation();
  const visible = fields.filter((field) => isVisible(field, values));
  if (visible.length === 0) return null;

  return (
    <fieldset disabled={disabled} className="flex flex-col gap-4">
      {visible.map((field) => (
        <div key={field.key} className="flex flex-col gap-1.5">
          {/* switch 自带文字状态，不需要额外 label */}
          {field.type === "switch" ? (
            <div className="flex items-center justify-between gap-3">
              <span className="label mb-0">{t(field.labelKey)}</span>
              <SwitchControl field={field} value={values[field.key]} onChange={onChange} />
            </div>
          ) : (
            <>
              <label className="label mb-0" htmlFor={`opt-${field.key}`}>
                {t(field.labelKey)}
              </label>
              <Control field={field} value={values[field.key]} onChange={onChange} pageCount={pageCount} />
            </>
          )}

          {field.hintKey ? (
            <p className="text-[11px] leading-relaxed text-ink-faint">{t(field.hintKey)}</p>
          ) : null}
        </div>
      ))}
    </fieldset>
  );
}

function isVisible(field: OptionField, values: OptionValues): boolean {
  if (!field.showIf) return true;
  return values[field.showIf.key] === field.showIf.value;
}

interface ControlProps<F extends OptionField> {
  field: F;
  value: OptionValues[string] | undefined;
  onChange: OptionsFormProps["onChange"];
  /** 只有页范围控件用得上，其余控件不必传 */
  pageCount?: number;
}

function Control<F extends OptionField>({
  field,
  value,
  onChange,
  pageCount,
}: ControlProps<F>): ReactElement {
  switch (field.type) {
    case "select":
      return <SelectControl field={field} value={value} onChange={onChange} />;
    case "segmented":
      return <SegmentedControl field={field} value={value} onChange={onChange} />;
    case "number":
      return <NumberControl field={field} value={value} onChange={onChange} />;
    case "text":
      return <TextControl field={field} value={value} onChange={onChange} />;
    case "switch":
      return <SwitchControl field={field} value={value} onChange={onChange} />;
    case "pageRange":
      return (
        <PageRangeControl field={field} value={value} onChange={onChange} pageCount={pageCount} />
      );
  }
}

function SelectControl({ field, value, onChange }: ControlProps<SelectField>): ReactElement {
  const { t } = useTranslation();
  return (
    <select
      id={`opt-${field.key}`}
      className="field"
      value={typeof value === "string" ? value : field.default}
      onChange={(event) => onChange(field.key, event.target.value)}
    >
      {field.options.map((option) => (
        <option key={option.value} value={option.value}>
          {t(option.labelKey)}
        </option>
      ))}
    </select>
  );
}

/** 分段单选：与页范围控件同一套视觉语言；选中档位的描述随切换淡入。 */
function SegmentedControl({ field, value, onChange }: ControlProps<SegmentedField>): ReactElement {
  const { t } = useTranslation();
  const current = typeof value === "string" ? value : field.default;
  const selected = field.options.find((option) => option.value === current);
  return (
    <div className="flex flex-col gap-1.5">
      <div
        id={`opt-${field.key}`}
        role="radiogroup"
        aria-label={t(field.labelKey)}
        className="grid gap-1 rounded-xl bg-surface-subtle p-1"
        style={{ gridTemplateColumns: `repeat(${field.options.length}, minmax(0, 1fr))` }}
      >
        {field.options.map((option) => (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={option.value === current}
            onClick={() => onChange(field.key, option.value)}
            className={clsx(
              "rounded-lg px-2 py-1.5 text-xs font-medium transition-colors",
              option.value === current
                ? "bg-surface text-brand-600 shadow-card"
                : "text-ink-muted hover:text-ink",
            )}
          >
            {t(option.labelKey)}
          </button>
        ))}
      </div>
      {selected?.hintKey ? (
        <p
          key={selected.value}
          className="animate-fade-in text-[11px] leading-relaxed text-ink-faint"
        >
          {t(selected.hintKey)}
        </p>
      ) : null}
    </div>
  );
}

function NumberControl({ field, value, onChange }: ControlProps<NumberField>): ReactElement {
  const { t } = useTranslation();
  return (
    <div className="flex items-center gap-2">
      <input
        id={`opt-${field.key}`}
        type="number"
        className="field"
        min={field.min}
        max={field.max}
        step={field.step ?? 1}
        value={typeof value === "number" ? value : field.default}
        onChange={(event) => {
          const raw = Number.parseFloat(event.target.value);
          // 清空输入框时不回写，否则 NaN 会污染表单值
          if (Number.isFinite(raw)) {
            onChange(field.key, Math.min(field.max, Math.max(field.min, raw)));
          }
        }}
      />
      {field.unitKey ? (
        <span className="shrink-0 text-xs text-ink-faint">{t(field.unitKey)}</span>
      ) : null}
    </div>
  );
}

function TextControl({ field, value, onChange }: ControlProps<TextField>): ReactElement {
  const { t } = useTranslation();
  return (
    <input
      id={`opt-${field.key}`}
      type="text"
      className="field font-mono text-xs"
      value={typeof value === "string" ? value : field.default}
      placeholder={field.placeholderKey ? t(field.placeholderKey) : undefined}
      onChange={(event) => onChange(field.key, event.target.value)}
    />
  );
}

function SwitchControl({ field, value, onChange }: ControlProps<SwitchField>): ReactElement {
  const { t } = useTranslation();
  const checked = typeof value === "boolean" ? value : field.default;
  return (
    <Switch
      id={`opt-${field.key}`}
      checked={checked}
      onChange={(next) => onChange(field.key, next)}
      label={checked ? t("common.on") : t("common.off")}
    />
  );
}

const SCOPE_KINDS: { kind: ScopeSelection["kind"]; labelKey: string }[] = [
  { kind: "all", labelKey: "options.scopeAll" },
  { kind: "odd", labelKey: "options.scopeOdd" },
  { kind: "even", labelKey: "options.scopeEven" },
  { kind: "spec", labelKey: "options.scopeSpec" },
];

function PageRangeControl({
  field,
  value,
  onChange,
  pageCount = 0,
}: ControlProps<PageRangeField>): ReactElement {
  const { t } = useTranslation();
  const selection: ScopeSelection =
    typeof value === "object" && value !== null && "kind" in value
      ? (value as ScopeSelection)
      : { ...field.default };

  const reason = pageCount > 0 ? validateScope(selection, pageCount) : null;
  const affected = pageCount > 0 ? countScope(selection, pageCount) : 0;

  return (
    <div className="flex flex-col gap-2">
      <div className="grid grid-cols-4 gap-1 rounded-xl bg-surface-subtle p-1" id={`opt-${field.key}`}>
        {SCOPE_KINDS.map((item) => (
          <button
            key={item.kind}
            type="button"
            onClick={() => onChange(field.key, { ...selection, kind: item.kind })}
            className={clsx(
              "rounded-lg px-2 py-1.5 text-xs font-medium transition-colors",
              selection.kind === item.kind
                ? "bg-surface text-brand-600 shadow-card"
                : "text-ink-muted hover:text-ink",
            )}
          >
            {t(item.labelKey)}
          </button>
        ))}
      </div>

      {selection.kind === "spec" ? (
        <input
          type="text"
          className="field font-mono text-xs"
          value={selection.spec}
          placeholder={field.placeholderKey ? t(field.placeholderKey) : t("options.specPlaceholder")}
          onChange={(event) => onChange(field.key, { ...selection, spec: event.target.value })}
        />
      ) : null}

      {pageCount > 0 ? (
        <p className={clsx("text-[11px]", reason ? "text-rose-500" : "text-ink-faint")}>
          {reason
            ? t(`errors.rangeReason.${reason}`)
            : t("common.affectedPages", { count: affected })}
        </p>
      ) : null}
    </div>
  );
}

export interface SwitchProps {
  id?: string;
  checked: boolean;
  onChange(next: boolean): void;
  /** 开关右侧的状态文字 */
  label?: string;
  disabled?: boolean;
}

/** 通用开关控件，设置页同样复用。 */
export function Switch({
  id,
  checked,
  onChange,
  label,
  disabled = false,
}: SwitchProps): ReactElement {
  return (
    <span className="inline-flex items-center gap-2">
      {label ? <span className="text-xs text-ink-muted">{label}</span> : null}
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={clsx(
          "relative inline-flex h-6 w-11 shrink-0 items-center rounded-full px-0.5",
          "transition-colors duration-150 disabled:opacity-50",
          checked ? "bg-brand-500" : "bg-line",
        )}
      >
        <span
          className={clsx(
            "h-5 w-5 rounded-full bg-white shadow-sm transition-transform duration-150",
            checked ? "translate-x-5" : "translate-x-0",
          )}
        />
      </button>
    </span>
  );
}
