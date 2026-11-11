//! 每日任务配额计数与门控。
//!
//! 未激活用户每日免费 [`crate::license::FREE_DAILY_LIMIT`] 次；激活后按 license 的
//! `quota` 字段执行（`unlimited` 不限 / 正整数为每日上限）。计数以「本地日期 + 次数」
//! 落在配置目录 `usage.json`，跨日自动归零，全程离线。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::license::{LicenseStatus, FREE_DAILY_LIMIT};

/// 配置目录内的计数文件。
fn usage_file(config_dir: &Path) -> PathBuf {
    config_dir.join("usage.json")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsageRecord {
    /// 计数所属日期 `YYYY-MM-DD`（本地）
    date: String,
    count: u32,
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn read_record(config_dir: &Path) -> UsageRecord {
    let today = today();
    std::fs::read_to_string(usage_file(config_dir))
        .ok()
        .and_then(|raw| serde_json::from_str::<UsageRecord>(&raw).ok())
        .filter(|record| record.date == today)
        .unwrap_or(UsageRecord { date: today, count: 0 })
}

fn write_record(config_dir: &Path, record: &UsageRecord) -> Result<(), AppError> {
    std::fs::create_dir_all(config_dir)?;
    let json = serde_json::to_string_pretty(record)?;
    std::fs::write(usage_file(config_dir), json)?;
    Ok(())
}

/// 供前端展示的配额快照。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaStatus {
    /// 是否处于有效激活状态
    pub licensed: bool,
    /// 额度是否无限
    pub unlimited: bool,
    /// 每日上限；无限时为 null
    pub limit: Option<u32>,
    /// 今日已用次数
    pub used: u32,
    /// 今日剩余次数；无限时为 null
    pub remaining: Option<u32>,
    /// 当前是否还允许再跑一次任务
    pub allowed: bool,
    pub date: String,
}

/// 依据授权状态推导今日上限：激活且 unlimited → None；激活且数字 → 该值；否则免费上限。
fn daily_limit(license: &LicenseStatus) -> Option<u32> {
    if license.active {
        if license.quota_unlimited {
            return None;
        }
        return license.quota_per_day.or(Some(FREE_DAILY_LIMIT));
    }
    Some(FREE_DAILY_LIMIT)
}

/// 只读快照，不消耗次数。
pub fn quota_status(config_dir: &Path, license: &LicenseStatus) -> QuotaStatus {
    let record = read_record(config_dir);
    let limit = daily_limit(license);
    build(record.count, limit, license.active)
}

fn build(used: u32, limit: Option<u32>, licensed: bool) -> QuotaStatus {
    QuotaStatus {
        licensed,
        unlimited: limit.is_none(),
        limit,
        used,
        remaining: limit.map(|limit| limit.saturating_sub(used)),
        allowed: limit.is_none_or(|limit| used < limit),
        date: today(),
    }
}

/// 任务入口门控：允许则记一次并返回快照，超额则返回 [`AppError::QuotaExceeded`]。
pub fn consume(config_dir: &Path, license: &LicenseStatus) -> Result<QuotaStatus, AppError> {
    let mut record = read_record(config_dir);
    let limit = daily_limit(license);

    if let Some(limit) = limit
        && record.count >= limit
    {
        return Err(AppError::QuotaExceeded);
    }

    record.count += 1;
    write_record(config_dir, &record)?;
    Ok(build(record.count, limit, license.active))
}
