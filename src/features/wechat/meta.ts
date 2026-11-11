import { MessageSquareText } from "lucide-react";
import { lazy } from "react";

import type { FeatureDefinition } from "@/features/types";

/**
 * 微信公众号文章导出 PDF。
 *
 * 与其它功能不同：来源是文章 URL 而非本地文件，且打印由内嵌 WebView 在 UI 线程发起，
 * 因此走 `mode: "custom"` 的专用命令 `export_wechat_pdf`，自带页面直接编排
 * 输入 / 输出 / 进度 / 取消，不进入统一 `run_task` 管线，也没有 `buildRequest`。
 *
 * `accepts.ext` 为空：全局拖拽按扩展名匹配，本功能不会被文件拖入误命中。
 */
const wechat: FeatureDefinition = {
  id: "wechat",
  route: "/wechat",
  titleKey: "features.wechat.title",
  descKey: "features.wechat.desc",
  icon: MessageSquareText,
  accent: "from-emerald-500 to-teal-500",
  category: "convert",
  order: 40,
  accepts: { ext: [], multiple: false },
  options: [],
  needsOutputDir: true,
  mode: "custom",
  sortEnabled: false,
  thumbEnabled: false,
  mixedSources: false,
  Page: lazy(() => import("./index")),
};

export default wechat;
