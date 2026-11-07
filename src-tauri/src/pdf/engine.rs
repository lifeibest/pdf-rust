//! PDFium 运行库的查找、绑定与单例管理。
//!
//! `pdfium-render` 在运行时动态加载 libpdfium，因此本模块负责按优先级定位库文件，
//! 并保证全进程只绑定一次（`pdfium-render` 内部使用全局 `OnceCell` 保存 bindings，
//! 重复调用 `Pdfium::bind_to_library` 会返回 `PdfiumLibraryBindingsAlreadyInitialized`）。

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use pdfium_render::prelude::Pdfium;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// 编译期绑定的 PDFium API 版本，用于在关于页提示库与绑定的对应关系。
/// 需与 `scripts/download-pdfium.mjs` 中的 `PDFIUM_TAG` 保持一致。
pub const EXPECTED_PDFIUM_TAG: &str = "chromium/8125";

static ENGINE: OnceLock<EngineState> = OnceLock::new();
static INIT_LOCK: Mutex<()> = Mutex::new(());

struct EngineState {
    pdfium: Pdfium,
    source: LibrarySource,
    /// 随库一同分发的 PDFIUM_VERSION.json 内容（若存在）
    manifest: Option<PdfiumManifest>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LibrarySource {
    /// 打包进 app 的 libpdfium（resource_dir）
    Bundled,
    /// 开发期使用的 src-tauri/binaries/libpdfium
    Workspace,
    /// 通过环境变量 PDFIUM_PATH 指定
    EnvOverride,
    /// 可执行文件同级目录
    ExecutableDir,
    /// 系统已安装的 libpdfium
    System,
}

impl LibrarySource {
    pub fn label(&self) -> &'static str {
        match self {
            LibrarySource::Bundled => "bundled",
            LibrarySource::Workspace => "workspace",
            LibrarySource::EnvOverride => "env",
            LibrarySource::ExecutableDir => "executable-dir",
            LibrarySource::System => "system",
        }
    }
}

/// `scripts/download-pdfium.mjs` 写入的版本清单。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfiumManifest {
    pub tag: Option<String>,
    pub asset: Option<String>,
    pub sha256: Option<String>,
    pub bytes: Option<u64>,
    pub downloaded_at: Option<String>,
}

/// 供关于页 / 诊断使用的引擎状态快照。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub ready: bool,
    pub source: Option<String>,
    pub library_path: Option<String>,
    pub expected_tag: &'static str,
    pub manifest: Option<PdfiumManifest>,
    pub error: Option<String>,
    pub searched: Vec<String>,
}

/// 当前平台下 PDFium 动态库的文件名。
pub fn library_file_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "pdfium.dll"
    } else if cfg!(target_os = "macos") {
        "libpdfium.dylib"
    } else {
        "libpdfium.so"
    }
}

