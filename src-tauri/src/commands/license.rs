//! 授权与配额命令：激活码校验/存取、配额快照。

use std::path::PathBuf;

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::license::{self, LicenseStatus};
use crate::usage::{self, QuotaStatus};

/// 配置目录取不到时退回空路径：此时读不到已存激活码，等价于「未激活」，不会 panic。
fn config_dir_or_empty(app: &AppHandle) -> PathBuf {
    app.path().app_config_dir().unwrap_or_else(|_| PathBuf::new())
}

/// 当前授权状态（含本机机器码，供申请绑定码时提供）。
#[tauri::command]
pub fn license_status(app: AppHandle) -> LicenseStatus {
    let dir = config_dir_or_empty(&app);
    license::status(&dir)
}

/// 校验并保存激活码；成功后返回最新授权状态。
#[tauri::command]
pub fn activate_license(app: AppHandle, code: String) -> Result<LicenseStatus, AppError> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|err| AppError::Internal(err.to_string()))?;
    // 先验签再落盘，避免把无效码写进配置
    license::parse(&code)?;
    license::save_code(&dir, &code)?;
    Ok(license::status(&dir))
}

/// 移除本地激活码（恢复为免费额度）。
#[tauri::command]
pub fn deactivate_license(app: AppHandle) -> LicenseStatus {
    let dir = config_dir_or_empty(&app);
    license::clear(&dir);
    license::status(&dir)
}

/// 今日配额快照（只读，不消耗次数）。
#[tauri::command]
pub fn quota_status(app: AppHandle) -> QuotaStatus {
    let dir = config_dir_or_empty(&app);
    let license = license::status(&dir);
    usage::quota_status(&dir, &license)
}

/// 供 run_task 门控复用：消耗一次配额，超额返回 QuotaExceeded。
pub fn gate(app: &AppHandle) -> Result<(), AppError> {
    let dir = config_dir_or_empty(app);
    let license = license::status(&dir);
    usage::consume(&dir, &license)?;
    Ok(())
}

/* ---------------------------------------------------------------- 设备指纹 */

/// 上报哈希的固定盐：仅让 deviceId 不可反推机器码原文，不参与验签。
/// 必须与后端 `vip_service.REPORT_HASH_SALT` 一致，才能与 VIP 账本的设备去重口径对齐。
const REPORT_HASH_SALT: &str = "pdfai-vip-report-v1";

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

/// 兑换激活码时随请求上报的设备指纹：只含机器码哈希与设备元信息，不含机器码/激活码原文。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceFingerprint {
    pub device_id: String,
    /// 机器码是否为真实硬件标识；false（退化为 OS-ARCH-LOCAL）时后端不计入授权台数。
    pub device_id_reliable: bool,
    pub os: &'static str,
    pub arch: &'static str,
    pub app_version: &'static str,
}

/// 生成本机设备指纹。纯本地计算、不依赖已保存的激活码；供激活时联网兑换随请求带上，
/// 后端据此登记设备数并判定是否超过授权台数上限。
#[tauri::command]
pub fn device_fingerprint() -> DeviceFingerprint {
    let machine = license::machine_code();
    DeviceFingerprint {
        device_id: sha256_hex(&format!("{REPORT_HASH_SALT}:{machine}")),
        device_id_reliable: license::machine_code_is_real(),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        app_version: env!("CARGO_PKG_VERSION"),
    }
}
