# 架构

PDF智转 是一个 Tauri 2 桌面应用：**所有 PDF 计算都在 Rust 侧完成**，前端只负责
界面与任务编排。唯一 PDF 引擎是 PDFium（`pdfium-render 0.9.5`，对应
`chromium/8125`），八大功能共用它，保证输出行为一致。

```
┌──────────────────────────── WebView (React 19) ────────────────────────────┐
│  features/registry.ts ──► 路由 / 侧边栏 / 首页卡片 / 全局拖拽匹配            │
│  useFeatureController ──► useSourceFiles + useOutputPicker + useTaskRunner  │
│  components/* ──────────► DropZone / FileList / OptionsForm / TaskProgress  │
│  store/* (zustand) ─────► settings / tasks / drop / app                     │
│  lib/tauri.ts ──────────► 后端契约的 TS 镜像（invoke 封装 + 类型）            │
└───────────────▲───────────────────────────────▲────────────────────────────┘
                │ invoke（命令）                 │ emit（事件）
┌───────────────┴───────────────────────────────┴────────────────────────────┐
│  commands/   tasks.rs（任务编排） pdf.rs / fs.rs / app.rs                    │
│  pdf/        merge split rotate remove_blank compress export_images      │
│              extract_images images_to_pdf invoice                        │
│              ranges（页范围表达式） thumbnails（缩略图缓存） engine（动态加载） │
│  state.rs    任务记录表 + 取消令牌（managed state）                          │
│  events.rs   task:progress / task:log / task:finished / task:failed          │
└───────────────────────────────▲────────────────────────────────────────────┘
                                │ pdfium-render（动态绑定）
                        libpdfium.dylib (chromium/8125)
```

## 后端

### 任务模型

- `run_task(request, out_dir, password, notify)` 是唯一入口：创建 `TaskRecord`、
  在 tokio 阻塞池里执行对应 `pdf::<op>`、把进度通过事件推给前端、结束时写回记录。
- 取消走 `CancelToken`（`commands/tasks.rs` 维护 `HashMap<taskId, token>`），
  各功能在页/文件循环里检查令牌，被取消时记录状态为 `cancelled`。
- 历史记录保存在内存（`AppState`，上限 50 条），不做持久化：任务中心是「本次会话」
  的视图，重启后为空是预期行为。
- 事件 payload 定义在 `src-tauri/src/events.rs`，前端镜像在
  `src/lib/tauri.ts` + `src/store/tasks.ts`。**改事件必须两边同步**。

| 事件 | payload | 用途 |
| --- | --- | --- |
| `task:progress` | `{taskId, feature, current, total, message}` | 进度条与状态文字 |
| `task:log` | `{taskId, level, message}` | 非致命告警（当前仅记录） |
| `task:finished` | `{taskId, feature, outputs, elapsedMs, summary}` | 产出列表、完成摘要（如压缩率）、通知 |
| `task:failed` | `{taskId, feature, error, elapsedMs}` | 错误展示（`error.kind` 映射 i18n） |

### 错误模型

`AppError` 用 serde 邻接标签序列化为 `{ kind, message }`，`kind` 是封闭枚举
（`pdfiumNotFound` / `passwordRequired` / `pdf` / `io` / `image` / `archive` /
`invalidParams` / `range` / `unsupported` / `cancelled` / `internal`）。
前端 `lib/errors.ts` 把 `kind` 映射成 i18n key，`message` 只作为补充细节展示 ——
因此新增错误种类时前端不改动也能给出可读文案（落到 `errors.unknown` 之外不会发生，
因为映射表是穷尽的）。

### 引擎加载（`pdf/engine.rs`）

`libpdfium` 不静态链接，运行时按以下顺序动态绑定，`init()` 幂等：

1. 环境变量 `PDFIUM_PATH`（指向文件或目录）
2. Tauri `resource_dir`（打包后 `*.app/Contents/Resources/libpdfium.dylib`，
   由 `bundle.resources` 从 `src-tauri/binaries/` 拷入）
3. 开发期 `src-tauri/binaries/libpdfium.dylib`
4. 可执行文件同级目录
5. 系统已安装的 libpdfium

失败时 `AppError::PdfiumNotFound(searched)` 带上全部搜索过的路径，关于页原样展示，
用户把 dylib 放进任一路径后点「重新检测」（`reload_pdfium`）即可，无需重启。
`binaries/PDFIUM_VERSION.json` 记录 tag / asset / sha256，随库一起分发并显示在关于页。

### 页范围表达式（`pdf/ranges.rs`）

语法：`all` / `*`、`odd` / `even`、`first` / `last`、`first:N` / `last:N`、
`1,3-5,8`（写反的区间自动纠正）。前端 `lib/pages.ts` 是同一语法的镜像实现，
只用于即时校验与「将影响 N 页」，**真正的展开以后端为准**。两侧各有单元测试。

### 缩略图（`pdf/thumbnails.rs`）

- 渲染结果以 PNG 落在 app cache 目录，缓存键为 `(source, max_px)`；
  前端用 `convertFileSrc` 生成 `asset://` URL 直接 `<img>` 读取，避免 base64 过 IPC。
