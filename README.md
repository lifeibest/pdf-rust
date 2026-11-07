# PDF智转

本地运行、不上传的 macOS PDF 工具箱。合并、拆分、旋转、PDF 转图片、图片转 PDF
五大功能全部由内嵌的 [PDFium](https://pdfium.googlesource.com/pdfium/) 引擎在 Rust 侧
完成，文件不会离开你的电脑。

- **技术栈**：Tauri 2 + Rust（`pdfium-render`）+ React 19 + TypeScript + Tailwind CSS
- **平台**：macOS 10.15+（本期只产出 `.app` / `.dmg`，ad-hoc 签名）
- **语言**：简体中文 / English，深浅色主题

## 功能

| 功能 | 说明 |
| --- | --- |
| PDF 合并 | 多个 PDF 与图片按顺序合并为一个文件，可拖拽排序、预览顺序 |
| PDF 拆分 | 每 N 页 / 每页一个 / 按范围多个 / 提取指定页，可打包 zip |
| PDF 旋转 | 90/180/270°，支持奇偶页与指定页码，可原地覆盖或另存 |
| PDF 转图片 | 按页导出 PNG / JPEG / WebP，可调 DPI、质量、最长边，可打包 zip |
| 图片转 PDF | 批量图片排版为 PDF，可选纸张、方向、边距与适配方式 |

通用能力：全局拖拽自动匹配功能、加密 PDF 密码解锁、页面缩略图预览、
页范围表达式（`1,3-5,8` / `first:3` / `last:2` / `odd` / `even`）、
任务中心（进度 / 取消 / 历史记录）、系统通知、输出目录策略记忆。

## 快速开始

```bash
pnpm install          # 安装前端依赖
pnpm pdfium           # 下载 libpdfium.dylib（首次必需，之后自动跳过）
pnpm tauri dev        # 开发模式（Vite + 热重载 + Rust 增量编译）
```

打包：

```bash
pnpm tauri build      # 产出 src-tauri/target/release/bundle/{macos,dmg}
pnpm sign:macos       # 对 .app 与内嵌 dylib 做 ad-hoc 签名
```

完整构建、签名与 Windows 预留步骤见 [docs/BUILD.md](docs/BUILD.md)。

## 常用脚本

| 命令 | 作用 |
| --- | --- |
| `pnpm dev` | 仅启动 Vite 前端 |
| `pnpm tauri dev` | 完整开发模式 |
| `pnpm typecheck` | `tsc --noEmit` |
| `pnpm test:rust` | Rust 单测 + 集成测试 |
| `pnpm lint:rust` | `cargo clippy -- -D warnings` |
| `pnpm pdfium` / `pnpm pdfium:force` | 下载 / 强制重新下载 PDFium 运行库 |
| `pnpm sign:macos` | 打包后 ad-hoc 签名 |

## 目录速览

```
src/                  前端（React）
  features/<id>/      每个功能一个目录：meta.ts（注册）+ index.tsx（可选自定义页）
  components/         通用组件（DropZone / FileList / OptionsForm / TaskProgress …）
  hooks/              useFeatureController / useTaskRunner / useSourceFiles …
  store/              zustand：settings / tasks / drop / app
  lib/tauri.ts        后端契约的 TypeScript 镜像
src-tauri/src/
  pdf/                五大功能的 Rust 实现 + engine / ranges / thumbnails
  commands/           Tauri 命令层（tasks / pdf / fs / app）
  tests/              集成测试（代码生成 fixture PDF，不提交二进制）
```

架构与扩展方式见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)，
新增一个功能需要改哪几处见 [docs/ADD-FEATURE.md](docs/ADD-FEATURE.md)。

## 手动验收清单

dev 与打包后各跑一遍：

- [ ] 5 个功能各 3 组用例：常规输入、加密 PDF（输入密码后能预览与处理）、
      超大 PDF（≥200 页，缩略图只显示前 12 页且提示截断）
- [ ] 合并页拖拽排序后输出顺序一致；顺序预览条随排序变化
- [ ] 页范围 `1,3-5` 即时显示「将影响 4 页」；越界输入给出原因且禁用运行按钮
- [ ] 任务运行中可取消，任务中心记录为「已取消」
- [ ] 任务完成发送系统通知（设置中可关）
- [ ] 产出列表的「打开」与「打开所在目录」均可用
- [ ] 设置项（语言 / 主题 / 输出策略 / DPI / 缩略图尺寸）重启后保持
- [ ] 深浅色主题与中英文切换后无遗漏文案（含相对时间、耗时）
- [ ] 关于页版本、Tauri 版本、PDFium 状态与搜索路径正确；「重新检测」可用
- [ ] dmg 安装后双击可启动且能成功渲染（验证 dylib 签名与 resource 路径解析）
- [ ] 清理缓存在「设置 → 存储与缓存」可用，重复点击提示「没有需要清理的内容」

## 已知限制

- PDFium 跨文档复制页面不会带过表单域（Interactive Form）、部分注释类型与嵌入附件；
  合并提供「压平注释与表单」开关作为缓解。
- PDFium 保存为全量重写（非增量），输出体积可能与原文件有差异。
- 加密 PDF 仅支持用用户密码解密后处理，不支持添加密码 / 权限保护。
- 本期不做 Apple 公证与自动更新；ad-hoc 签名产物仅适合本机与团队内分发。

## 开源许可

本项目基于 PDFium（BSD-3-Clause）、pdfium-render、Tauri、React、Tailwind CSS 等
开源组件构建，完整清单见应用内「关于 → 开源许可」。
