/**
 * 开源许可清单。
 *
 * 组件名、许可证与主页 URL 都是专有名词，不进 i18n；分组标题走 key。
 * 版本号只记录主版本（与 `Cargo.toml` / `package.json` 的声明一致），
 * 精确到补丁号的版本由构建时的 lockfile 决定，写死在这里反而会过期。
 */

export type LicenseGroup = "engine" | "backend" | "frontend";

export interface LicenseEntry {
  name: string;
  /** 声明在依赖清单里的版本范围 */
  version: string;
  license: string;
  url: string;
  group: LicenseGroup;
}

export const LICENSE_GROUPS: { id: LicenseGroup; titleKey: string }[] = [
  { id: "engine", titleKey: "about.licenseGroupEngine" },
  { id: "backend", titleKey: "about.licenseGroupBackend" },
  { id: "frontend", titleKey: "about.licenseGroupFrontend" },
];

export const LICENSES: LicenseEntry[] = [
  {
    name: "PDFium",
    version: "chromium/8125",
    license: "BSD-3-Clause",
    url: "https://pdfium.googlesource.com/pdfium/",
    group: "engine",
  },
  {
    name: "pdfium-binaries",
    version: "chromium/8125",
    license: "BSD-3-Clause",
    url: "https://github.com/bblanchon/pdfium-binaries",
    group: "engine",
  },
  {
    name: "pdfium-render",
    version: "0.9",
    license: "MIT OR Apache-2.0",
    url: "https://github.com/ajrcarey/pdfium-render",
    group: "engine",
  },

  {
    name: "Tauri",
    version: "2",
    license: "MIT OR Apache-2.0",
    url: "https://tauri.app/",
    group: "backend",
  },
  {
    name: "image",
    version: "0.25",
    license: "MIT OR Apache-2.0",
    url: "https://github.com/image-rs/image",
    group: "backend",
  },
  {
    name: "zip",
    version: "9",
    license: "MIT",
    url: "https://github.com/zip-rs/zip2",
    group: "backend",
  },
  {
    name: "serde",
    version: "1",
    license: "MIT OR Apache-2.0",
    url: "https://serde.rs/",
    group: "backend",
  },
  {
    name: "tokio",
    version: "1",
    license: "MIT",
    url: "https://tokio.rs/",
    group: "backend",
  },
  {
    name: "thiserror",
    version: "2",
    license: "MIT OR Apache-2.0",
    url: "https://github.com/dtolnay/thiserror",
    group: "backend",
  },
  {
    name: "uuid",
    version: "1",
    license: "MIT OR Apache-2.0",
    url: "https://github.com/uuid-rs/uuid",
    group: "backend",
  },

  {
    name: "React",
    version: "19",
    license: "MIT",
    url: "https://react.dev/",
    group: "frontend",
  },
  {
    name: "React Router",
    version: "7",
    license: "MIT",
    url: "https://reactrouter.com/",
    group: "frontend",
  },
  {
    name: "Vite",
    version: "8",
    license: "MIT",
    url: "https://vite.dev/",
    group: "frontend",
  },
  {
    name: "TypeScript",
    version: "5.9",
    license: "Apache-2.0",
    url: "https://www.typescriptlang.org/",
    group: "frontend",
  },
  {
    name: "Tailwind CSS",
    version: "3.4",
    license: "MIT",
    url: "https://tailwindcss.com/",
    group: "frontend",
  },
  {
    name: "Zustand",
    version: "5",
    license: "MIT",
    url: "https://github.com/pmndrs/zustand",
    group: "frontend",
  },
  {
    name: "i18next",
    version: "23",
    license: "MIT",
    url: "https://www.i18next.com/",
    group: "frontend",
  },
  {
    name: "react-i18next",
    version: "15",
    license: "MIT",
    url: "https://react.i18next.com/",
    group: "frontend",
  },
  {
    name: "dnd-kit",
    version: "6",
    license: "MIT",
    url: "https://dndkit.com/",
    group: "frontend",
  },
  {
    name: "Lucide",
    version: "0.469",
    license: "ISC",
    url: "https://lucide.dev/",
    group: "frontend",
  },
];

export function licensesByGroup(group: LicenseGroup): LicenseEntry[] {
  return LICENSES.filter((entry) => entry.group === group);
}
