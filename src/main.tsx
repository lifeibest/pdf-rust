import React from "react";
import ReactDOM from "react-dom/client";

// i18n 必须在任何组件渲染前初始化，否则首屏拿不到文案
import "@/i18n";
import "@/styles/globals.css";

import App from "@/App";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
