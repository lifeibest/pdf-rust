import { HashRouter } from "react-router-dom";
import type { ReactElement } from "react";

import { useAppBootstrap } from "@/hooks/useAppBootstrap";
import { useTheme } from "@/hooks/useTheme";
import { AppRouter } from "@/router";

/**
 * 应用根组件。
 *
 * 用 `HashRouter` 而不是 `BrowserRouter`：打包后页面由 `tauri://` 自定义协议提供，
 * 深链接（如 `/merge`）在服务端没有对应资源，hash 路由不依赖服务端重写。
 */
export default function App(): ReactElement {
  useAppBootstrap();
  useTheme();

  return (
    <HashRouter>
      <AppRouter />
    </HashRouter>
  );
}