- `assetProtocol.scope` 只允许 `$APPCACHE/**`，因此**用户原图不能直接预览**
  （图片转 PDF 页明确说明了这一点）。
- `pages` 传空数组表示渲染全部；前端默认只请求前 12 页并提示截断。
- `ThumbOut.page` 是 0-based 页索引，前端展示时 +1。

## 前端

### 功能注册表（`features/registry.ts`）

`FeatureDefinition` 是唯一的扩展点：路由、侧边栏分组、首页卡片、全局拖拽匹配、
选项表单、请求构造全部由它驱动。

- `accepts.ext` 决定拖拽匹配；`matchFeatures()` 要求候选功能的接受范围**覆盖全部**
  拖入类型，并按「接受范围越窄越靠前」排序，因此第一个就是最可能的意图，
  多于一个时弹 `FeatureChooser`。
- `buildRequest(files, values, context)` 把界面状态翻译成后端 `TaskRequest`；
  `kind` 直接等于 `feature.id`，中间没有映射表。
- `Page` 可选：缺省落到 `GenericFeaturePage`（零页面代码即可上线一个简单功能），
  需要额外信息呈现时（合并的顺序预览、旋转的实时预览）才写自定义页。

### 控制器（`hooks/useFeatureController.ts`）

把「来源列表 + 选项值 + 输出目录 + 任务运行」组合成一个对象，所有功能页共用：

- `useSourceFiles`：去重、按扩展名分流、`stat_paths` 补体积、逐个 `probe_document`
  探测（失败静默，不让整批添加失败）、加密文档的密码解锁。
- `useOutputPicker`：按设置里的输出策略（remember / ask / fixed）解析目录，
  `ask` 模式现场弹目录框，用户取消则**不**悄悄写默认目录。
- `useTaskRunner`：把后端事件绑定到「本次运行」的记录上；`taskId` 要等 `run_task`
  返回才知道，而进度事件在返回前就到达，因此运行期间先按「同功能 + 开始时间下界」
  匹配，拿到 id 后切换为精确匹配。
- `canRun` / `hint`：缺文件 → 密码未解锁 → 页范围非法 → 功能自定义校验，
  逐层给出禁用原因，显示在运行按钮旁。

### 全局拖拽（`store/drop.ts` + `hooks/useDropFiles.ts`）

Tauri 会拦截 HTML5 拖放（`dragDropEnabled`），因此统一订阅
`getCurrentWebview().onDragDropEvent()`。store 里只存**路径**（不存已构造的
`SourceFile`），体积与文档探测交给目标页的 `useSourceFiles.add()`，避免两套异步逻辑。
目标页可能已经打开（不会有 mount），所以 `usePendingDrop` 订阅 store 而不是取一次。

### 状态与持久化

- `store/settings.ts`：`@tauri-apps/plugin-store` 落盘 `settings.json`，300ms 防抖；
  磁盘值逐字段 `sanitize` 后与默认值合并，旧版本配置不会让应用崩。
  **`ready` 之前不写盘**，否则内存里的默认值会覆盖用户旧配置。
- `store/tasks.ts`：订阅四个事件维护任务表；`bootstrap()` 拉一次 `list_tasks`。
- 主题：Tailwind `darkMode: "class"`，`useTheme` 只在 `<html>` 上增删 `dark`，
  `system` 模式监听 `prefers-color-scheme`。
- 颜色令牌定义在 `styles/globals.css` 的 CSS 变量里，组件不写 `dark:` 前缀。

### i18n

`zh-CN.ts` 是形状来源（`TranslationShape`），`en-US.ts` 声明为该类型，
**缺 key 会在编译期报错**。所有界面字符串走 key，包括纯函数工具
（`lib/format.ts` 接收 `Translate` 参数）。`t()` 的复数兜底依赖 i18next 先查基础 key，
因此 `{{count}}` 插值无需 `_one/_other` 变体。

## 安全

- CSP 见 `tauri.conf.json`：`default-src 'self'`，图片额外允许 `asset:` / `blob:` /
  `data:`，连接允许 `ipc:` 与 `asset:`。
- `assetProtocol.scope` 仅 `$APPCACHE/**`：缩略图可读，用户文件不可读。
- 打开任意产出文件走后端命令 `open_path`（`plugin-opener` 的 JS `openPath` 需要
  `allow-open-path` + 路径 scope，而产出目录是用户任选的，无法提前枚举）。
- 能力集 `capabilities/default.json` 只授予 `core:default`、`dialog:default`、
  `opener:default`、`store:default`、`log:default`、`notification:default` 等最小集合。

## 测试

- Rust 单测：`ranges.rs` 全部分支、`images_to_pdf` 的 px→pt 与适配缩放、
  `split` 的分组边界、`rotate` 的角度归一化、`remove_blank` 的墨量占比、
  `compress` 的体积文案与 JPEG 编码、`fs` 的 `stat_paths`。
- Rust 集成测试（`src-tauri/tests/`）：用代码生成 6 页 fixture PDF（不提交二进制），
  覆盖 merge / split / rotate / rmblank / compress / export / extract / import / invoice 九条链路。
- 前端：`pnpm typecheck` 零错误；`TranslationShape` 保证双语文案结构同步。
