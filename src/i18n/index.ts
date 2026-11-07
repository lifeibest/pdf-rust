import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import enUS from "./locales/en-US";
import zhCN from "./locales/zh-CN";

export const LANGUAGES = [
  { code: "zh-CN", label: "简体中文" },
  { code: "en-US", label: "English" },
] as const;

export type LanguageCode = (typeof LANGUAGES)[number]["code"];

export const DEFAULT_LANGUAGE: LanguageCode = "zh-CN";

export function isLanguageCode(value: unknown): value is LanguageCode {
  return LANGUAGES.some((language) => language.code === value);
}

/**
 * `t()` 的收窄类型。
 *
 * 纯函数工具（如 `lib/format.ts`）需要接收翻译能力但不应依赖 React，
 * 用这个签名即可；`useTranslation()` 返回的 `t` 可以直接赋值给它。
 */
export type Translate = (key: string, options?: Record<string, unknown>) => string;

/**
 * 同步初始化，保证首屏渲染前 `t()` 就可用。
 *
 * 用户保存过的语言由 `store/settings.ts` 在读取到持久化配置后通过
 * `i18n.changeLanguage()` 应用，因此这里固定用默认语言起步。
 */
void i18n.use(initReactI18next).init({
  resources: {
    "zh-CN": { translation: zhCN },
    "en-US": { translation: enUS },
  },
  lng: DEFAULT_LANGUAGE,
  fallbackLng: DEFAULT_LANGUAGE,
  defaultNS: "translation",
  interpolation: {
    // React 已经做了转义，二次转义会让 `{{count}}` 之类的插值失效
    escapeValue: false,
  },
  returnNull: false,
});

export default i18n;
