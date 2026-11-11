import { Outlet } from "react-router-dom";
import { useEffect, type ReactElement } from "react";

import { DropOverlay } from "@/components/DropOverlay";
import { FeatureChooser } from "@/components/FeatureChooser";
import { ActivationModal } from "@/components/ActivationModal";
import { TaskDrawer } from "@/components/TaskDrawer";
import { useDropFiles } from "@/hooks/useDropFiles";
import { useQuota } from "@/store/quota";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";

/**
 * 应用主布局：左侧固定侧边栏 + 右侧「顶栏 / 内容区」。
 *
 * 任务抽屉、拖拽遮罩、功能选择器都挂在这一层，因此它们与当前路由无关，
 * 在任何页面上都能出现。
 */
export default function AppLayout(): ReactElement {
  // 全局窗口拖拽：必须在 Router 内部（用到 navigate）
  useDropFiles();

  // 启动即拉取一次授权状态，让侧边栏 / 顶栏重启后也能区分「获取激活码」与「VIP 会员」。
  // （设备登记已合并到激活时的联网兑换，不再需要后台静默上报与周期补报。）
  useEffect(() => {
    void useQuota.getState().refresh();
  }, []);

  return (
    <div className="flex h-full overflow-hidden bg-surface-muted">
      <Sidebar />

      <div className="flex min-w-0 flex-1 flex-col">
        <TopBar />
        <main className="min-h-0 flex-1 overflow-y-auto">
          <div className="mx-auto w-full max-w-6xl px-6 py-6">
            <Outlet />
          </div>
        </main>
      </div>

      <TaskDrawer />
      <DropOverlay />
      <FeatureChooser />
      <ActivationModal />
    </div>
  );
}