/// 初始化引擎。`extra_dirs` 为额外的搜索目录（通常是 Tauri 的 resource_dir）。
///
/// 该函数是幂等的：重复调用会直接返回已初始化的实例。
pub fn init(extra_dirs: &[PathBuf]) -> Result<&'static Pdfium, AppError> {
    if let Some(state) = ENGINE.get() {
        return Ok(&state.pdfium);
    }

    // 串行化首次初始化，避免并发命令触发重复绑定
    let _guard = INIT_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    if let Some(state) = ENGINE.get() {
        return Ok(&state.pdfium);
    }

    let mut searched: Vec<String> = Vec::new();
    let mut candidates: Vec<(PathBuf, LibrarySource)> = Vec::new();

    // 1. 环境变量覆盖，便于本地调试不同版本的 libpdfium
    if let Ok(explicit) = std::env::var("PDFIUM_PATH") {
        let path = PathBuf::from(&explicit);
        let path = if path.is_dir() {
            path.join(library_file_name())
        } else {
            path
        };
        candidates.push((path, LibrarySource::EnvOverride));
    }

    // 2. 生产环境的 resource_dir（macOS: *.app/Contents/Resources）
    for dir in extra_dirs {
        candidates.push((dir.join(library_file_name()), LibrarySource::Bundled));
    }

    // 3. 开发期 src-tauri/binaries/
    candidates.push((
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(library_file_name()),
        LibrarySource::Workspace,
    ));

    // 4. 可执行文件同级目录
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
    {
        candidates.push((dir.join(library_file_name()), LibrarySource::ExecutableDir));
    }

    for (path, source) in &candidates {
        searched.push(path.display().to_string());
        if !path.exists() {
            continue;
        }

        match Pdfium::bind_to_library(path) {
            Ok(bindings) => {
                let pdfium = Pdfium::new(bindings);
                let manifest = read_manifest(path);
                let state = EngineState {
                    pdfium,
                    source: source.clone(),
                    manifest,
                };
                let installed = ENGINE.get_or_init(|| state);
                log::info!(
                    "PDFium 绑定成功: {} ({})",
                    path.display(),
                    installed.source.label()
                );
                return Ok(&installed.pdfium);
            }
            Err(err) => {
                log::warn!("无法加载 {}: {}", path.display(), err);
            }
        }
    }

    // 5. 兜底：系统已安装的 libpdfium
    searched.push(format!("<system {}>", library_file_name()));
    match Pdfium::bind_to_system_library() {
        Ok(bindings) => {
            let state = EngineState {
                pdfium: Pdfium::new(bindings),
                source: LibrarySource::System,
                manifest: None,
            };
            let installed = ENGINE.get_or_init(|| state);
            log::info!("PDFium 绑定成功: 系统库");
            return Ok(&installed.pdfium);
        }
        Err(err) => {
            log::error!("无法绑定系统 PDFium 库: {}", err);
        }
    }

    Err(AppError::PdfiumNotFound(searched))
}

/// 获取已初始化的引擎；未初始化时返回错误（正常流程下 `lib.rs::setup` 已完成初始化）。
pub fn get() -> Result<&'static Pdfium, AppError> {
    ENGINE
        .get()
        .map(|state| &state.pdfium)
        .ok_or_else(|| AppError::PdfiumNotFound(vec!["<engine not initialized>".to_string()]))
}

/// 引擎状态快照，供关于页与故障诊断使用。
pub fn status() -> EngineStatus {
    match ENGINE.get() {
        Some(state) => {
            let library_path = match &state.source {
                LibrarySource::System => None,
                _ => locate_library_path(),
            };

            EngineStatus {
                ready: true,
                source: Some(state.source.label().to_string()),
                library_path,
                expected_tag: EXPECTED_PDFIUM_TAG,
                manifest: state.manifest.clone(),
                error: None,
                searched: Vec::new(),
            }
        }
        None => EngineStatus {
            ready: false,
            source: None,
            library_path: None,
            expected_tag: EXPECTED_PDFIUM_TAG,
            manifest: None,
            error: Some("PDFium 尚未初始化".to_string()),
            searched: locate_library_path()
                .map(|path| vec![path])
                .unwrap_or_default(),
        },
    }
}

/// 重新按候选顺序查找一次库路径，仅用于展示，不做加载。
fn locate_library_path() -> Option<String> {
    let candidates = [
        std::env::var("PDFIUM_PATH").ok().map(PathBuf::from),
        Some(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("binaries")
                .join(library_file_name()),
        ),
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.join(library_file_name()))),
    ];

    candidates
        .into_iter()
        .flatten()
        .map(|path| {
            if path.is_dir() {
                path.join(library_file_name())
            } else {
                path
            }
        })
        .find(|path| path.exists())
        .map(|path| path.display().to_string())
}

/// 读取与库文件同目录的 PDFIUM_VERSION.json。
fn read_manifest(library_path: &Path) -> Option<PdfiumManifest> {
    let manifest_path = library_path.parent()?.join("PDFIUM_VERSION.json");
    let raw = std::fs::read_to_string(&manifest_path).ok()?;
    serde_json::from_str::<PdfiumManifest>(&raw).ok()
}

/// 编译期断言：`Pdfium` 必须可跨线程共享，否则 `AppState` 无法作为 Tauri managed state。
#[allow(dead_code)]
const fn _assert_pdfium_is_send_sync() {
    const fn check<T: Send + Sync + 'static>() {}
    check::<Pdfium>();
}
