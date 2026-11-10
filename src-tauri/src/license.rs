//! 激活码(license)离线验签与本地存储。
//!
//! 激活码格式与签发工具 `pdf-api.pdfai.cn/tools/license/issue_license.py`（已迁至后端项目、私钥 gitignore）一致：
//! `base64url(payload_json) + "." + base64url(ed25519_sig)`。
//!
//! 客户端只内嵌 Ed25519 **公钥**做离线验签，私钥绝不出现在安装包内；
//! 因此校验全程不联网，符合「隐私优先、本地运行」的定位。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// 内嵌公钥（base64url, 32 字节 raw）。由 `pdf-api.pdfai.cn/tools/license/issue_license.py pubkey` 产出。
/// 轮换密钥时需同步更新此处与签发工具的 KEY_ID。
const LICENSE_PUBLIC_KEY: &str = "JpzTDwSgUuBCEo6ZWQ3OvnGhtM1oyzgl36lbLdTFt3k";

/// 与签发工具一致的密钥 id；token 的 `kid` 必须等于它。
const LICENSE_KEY_ID: &str = "v1";

/// 未激活时的每日免费任务次数。
pub const FREE_DAILY_LIMIT: u32 = 10;

/// 每日额度：`unlimited` 字符串或正整数。
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum QuotaLimit {
    Unlimited(String),
    PerDay(u32),
}

impl QuotaLimit {
    pub fn is_unlimited(&self) -> bool {
        matches!(self, QuotaLimit::Unlimited(tag) if tag == "unlimited")
    }

    pub fn per_day(&self) -> Option<u32> {
        match self {
            QuotaLimit::PerDay(n) => Some(*n),
            QuotaLimit::Unlimited(_) => None,
        }
    }
}

/// 激活码 payload，字段与签发工具的 JSON 一一对应。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LicensePayload {
    pub kid: String,
    pub id: String,
    pub plan: String,
    pub quota: QuotaLimit,
    pub machine: Option<String>,
    pub act: Option<u32>,
    pub iat: i64,
    pub exp: Option<i64>,
    #[serde(default)]
    pub nonce: Option<String>,
}

/// 供前端展示的授权状态快照。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseStatus {
    pub active: bool,
    pub plan: Option<String>,
    pub quota_unlimited: bool,
    pub quota_per_day: Option<u32>,
    pub exp: Option<i64>,
    pub license_id: Option<String>,
    /// token 绑定的机器码（未绑定为 null）
    pub machine: Option<String>,
    /// 本机机器码，供用户向签发方申请绑定码时提供
    pub machine_code: String,
    /// 未激活/校验失败的原因
    pub reason: Option<String>,
}

fn inactive(reason: Option<String>) -> LicenseStatus {
    LicenseStatus {
        active: false,
        plan: None,
        quota_unlimited: false,
        quota_per_day: None,
        exp: None,
        license_id: None,
        machine: None,
        machine_code: machine_code(),
        reason,
    }
}

fn from_payload(payload: &LicensePayload) -> LicenseStatus {
    LicenseStatus {
        active: true,
        plan: Some(payload.plan.clone()),
        quota_unlimited: payload.quota.is_unlimited(),
        quota_per_day: payload.quota.per_day(),
        exp: payload.exp,
        license_id: Some(payload.id.clone()),
        machine: payload.machine.clone(),
        machine_code: machine_code(),
        reason: None,
    }
}

/* ------------------------------------------------------------------ 验签 */

/// 解析并校验激活码：签名 → kid → 有效期 → 机器码绑定。
pub fn parse(code: &str) -> Result<LicensePayload, AppError> {
    let code = code.trim();
    let (payload_b64, sig_b64) = code
        .split_once('.')
        .ok_or_else(|| AppError::LicenseInvalid("格式不正确".to_string()))?;

    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|_| AppError::LicenseInvalid("内容无法解码".to_string()))?;
    let sig_bytes = URL_SAFE_NO_PAD
        .decode(sig_b64)
        .map_err(|_| AppError::LicenseInvalid("签名无法解码".to_string()))?;
    if sig_bytes.len() != Signature::BYTE_SIZE {
        return Err(AppError::LicenseInvalid("签名长度不正确".to_string()));
    }
    let mut sig_arr = [0u8; Signature::BYTE_SIZE];
    sig_arr.copy_from_slice(&sig_bytes);
    let signature = Signature::from_bytes(&sig_arr);

    let pub_bytes = URL_SAFE_NO_PAD
        .decode(LICENSE_PUBLIC_KEY)
        .map_err(|_| AppError::Internal("内嵌公钥配置错误".to_string()))?;
    let mut pub_arr = [0u8; 32];
    if pub_bytes.len() != 32 {
        return Err(AppError::Internal("内嵌公钥长度错误".to_string()));
    }
    pub_arr.copy_from_slice(&pub_bytes);
    let verifying_key = VerifyingKey::from_bytes(&pub_arr)
        .map_err(|_| AppError::Internal("内嵌公钥无效".to_string()))?;

    verifying_key
        .verify_strict(&payload_bytes, &signature)
        .map_err(|_| AppError::LicenseInvalid("签名校验失败".to_string()))?;

    let payload: LicensePayload = serde_json::from_slice(&payload_bytes)
        .map_err(|_| AppError::LicenseInvalid("内容结构不正确".to_string()))?;

    if payload.kid != LICENSE_KEY_ID {
        return Err(AppError::LicenseInvalid("密钥版本不受支持".to_string()));
    }
    if let Some(exp) = payload.exp {
        let now = chrono::Utc::now().timestamp();
        if now > exp {
            return Err(AppError::LicenseInvalid("激活码已过期".to_string()));
        }
    }
    if let Some(bound) = &payload.machine {
        if !bound.eq_ignore_ascii_case(&machine_code()) {
            return Err(AppError::LicenseInvalid("激活码与本机不匹配".to_string()));
        }
    }

    Ok(payload)
}

