//! 文件与目录选择、在访达中定位。
//!
//! 这些命令把 `tauri-plugin-dialog` / `tauri-plugin-opener` 包在 Rust 侧，
//! 前端只依赖统一的 `lib/tauri.ts`，不需要为每个插件单独申请能力。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::error::AppError;

/// 文件选择参数。`extensions` 为空时不过滤类型。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PickFilesArgs {
    pub title: Option<String>,
    /// 允许的扩展名（不含点），如 `["pdf", "png"]`
    pub extensions: Vec<String>,
    /// 过滤器显示名，如 `PDF 文档`
    pub filter_name: Option<String>,
    /// 是否允许多选，默认 true
    pub multiple: Option<bool>,
    /// 打开对话框时的初始目录
    pub directory: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickedFiles {
    pub paths: Vec<String>,
    /// 用户是否取消了选择
    pub cancelled: bool,
}

impl PickedFiles {
    fn from_paths(paths: Vec<PathBuf>) -> Self {
        Self {
            paths: paths
                .into_iter()
                .map(|path| path.display().to_string())
                .collect(),
            cancelled: false,
        }
    }

    fn cancelled() -> Self {
        Self {
            paths: Vec::new(),
            cancelled: true,
        }
    }
}

/// 选择一个或多个文件。
///
/// `blocking_pick_files` 会阻塞当前线程，因此放进 `spawn_blocking`，
/// 避免卡住 Tauri 的异步运行时。
#[tauri::command]
pub async fn pick_files(app: AppHandle, args: PickFilesArgs) -> Result<PickedFiles, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let multiple = args.multiple.unwrap_or(true);
        let mut builder = app.dialog().file();

        if let Some(title) = args.title.as_deref().filter(|t| !t.is_empty()) {
            builder = builder.set_title(title);
        }
        if !args.extensions.is_empty() {
            let name = args
                .filter_name
                .clone()
                .unwrap_or_else(|| "支持的格式".to_string());
            let extensions = args
                .extensions
                .iter()
                .map(|ext| ext.as_str())
                .collect::<Vec<_>>();
            builder = builder.add_filter(name, &extensions);
        }
        if let Some(dir) = args
            .directory
            .as_deref()
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .filter(|d| d.is_dir())
        {
            builder = builder.set_directory(dir);
        }

        let picked = if multiple {
            builder.blocking_pick_files().map(|files| {
                files
                    .into_iter()
                    .filter_map(|f| f.into_path().ok())
                    .collect()
            })
        } else {
            builder
                .blocking_pick_file()
                .and_then(|file| file.into_path().ok())
                .map(|path| vec![path])
        };

        Ok(match picked {
            Some(paths) if !paths.is_empty() => PickedFiles::from_paths(paths),
            _ => PickedFiles::cancelled(),
        })
    })
    .await
    .map_err(|_| AppError::Internal("文件选择器异常退出".to_string()))?
}

/// 选择一个目录（用作输出目录）。
#[tauri::command]
pub async fn pick_dir(app: AppHandle, title: Option<String>) -> Result<PickedFiles, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut builder = app.dialog().file();
        if let Some(title) = title.as_deref().filter(|t| !t.is_empty()) {
            builder = builder.set_title(title);
        }

        Ok(match builder.blocking_pick_folder() {
            Some(dir) => match dir.into_path() {
                Ok(path) => PickedFiles::from_paths(vec![path]),
                Err(_) => PickedFiles::cancelled(),
            },
            None => PickedFiles::cancelled(),
        })
    })
    .await
    .map_err(|_| AppError::Internal("目录选择器异常退出".to_string()))?
}

/// 「另存为」对话框，返回用户期望的完整保存路径。
#[tauri::command]
pub async fn pick_save(
    app: AppHandle,
    default_name: Option<String>,
    directory: Option<String>,
) -> Result<PickedFiles, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut builder = app.dialog().file();
        if let Some(name) = default_name.as_deref().filter(|n| !n.is_empty()) {
            builder = builder.set_file_name(name);
        }
        if let Some(dir) = directory
            .as_deref()
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .filter(|d| d.is_dir())
        {
            builder = builder.set_directory(dir);
        }

        Ok(match builder.blocking_save_file() {
            Some(target) => match target.into_path() {
                Ok(path) => PickedFiles::from_paths(vec![path]),
                Err(_) => PickedFiles::cancelled(),
            },
            None => PickedFiles::cancelled(),
        })
    })
    .await
    .map_err(|_| AppError::Internal("保存对话框异常退出".to_string()))?
}

