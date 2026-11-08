# 构建与发布

本文覆盖 PDF智转 从**环境准备**到**本地构建**、**代码签名**、**CI 自动构建**、**正式发版**的完整流程。
架构与模块划分见 [ARCHITECTURE.md](./ARCHITECTURE.md)，新增功能见 [ADD-FEATURE.md](./ADD-FEATURE.md)。

> 关键事实：所有 PDF 计算在 Rust 侧完成，运行时**动态加载** PDFium 动态库（不静态链接）。
> 因此每个平台构建时都必须先备好对应平台的 PDFium 库文件。

---

## 1. 环境准备

| 工具 | 版本 | 说明 |
|---|---|---|
| Node.js | 20+ | 前端构建 |
| pnpm | 9 | 唯一包管理器（CI 用 `--frozen-lockfile`，务必提交 `pnpm-lock.yaml`） |
| Rust | stable（≥ 1.90，edition 2024） | 后端与 PDF 计算 |
| Xcode Command Line Tools | 最新 | 仅 macOS：`codesign` / `hdiutil` / 链接系统库 |
| WebView2 Runtime | — | 仅 Windows：Win10/11 一般已内置，NSIS 安装时会自动引导 |

macOS 打**通用二进制**（Intel + Apple Silicon）需要两个 target：

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

---

## 2. PDFium 引擎下载

引擎库不入库（`src-tauri/binaries/` 已在 `.gitignore`），由脚本按需下载到该目录。

```bash
pnpm pdfium                 # 下载当前平台所需的库（已就位则跳过）
pnpm pdfium:force           # 强制重新下载
node scripts/download-pdfium.mjs --platform win32 --arch x64   # 交叉准备其它平台
```

- 版本与 `src-tauri/Cargo.toml` 里 `pdfium-render` 绑定的 `chromium/8125` 对齐，记录在
  `src-tauri/binaries/PDFIUM_VERSION.json`。
- 各平台产物文件名（Rust 侧 `engine::library_file_name()` 按平台选取）：

  | 平台 | 文件名 | 备注 |
  |---|---|---|
  | macOS | `libpdfium.dylib` | 下载的是 **mac-univ 通用库**，一份同时含 arm64 + x86_64 |
  | Windows | `pdfium.dll` | x64 |
  | Linux | `libpdfium.so` | 当前不在打包范围 |

- **`pretauri` 钩子**：`package.json` 里 `pretauri` 会在任何 `pnpm tauri …` 命令前自动跑一次
  `download-pdfium.mjs`，所以本地开发/构建无需手动下载。CI 里则显式执行 `pnpm pdfium` 以便尽早失败。

---

## 3. 本地开发

```bash
pnpm tauri dev
```

- 提供 HMR：改前端即时刷新，改 Rust 自动重编重启。
- ⚠️ 不要用纯 `pnpm dev`（只起 Vite 到 `localhost:1420`）：浏览器里 Tauri `invoke` 全失效，
  所有 PDF 功能都用不了。`pnpm dev` 仅供前端纯 UI 调试。

---

## 4. 本地构建

```bash
pnpm tauri build                       # 按当前平台打包（macOS: .app + .dmg）
pnpm tauri build --bundles app         # 只打 .app，跳过 DMG，迭代更快
pnpm build:mac-universal               # macOS 通用二进制（arm64 + x86_64）
```

- `pnpm build:mac-universal` 等价于 `pnpm tauri build --target universal-apple-darwin`，
  需先 `rustup target add x86_64-apple-darwin`。
- 通用二进制产物比单架构大（约 30 MiB vs 22 MiB），因为含两份架构的可执行体。
- 验证是否真为通用：

  ```bash
  lipo -info "src-tauri/target/universal-apple-darwin/release/bundle/macos/PDF智转.app/Contents/MacOS/pdfai-cn"
  # 期望：Architectures in the fat file: ... are: x86_64 arm64
  ```

### 产物路径

| 平台 / target | 路径 |
|---|---|
| macOS（本机架构） | `src-tauri/target/release/bundle/{macos,dmg}/` |
| macOS（universal） | `src-tauri/target/universal-apple-darwin/release/bundle/{macos,dmg}/` |
| Windows | `src-tauri/target/release/bundle/nsis/*.exe` |

---

## 5. 平台配置拆分

通用配置在 `src-tauri/tauri.conf.json`；平台专属的 `targets` / `resources` / 安装包参数拆分到
`tauri.<platform>.conf.json`，**Tauri CLI 按构建平台自动合并**。这样每个平台只引用自己那份
PDFium 库，不会因缺失其它平台的库而构建失败。

