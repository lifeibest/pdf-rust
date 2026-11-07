import { useEffect } from "react";

import { useSettings } from "@/store/settings";
import i18n from "@/i18n";

/** 系统偏好变化时重新计算深浅色。 */
const MEDIA_QUERY = "(prefers-color-scheme: dark)";

/**
 * 把设置里的主题与语言应用到 DOM。
 *
 * Tailwind 用的是 `darkMode: "class"`，因此只需在 `<html>` 上增删 `dark`；
 * CSS 变量随之切换，组件里不必到处写 `dark:` 前缀。
 */
export function useTheme(): void {
  const theme = useSettings((state) => state.settings.theme);
  const language = useSettings((state) => state.settings.language);

  useEffect(() => {
    const root = document.documentElement;

    const apply = (dark: boolean) => root.classList.toggle("dark", dark);

    if (theme !== "system") {
      apply(theme === "dark");
      return;
    }

    const media = window.matchMedia(MEDIA_QUERY);
    apply(media.matches);
    const onChange = (event: MediaQueryListEvent) => apply(event.matches);
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [theme]);

  useEffect(() => {
    document.documentElement.lang = language;
    if (i18n.language !== language) void i18n.changeLanguage(language);
  }, [language]);
}