/// 在系统文件管理器中定位路径。
///
/// 目录直接打开，文件则选中并高亮。
#[tauri::command]
pub fn reveal(app: AppHandle, path: String) -> Result<(), AppError> {
    let target = PathBuf::from(&path);
    if target.as_os_str().is_empty() {
        return Err(AppError::invalid("路径不能为空"));
    }

    // 输出文件被用户移走后，退回到其所在目录（若仍存在），避免报错
    let resolved = if target.exists() {
        target.clone()
    } else if let Some(parent) = target.parent().filter(|p| p.exists()) {
        parent.to_path_buf()
    } else {
        return Err(AppError::Io(format!("路径不存在：{path}")));
    };

    let opener = app.opener();
    let result = if resolved.is_dir() {
        opener.open_path(resolved.display().to_string(), None::<&str>)
    } else {
        opener.reveal_item_in_dir(resolved)
    };

    result.map_err(|err| AppError::Io(format!("无法打开文件管理器：{err}")))
}

/// 用系统默认应用打开文件（目录则在访达中打开）。
///
/// 走后端而不是 `plugin-opener` 的 JS `openPath`：后者需要 `opener:allow-open-path`
/// 并配置路径 scope，而产出目录是用户任选的，scope 无法提前枚举。
#[tauri::command]
pub fn open_path(app: AppHandle, path: String) -> Result<(), AppError> {
    let target = PathBuf::from(&path);
    if target.as_os_str().is_empty() {
        return Err(AppError::invalid("路径不能为空"));
    }
    if !target.exists() {
        return Err(AppError::Io(format!("路径不存在：{path}")));
    }

    app.opener()
        .open_path(target.display().to_string(), None::<&str>)
        .map_err(|err| AppError::Io(format!("无法打开文件：{err}")))
}

/// 判断路径是否为受支持的输入文件，供前端在拖拽时快速分类。
#[tauri::command]
pub fn classify_path(path: String) -> String {
    let target = Path::new(&path);
    if crate::pdf::imaging::is_pdf(target) {
        "pdf".to_string()
    } else if crate::pdf::imaging::is_image(target) {
        "image".to_string()
    } else {
        "unknown".to_string()
    }
}

/// 确保目录存在，返回其绝对路径字符串。前端「记住上次输出目录」时使用。
#[tauri::command]
pub fn ensure_dir(path: String) -> Result<String, AppError> {
    let target = PathBuf::from(&path);
    if target.as_os_str().is_empty() {
        return Err(AppError::invalid("路径不能为空"));
    }
    std::fs::create_dir_all(&target)?;
    Ok(target.display().to_string())
}

/// 单个路径的轻量元信息。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathStat {
    pub path: String,
    pub name: String,
    pub exists: bool,
    pub is_dir: bool,
    pub size_bytes: u64,
    /// `pdf` / `image` / `unknown`，与 [`classify_path`] 的取值一致
    pub kind: String,
}

/// 批量读取路径元信息，供前端文件列表显示体积与类型。
///
/// 不存在的条目不会报错，而是把 `exists` 置假：拖入后文件被移走是常见情形，
/// 界面应当能标出这一项而不是整批失败。
#[tauri::command]
pub fn stat_paths(paths: Vec<String>) -> Vec<PathStat> {
    paths.into_iter().map(stat_one).collect()
}

fn stat_one(path: String) -> PathStat {
    let target = Path::new(&path);
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());
    // 先把需要借用 `path` 的值算完，再把它移动进结果结构
    let kind = classify_path(path.clone());

    match std::fs::metadata(target) {
        Ok(meta) => PathStat {
            path,
            name,
            exists: true,
            is_dir: meta.is_dir(),
            size_bytes: meta.len(),
            kind,
        },
        Err(_) => PathStat {
            path,
            name,
            exists: false,
            is_dir: false,
            size_bytes: 0,
            kind,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_reports_size_and_kind_of_existing_files() {
        let dir = tempfile::tempdir().expect("临时目录");
        let file = dir.path().join("sample.pdf");
        std::fs::write(&file, b"%PDF-1.7\n").expect("写入应当成功");

        let stats = stat_paths(vec![file.display().to_string()]);
        assert_eq!(stats.len(), 1);
        assert!(stats[0].exists);
        assert!(!stats[0].is_dir);
        assert_eq!(stats[0].name, "sample.pdf");
        assert_eq!(stats[0].kind, "pdf");
        assert!(stats[0].size_bytes > 0);
    }

    #[test]
    fn stat_marks_missing_files_instead_of_failing() {
        let dir = tempfile::tempdir().expect("临时目录");
        let missing = dir.path().join("gone.png");

        let stats = stat_paths(vec![missing.display().to_string()]);
        assert_eq!(stats.len(), 1);
        assert!(!stats[0].exists);
        assert_eq!(stats[0].size_bytes, 0);
        assert_eq!(stats[0].name, "gone.png", "不存在时仍应从路径推导名称");
    }

    #[test]
    fn classify_recognizes_supported_inputs() {
        assert_eq!(classify_path("/tmp/a.PDF".to_string()), "pdf");
        assert_eq!(classify_path("/tmp/a.jpeg".to_string()), "image");
        assert_eq!(classify_path("/tmp/a.txt".to_string()), "unknown");
        assert_eq!(classify_path("/tmp/noext".to_string()), "unknown");
    }
}