/* ------------------------------------------------------------------ 存储 */

fn license_file(config_dir: &Path) -> PathBuf {
    config_dir.join("license.json")
}

/// 读取本地保存的激活码原文。
pub fn load_code(config_dir: &Path) -> Option<String> {
    #[derive(Deserialize)]
    struct Stored {
        code: String,
    }
    let raw = std::fs::read_to_string(license_file(config_dir)).ok()?;
    serde_json::from_str::<Stored>(&raw).ok().map(|s| s.code)
}

/// 保存激活码原文（验签通过后调用）。
pub fn save_code(config_dir: &Path, code: &str) -> Result<(), AppError> {
    std::fs::create_dir_all(config_dir)?;
    #[derive(Serialize)]
    struct Stored<'a> {
        code: &'a str,
    }
    let json = serde_json::to_string_pretty(&Stored { code })?;
    std::fs::write(license_file(config_dir), json)?;
    Ok(())
}

/// 移除本地激活码。
pub fn clear(config_dir: &Path) {
    let _ = std::fs::remove_file(license_file(config_dir));
}

/// 综合状态：有存储码且验签通过则 active，否则给出原因。
pub fn status(config_dir: &Path) -> LicenseStatus {
    let Some(code) = load_code(config_dir) else {
        return inactive(Some("未激活".to_string()));
    };
    match parse(&code) {
        Ok(payload) => from_payload(&payload),
        Err(err) => inactive(Some(err.to_string())),
    }
}

/* ---------------------------------------------------------------- 机器码 */

static MACHINE_CODE: OnceLock<String> = OnceLock::new();

/// 本机机器码：优先取平台稳定 UUID，取不到时退化为 os-arch 标识。
///
/// 结果缓存于进程内；用于展示与「绑定机器」校验，不参与联网。
pub fn machine_code() -> String {
    MACHINE_CODE
        .get_or_init(|| detect_machine_code().unwrap_or_else(fallback_machine_code))
        .clone()
}

/// 机器码是否为真实硬件标识。退化为 `OS-ARCH-LOCAL` 时同平台会撞车，
/// 上报侧据此标记 `deviceIdReliable=false`，后端不对这类记录计槽位。
pub fn machine_code_is_real() -> bool {
    machine_code() != fallback_machine_code()
}

fn fallback_machine_code() -> String {
    format!("{}-{}-LOCAL", std::env::consts::OS, std::env::consts::ARCH).to_uppercase()
}

fn detect_machine_code() -> Option<String> {
    let raw = if cfg!(target_os = "macos") {
        let out = Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        text.lines()
            .find(|line| line.contains("IOPlatformUUID"))
            .and_then(|line| line.split('"').nth(3).map(str::to_string))
    } else if cfg!(target_os = "windows") {
        let out = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-CimInstance -ClassName Win32_ComputerSystemProduct).UUID",
            ])
            .output()
            .ok()?;
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        std::fs::read_to_string("/etc/machine-id")
            .ok()
            .map(|s| s.trim().to_string())
    };

    raw.map(|value| value.trim().replace('-', "").to_uppercase())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 由 `pdf-api.pdfai.cn/tools/license/issue_license.py issue --plan paid --quota unlimited --days 36500 --id selftest01`
    /// 签发的样例码（exp 远在将来、未绑机器），用于验证 Rust 验签与 Python 签发 interoperable。
    const SAMPLE_CODE: &str = "eyJhY3QiOm51bGwsImV4cCI6NDk0NzkxNTcwMiwiaWF0IjoxNzk0MzE1NzAyLCJpZCI6InNlbGZ0ZXN0MDEiLCJraWQiOiJ2MSIsIm1hY2hpbmUiOm51bGwsIm5vbmNlIjoiYTJhZWQxYzEiLCJwbGFuIjoicGFpZCIsInF1b3RhIjoidW5saW1pdGVkIn0.bqwA-BLmQgsgBgyc0dafBcfZwaFnUChnSIk5Alvq99tN7S6NYRSCRyHFv5PK74cTnV_TqOjEtAhv2hsb5gr5DA";

    #[test]
    fn verifies_code_issued_by_python_tool() {
        let payload = parse(SAMPLE_CODE).expect("样例码应通过验签");
        assert_eq!(payload.kid, LICENSE_KEY_ID);
        assert_eq!(payload.plan, "paid");
        assert!(payload.quota.is_unlimited());
        assert_eq!(payload.quota.per_day(), None);
    }

    #[test]
    fn rejects_tampered_code() {
        let mut tampered = SAMPLE_CODE.to_string();
        tampered.push('A');
        assert!(parse(&tampered).is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("not-a-license").is_err());
        assert!(parse("aaaa.bbbb").is_err());
    }
}