| 文件 | 内容 |
|---|---|
| `tauri.conf.json` | `identifier`(`cn.pdfai.desktop`)、`productName`(PDF智转)、包名(`pdfai-cn`)、分类、图标、CSP、asset scope |
| `tauri.macos.conf.json` | `targets: ["app","dmg"]`、`resources` 拷入 `libpdfium.dylib`、最低系统 `10.15`、`signingIdentity`、DMG 布局 |
| `tauri.windows.conf.json` | `targets: ["nsis"]`、`resources` 拷入 `pdfium.dll`、`webviewInstallMode: downloadBootstrapper`、NSIS `currentUser`（免 UAC）、简中/英文界面 |

> 改通用项（如版本号、identifier）改 `tauri.conf.json`；改某平台打包行为改对应的 `tauri.<platform>.conf.json`。

---

## 6. macOS 签名与公证

### 现状：ad-hoc 签名

`tauri.macos.conf.json` 里 `signingIdentity: null`，Tauri 不做正式签名。构建后用脚本做 ad-hoc 签名：

```bash
pnpm sign:macos                 # 自动找最新构建的 .app 做 ad-hoc 签名
pnpm sign:macos path/to/X.app   # 指定 .app
pnpm sign:macos --dmg           # 签名后用已签名的 .app 重建 dmg
```

脚本 [`scripts/sign-macos.sh`](../scripts/sign-macos.sh) 的关键点：**必须先签内嵌 dylib，再整体签 .app**。
因为 `libpdfium.dylib` 是运行时 `dlopen` 加载的，没有有效签名的动态库会被 Gatekeeper/AMFI 拒绝加载，
表现为「应用能启动，但所有 PDF 功能报 `pdfiumNotFound`」。

ad-hoc 产物只适合本机与团队内分发；分发给他人首次打开需在「系统设置 → 隐私与安全性」放行。

### 正式签名 + 公证（需付费 Apple Developer 账号）

需要 **Developer ID Application** 证书（不是 Apple Development / Apple Distribution）。

**签名参数：**

| 变量 | 含义 | 获取方式 |
|---|---|---|
| `APPLE_SIGNING_IDENTITY` | 如 `Developer ID Application: 公司名 (TEAMID)` | `security find-identity -v -p codesigning` |
| `APPLE_CERTIFICATE` | `.p12` 的 base64（仅 CI） | 钥匙串导出 `.p12` → `openssl base64 -A -in cert.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | 导出 `.p12` 时设的密码（仅 CI） | 导出时自定义 |

**公证参数（二选一）：**

- Apple ID 方式：`APPLE_ID`（账号邮箱）、`APPLE_PASSWORD`（[appleid.apple.com](https://appleid.apple.com) 生成的 App 专用密码）、`APPLE_TEAM_ID`（Membership 页）
- App Store Connect API Key 方式（CI 更稳）：`APPLE_API_ISSUER`、`APPLE_API_KEY`、`APPLE_API_KEY_PATH`（`.p8` 私钥）

配好环境变量后重跑 `pnpm tauri build`，Tauri 会自动签名 + 提交公证 + `stapler` 装订。

> ⚠️ 本项目两个坑：
> 1. 内嵌的 `libpdfium.dylib` 也必须用**同一个 Developer ID** 签名并一起公证，漏签会被拒。
> 2. 因开了 hardened runtime 且动态加载第三方库，通常需要 entitlements 加
>    `com.apple.security.cs.disable-library-validation`，并在 `tauri.macos.conf.json` 的
>    `entitlements` 指向该 `.plist`（当前为 `null`）。

---

## 7. CI 自动构建（GitHub Actions）

Tauri/Rust **不能交叉编译**，Windows 安装包必须在 Windows 上构建。
[`.github/workflows/build.yml`](../.github/workflows/build.yml) 用云端矩阵一次产出 mac + Windows 两个平台。

### 触发方式

- 推送 `v*` 标签（如 `v0.1.0`）→ 构建并自动创建 **Release 草稿**
- Actions 页手动 **Run workflow**（`workflow_dispatch`）→ 仅构建，产物存为 **Artifact**（不发版）

### 矩阵与步骤

| 平台 | runner | 构建参数 | 产物 |
|---|---|---|---|
| macOS | `macos-latest` | `--target universal-apple-darwin` | `.dmg`（arm64 + x86_64） |
| Windows | `windows-latest` | 无 | NSIS `.exe`（x64） |

步骤：checkout → pnpm 9 / Node 20 → Rust stable（`Swatinem/rust-cache`；macOS 额外 `rustup target add` 两架构）
→ `pnpm install --frozen-lockfile` → `pnpm pdfium`（按 runner 平台下载对应引擎）→ `pnpm tauri build <args>`
→ 上传 artifact。两端成功后 `release` 作业（`needs: build`，仅标签触发）下载产物并用
`softprops/action-gh-release` 建 **Release 草稿**。

### 从 Codeup 镜像到 GitHub

仓库主 remote 是阿里云效 Codeup，GitHub Actions 只在代码推到 **GitHub** 后才触发：

```bash
git remote add github git@github.com:<用户名>/pdf-rust.git
git push -u github master
```

### ⚠️ 已知坑：Actions 表达式字符串必须用单引号

`if:` / `${{ }}` 等**表达式**里的字符串字面量只能用**单引号**，用双引号会导致整个 workflow 校验失败
（表现：运行秒失败、0 个 job、run 名退回显示为文件路径）。

```yaml
# ✗ 错误：Invalid workflow file
if: startsWith(github.ref, "refs/tags/")
# ✓ 正确
if: startsWith(github.ref, 'refs/tags/')
```

注意区分：YAML 普通标量值（如 tag 过滤 `- "v*"`、`args: ""`）不是表达式，双引号合法。
本地 `yaml.safe_load` 能过 ≠ Actions 能过；排障时先看 run 页面的 **Annotations → Invalid workflow file**。

---

## 8. 正式发版流程

1. **改版本号**（三处保持一致）：`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`package.json`
2. 提交并推送到 GitHub：`git push github master`
3. **打标签触发构建**：

   ```bash
   git tag -a v0.2.0 -m "v0.2.0"
   git push github v0.2.0
   ```

