import { Loader2 } from "lucide-react";
import { lazy, Suspense, type ReactElement } from "react";
import { Navigate, Route, Routes } from "react-router-dom";

import GenericFeaturePage from "@/components/GenericFeaturePage";
import { FEATURES, SYSTEM_PAGES } from "@/features/registry";
import type { FeatureDefinition } from "@/features/types";
import AppLayout from "@/layouts/AppLayout";

const HomePage = lazy(() => import("@/features/home/HomePage"));

/**
 * 路由表。
 *
 * 完全由注册表生成：新增功能只要在 `FEATURES` 里加一行，这里自动多出一条路由，
 * 没有自定义页面的功能落到 `GenericFeaturePage`。
 */
export function AppRouter(): ReactElement {
  return (
    <Routes>
      <Route element={<AppLayout />}>
        <Route
          index
          element={
            <Suspense fallback={<PageFallback />}>
              <HomePage />
            </Suspense>
          }
        />

        {FEATURES.map((feature) => (
          <Route
            key={feature.id}
            path={feature.route}
            element={
              <Suspense fallback={<PageFallback />}>
                <FeatureRoute feature={feature} />
              </Suspense>
            }
          />
        ))}

        {SYSTEM_PAGES.map((page) => {
          const Component = page.Page;
          return (
            <Route
              key={page.id}
              path={page.route}
              element={
                <Suspense fallback={<PageFallback />}>
                  <Component />
                </Suspense>
              }
            />
          );
        })}

        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  );
}

/** 有自定义页面就用它，否则走通用页。 */
function FeatureRoute({ feature }: { feature: FeatureDefinition }): ReactElement {
  const Custom = feature.Page;
  return Custom ? <Custom feature={feature} /> : <GenericFeaturePage feature={feature} />;
}

function PageFallback(): ReactElement {
  return (
    <div className="flex h-64 items-center justify-center text-ink-faint">
      <Loader2 size={18} className="animate-spin" />
    </div>
  );
}
