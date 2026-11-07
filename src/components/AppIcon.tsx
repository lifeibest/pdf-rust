import clsx from "clsx";
import { FileText } from "lucide-react";

export interface AppIconProps {
  /** 边长（像素），默认 40 */
  size?: number;
  className?: string;
}

/**
 * 应用标识。
 *
 * 打包后 `.icns` 由 `tauri.conf.json` 指定，这里只负责界面内的品牌呈现，
 * 两者共用同一套靛蓝渐变，视觉上保持一致。
 */
export function AppIcon({ size = 40, className }: AppIconProps) {
  return (
    <span
      className={clsx(
        "inline-flex shrink-0 items-center justify-center rounded-2xl",
        "bg-gradient-to-br from-brand-500 to-violet-500 text-white shadow-card",
        className,
      )}
      style={{ width: size, height: size }}
      aria-hidden
    >
      <FileText size={Math.round(size * 0.52)} strokeWidth={2.2} />
    </span>
  );
}