4. 到 [Actions](https://github.com/lifeibest/pdf-rust/actions) 看构建（约 13 分钟，mac universal 是耗时大头）
5. 完成后到 [Releases](https://github.com/lifeibest/pdf-rust/releases) 检查草稿里的
   `.dmg` + `.exe`，确认无误点 **Publish release** 正式发布

> 草稿（draft）对未认证请求不可见，`GET /releases` 返回空是正常的，登录后即可看到。

### 重跑同一个标签

标签已存在时，需先删除再重建才能让 CI 重新触发（避免 `--force`）：

```bash
git tag -d v0.1.0                      # 删本地
git push github :refs/tags/v0.1.0      # 删远程
git tag -a v0.1.0 -m "..."             # 在最新提交上重建
git push github v0.1.0                 # 推送触发
```

### 清理测试痕迹

```bash
git push github :refs/tags/v0.1.0 && git tag -d v0.1.0   # 删标签
# Release 草稿在 Releases 页面手动删除
```

---

## 9. 测试与校验

```bash
pnpm typecheck      # tsc --noEmit，双语 i18n key 结构同步校验
pnpm test:rust      # cargo test（单测 + 集成测试，集成测试代码生成 fixture PDF）
pnpm lint:rust      # cargo clippy -- -D warnings
```

改动后建议至少跑 `pnpm typecheck`；涉及 Rust PDF 逻辑的改动跑 `pnpm test:rust`。

---

## 10. 常见问题速查

| 现象 | 原因 / 处理 |
|---|---|
| 应用能启动但所有功能报 `pdfiumNotFound` | 内嵌 dylib 未签名/签名无效被 AMFI 拒载；用 `pnpm sign:macos` 先签 dylib 再签 app |
| CI 运行秒失败、0 job、run 名是文件路径 | workflow 表达式用了双引号；改单引号（见 §7） |
| CI `pnpm install --frozen-lockfile` 失败 | `pnpm-lock.yaml` 与 `package.json` 不同步；本地 `pnpm install` 后提交 lockfile |
| 本地 `hdiutil create` 报「操作不被允许」 | 沙箱限制；DMG 打包需在沙箱外运行 |
| 改了源码但装 DMG 后没生效 | 只重打了 `.app` 没重打 DMG；改了源码要重跑完整 `pnpm tauri build` 刷新 DMG |
| Windows 首次运行弹 SmartScreen | 未做代码签名；点「仍要运行」，正式分发需 Windows 代码签名证书 |
| macOS 首次打开被 Gatekeeper 拦 | ad-hoc/未公证；「系统设置→隐私与安全性」放行，或接入 Developer ID 签名 + 公证 |
