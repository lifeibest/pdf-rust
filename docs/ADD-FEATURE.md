# 新增一个功能

以新增「PDF 加水印」（`watermark`）为例，完整走一遍需要改动的地方。
目标：**简单功能只写一个 `meta.ts` + 一个 Rust 模块 + 一行注册**。

## 1. 后端：实现处理逻辑

新建 `src-tauri/src/pdf/watermark.rs`：

```rust
use std::path::Path;

use crate::error::AppError;
use crate::pdf::types::{OutputFile, ProgressSink, TaskCtx};
use crate::pdf::{engine, ranges};

pub struct WatermarkOptions {
    pub source: PathBuf,
    pub password: Option<String>,
    pub text: String,
    pub scope: PageScope,
    pub output_name: Option<String>,
}

pub fn run(
    ctx: &TaskCtx,
    out_dir: &Path,
    opts: &WatermarkOptions,
    progress: ProgressSink<'_>,
) -> Result<Vec<OutputFile>, AppError> {
    let pdfium = engine::get()?;                    // 未初始化时返回 pdfiumNotFound
    // 1. 打开文档（加密用 `ctx.password` 解密）
    // 2. `ranges::expand` 展开作用页
    // 3. 页循环里检查 `ctx.cancel.is_cancelled()` 并调用 `progress(current, total, msg)`
    // 4. `crate::pdf::resolve_output_path(out_dir, opts.output_name.as_deref(), &fallback)`
    //    决定输出路径（不覆盖已存在文件），保存后返回 OutputFile 列表
    let _ = (pdfium, ctx, opts, progress);
    todo!()
}
```

约定（照抄现有模块即可）：

- 进度单位是「页」或「文件」，`total` 未知时传 0，前端会显示不定进度条。
- 输出命名统一走 `crate::pdf::resolve_output_path`，避免覆盖用户文件。
- 所有失败都转成 `AppError` 的某个 kind；**不要**自己拼用户可见文案。

在 `src-tauri/src/pdf/mod.rs` 里 `pub mod watermark;`。

## 2. 后端：接线命令

`src-tauri/src/commands/tasks.rs`：

1. 在 `TaskRequest` 枚举里加变体：`#[serde(rename = "watermark")] Watermark(WatermarkOptions)`。
   枚举是 `#[serde(tag = "kind", content = "options")]`，rename 必须等于功能 id。
2. 在 `impl TaskRequest` 的 `feature_id()` 与中文标签两个 match 里各补一个分支。
3. 在分发处加一个分支，调用 `pdf::watermark::run`，进度回调里发
   `events::TASK_PROGRESS`。

如果新功能需要新的前端可见类型（选项结构），同步到 `src/lib/tauri.ts`。

## 3. 前端：写 `meta.ts`

新建 `src/features/watermark/meta.ts`：

```ts
import { Stamp } from "lucide-react";
import { PDF_EXTENSIONS } from "@/features/accepts";
import { fields, scopeField } from "@/features/options";
import type { FeatureDefinition } from "@/features/types";
import { asString, asScope } from "@/features/types";
import { toPageScope } from "@/lib/pages";

const watermark: FeatureDefinition = {
  id: "watermark",                 // 必须与后端 TaskRequest 的 rename 一致
  route: "/watermark",
  titleKey: "features.watermark.title",
  descKey: "features.watermark.desc",
  icon: Stamp,
  accent: "from-teal-500 to-emerald-500",
  category: "organize",
  order: 40,
  accepts: { ext: [...PDF_EXTENSIONS], multiple: false },
  options: fields(
    { type: "text", key: "text", labelKey: "options.watermarkText", default: "" },
    scopeField(),
  ),
  needsOutputDir: true,
  sortEnabled: false,
  thumbEnabled: true,              // 通用页会自动加载页面缩略图
  mixedSources: false,
  buildRequest: (files, values, context) => ({
    kind: "watermark",
    options: {
      source: files[0]?.path ?? "",
      password: files[0]?.password ?? null,
      text: asString(values, "text"),
      scope: toPageScope(asScope(values, "scope")),
      outputName: context.outputName,
    },
  }),
  validate: (files) => (files.length === 0 ? "feature.needOneFile" : null),
};

export default watermark;
```

## 4. 前端：注册 + 文案

1. `src/features/registry.ts` 的 `FEATURES` 数组加一行 `watermark`。
   路由、侧边栏、首页卡片、拖拽匹配**全部自动生成**，不需要改任何布局代码。
2. `src/features/types.ts` 的 `FeatureId` 联合类型加 `"watermark"`。
3. 两个语言包加 key：`src/i18n/locales/zh-CN.ts` 与 `en-US.ts`
   （`features.watermark.*`、`options.watermarkText`）。
   `en-US.ts` 声明为 `TranslationShape`，**漏了会编译报错**，这是故意的。

到此为止，功能已经可用：走 `GenericFeaturePage`，自动获得来源选择、
密码解锁、缩略图预览、选项表单、输出目录、进度 / 取消 / 结果列表。

## 5. 可选：自定义页面

只有当通用页表达不了时才写 `src/features/watermark/index.tsx`：

```ts
Page: lazy(() => import("./index")),
```

页面用 `useFeatureController(feature)` 拿全套状态，再自由排版：

- 需要额外视觉信息（如合并的顺序预览条）→ 参考 `features/merge/index.tsx`；
- 需要所见即所得预览（如旋转的实时角度）→ 参考 `features/rotate/index.tsx`
  （`PageThumbGrid` 的 `previewRotation`）；
- 有「原地修改」这类会隐藏输出目录的模式 → `controller.run({ needsDir: false })`。

## 6. 测试与验收

- Rust：`src-tauri/tests/watermark_test.rs`，用 `tests/common/mod.rs` 生成 fixture，
  断言页数 / 页面内容 / 输出文件名。
- 页范围逻辑不用重写：复用 `pdf::ranges`，它已有完整单测。
- 前端：`pnpm typecheck`；手动跑一遍 README 的验收清单对应条目。
- 文案：切到英文确认没有漏翻（硬编码中文在 review 时会被打回）。

## 检查清单

- [ ] `src-tauri/src/pdf/watermark.rs` + `mod.rs` 导出
- [ ] `commands/tasks.rs`：`TaskRequest` 变体 + 分发分支
- [ ] `src/lib/tauri.ts`：新增选项类型（如有）
- [ ] `src/features/watermark/meta.ts`
- [ ] `src/features/registry.ts`：`FEATURES` 加一行
- [ ] `src/features/types.ts`：`FeatureId` 加成员
- [ ] `zh-CN.ts` / `en-US.ts`：全部新 key
- [ ] （可选）`src/features/watermark/index.tsx` 自定义页
- [ ] `src-tauri/tests/watermark_test.rs`
