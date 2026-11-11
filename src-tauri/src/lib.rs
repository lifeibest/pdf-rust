//! 应用入口：插件注册、共享状态注入、PDFium 初始化与 command 路由。

mod commands;
pub mod error;
pub mod events;
pub mod license;
pub mod pdf;
pub mod state;
pub mod usage;

use tauri::Manager;
use tauri_plugin_log::{Target, TargetKind};

/// 日志同时输出到 stdout（`pnpm tauri dev` 时可见）、app log 目录与前端控制台。
fn log_targets() -> Vec<Target> {
    vec![
        Target::new(TargetKind::Stdout),
        Target::new(TargetKind::LogDir { file_name: None }),
        Target::new(TargetKind::Webview),
    ]
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // single-instance 必须最先注册，否则第二个实例会先跑完其它插件初始化
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_log::Builder::default()
                .targets(log_targets())
                .level(log::LevelFilter::Info)
                .build(),
        )
        .manage(state::AppState::default())
        .setup(|app| {
            let handle = app.handle().clone();
            // resource_dir 在生产环境指向 *.app/Contents/Resources，即 libpdfium 的落点；
            // 开发期取不到时退回空列表，engine 会继续尝试 binaries/ 与可执行文件同级目录。
            let extra_dirs = handle
                .path()
                .resource_dir()
                .map(|dir| vec![dir])
                .unwrap_or_default();

            // 初始化失败不退出进程：前端「关于 / 首页」会读取 pdfium_status 并给出
            // 可操作的安装指引，而不是让用户面对一个白屏窗口。
            match pdf::engine::init(&extra_dirs) {
                Ok(_) => log::info!("PDFium 引擎就绪"),
                Err(err) => log::error!("PDFium 引擎初始化失败: {err}"),
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 任务
            commands::tasks::run_task,
            commands::tasks::cancel_task,
            commands::tasks::list_tasks,
            commands::tasks::clear_tasks,
            commands::tasks::clear_temp,
            // 微信公众号文章导出
            commands::wechat::export_wechat_pdf,
            commands::wechat::cancel_wechat_export,
            commands::wechat::confirm_wechat_export,
            // 文档与缩略图
            commands::pdf::probe_document,
            commands::pdf::render_thumbnails,
            commands::pdf::clear_thumb_cache,
            // 文件系统
            commands::fs::pick_files,
            commands::fs::pick_dir,
            commands::fs::pick_save,
            commands::fs::reveal,
            commands::fs::open_path,
            commands::fs::classify_path,
            commands::fs::ensure_dir,
            commands::fs::stat_paths,
            // 应用信息
            commands::app::app_info,
            commands::app::pdfium_status,
            commands::app::reload_pdfium,
            // 授权与配额
            commands::license::license_status,
            commands::license::activate_license,
            commands::license::deactivate_license,
            commands::license::quota_status,
            commands::license::device_fingerprint,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
