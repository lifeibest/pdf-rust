import { useEffect } from "react";
import { isPermissionGranted, requestPermission } from "@tauri-apps/plugin-notification";

import { useSettings } from "@/store/settings";
import { useTasks } from "@/store/tasks";
import { useApp } from "@/store/app";

/**
 * 应用级启动流程：读取持久化设置、读取应用信息、订阅任务事件、申请通知权限。
 *
 * 只应在根组件调用一次。store 内部都做了重入保护，
 * 因此 StrictMode 下的双次执行不会产生重复订阅。
 */
export function useAppBootstrap(): void {
  const loadSettings = useSettings((state) => state.load);
  const bootstrapTasks = useTasks((state) => state.bootstrap);
  const loadApp = useApp((state) => state.load);
  const notifyOnFinish = useSettings((state) => state.settings.notifyOnFinish);

  useEffect(() => {
    void loadSettings();
    void bootstrapTasks();
    void loadApp();
  }, [loadSettings, bootstrapTasks, loadApp]);

  useEffect(() => {
    if (!notifyOnFinish) return;
    // 通知由后端在任务结束时发送，但权限必须由前端申请一次
    void isPermissionGranted()
      .then((granted) => (granted ? null : requestPermission()))
      .catch(() => null);
  }, [notifyOnFinish]);
}
