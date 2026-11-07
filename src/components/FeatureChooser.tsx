import { X } from "lucide-react";
import { useNavigate } from "react-router-dom";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { useDrop } from "@/store/drop";
import type { FeatureId } from "@/features/types";

/**
 * 拖入的文件同时匹配多个功能时的选择器。
 *
 * 例如拖入一堆图片既可以做「图片转 PDF」也可以参与「PDF 合并」，
 * 这时不猜用户意图，直接让他选。
 */
export function FeatureChooser(): ReactElement | null {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const chooser = useDrop((state) => state.chooser);
  const closeChooser = useDrop((state) => state.closeChooser);
  const setPending = useDrop((state) => state.setPending);

  if (!chooser) return null;

  const choose = (feature: FeatureId, route: string) => {
    setPending({ feature, paths: chooser.paths });
    closeChooser();
    navigate(route);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6 backdrop-blur-[2px]">
      <div className="w-full max-w-md overflow-hidden rounded-2xl border border-line bg-surface shadow-panel animate-slide-up">
        <header className="flex items-center gap-2 border-b border-line px-4 py-3">
          <h2 className="flex-1 text-sm font-semibold text-ink">{t("home.droppedPickFeature")}</h2>
          <button
            type="button"
            className="btn-quiet h-7 w-7 rounded-lg px-0 py-0"
            onClick={closeChooser}
            aria-label={t("common.close")}
            title={t("common.close")}
          >
            <X size={15} />
          </button>
        </header>

        <ul className="flex flex-col gap-1 p-3">
          {chooser.candidates.map((feature) => {
            const Icon = feature.icon;
            return (
              <li key={feature.id}>
                <button
                  type="button"
                  onClick={() => choose(feature.id, feature.route)}
                  className="flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left transition-colors hover:bg-surface-subtle"
                >
                  <span
                    className={`inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br text-white ${feature.accent}`}
                  >
                    <Icon size={17} />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm font-medium text-ink">
                      {t(feature.titleKey)}
                    </span>
                    <span className="block truncate text-xs text-ink-faint">
                      {t(feature.descKey)}
                    </span>
                  </span>
                </button>
              </li>
            );
          })}
        </ul>

        <footer className="border-t border-line px-4 py-2.5 text-[11px] text-ink-faint">
          {t("common.totalSize", {
            size: `${chooser.paths.length} ${t("common.items")}`,
          })}
        </footer>
      </div>
    </div>
  );
}
