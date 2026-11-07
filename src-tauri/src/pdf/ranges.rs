//! 页码范围解析。被拆分、PDF 转图片、旋转三个功能共用。
//!
//! 支持的表达式（1-based，逗号分隔，空白容错）：
//! - `all` / `*`：全部页
//! - `odd`：奇数页；`even`：偶数页
//! - `first` / `last`：首页 / 末页
//! - `first:N` / `last:N`：前 N 页 / 后 N 页（超出总页数时自动收敛）
//! - `1,3-5,8`：显式页码与区间；区间写反（`5-3`）时自动纠正为 `3-5`

use crate::error::AppError;

/// 把范围表达式解析为 1-based 页码列表，按输入顺序去重。
pub fn parse_ranges(spec: &str, total: usize) -> Result<Vec<usize>, AppError> {
    Ok(parse_ranges_to_groups(spec, total)?
        .into_iter()
        .flatten()
        .collect::<Vec<usize>>()
        .dedup_in_order())
}

/// 把范围表达式解析为分组结果：每个逗号分隔的 token 成为一个独立分组。
///
/// 例如 `1-3,4-6` 在 `total = 8` 时返回 `[[1,2,3],[4,5,6]]`，拆分功能据此产出两个文件。
pub fn parse_ranges_to_groups(spec: &str, total: usize) -> Result<Vec<Vec<usize>>, AppError> {
    if total == 0 {
        return Err(AppError::Range("文档中没有任何页面".to_string()));
    }

    let normalized = spec.trim();
    if normalized.is_empty() {
        return Err(AppError::Range("页码范围不能为空".to_string()));
    }

    let lower = normalized.to_ascii_lowercase();
    if let Some(group) = parse_keyword(&lower, total) {
        if group.is_empty() {
            return Err(AppError::Range(format!(
                "表达式 {spec} 在该文档中没有匹配到任何页面"
            )));
        }
        return Ok(vec![group]);
    }

    let mut groups = Vec::new();
    for token in normalized.split(',') {
        let token = token.trim();
        if token.is_empty() {
            return Err(AppError::Range(format!("表达式 {spec} 中存在空的页码项")));
        }

        let group = if let Some(sub) = token.to_ascii_lowercase().strip_prefix("first:") {
            take_from_start(parse_count(sub, token)?, total)
        } else if let Some(sub) = token.to_ascii_lowercase().strip_prefix("last:") {
            take_from_end(parse_count(sub, token)?, total)
        } else {
            parse_single_token(token, total)?
        };

        if group.is_empty() {
            return Err(AppError::Range(format!(
                "表达式 {spec} 中的 {token} 没有匹配到任何页面"
            )));
        }
        groups.push(group);
    }

    Ok(groups)
}

/// 按固定页数把 `1..=total` 切成若干闭区间分组。`size` 为 0 时视为 1，避免死循环。
pub fn chunk_pages(total: usize, size: usize) -> Vec<(usize, usize)> {
    let size = size.max(1);
    if total == 0 {
        return Vec::new();
    }

    (1..=total)
        .step_by(size)
        .map(|start| (start, (start + size - 1).min(total)))
        .collect()
}

/// 把 1-based 页码列表转换为 0-based 索引列表。
pub fn to_zero_based(pages: &[usize]) -> Vec<usize> {
    pages.iter().map(|p| p.saturating_sub(1)).collect()
}

/// 解析 `first:N` / `last:N` 中的 N。
fn parse_count(sub: &str, token: &str) -> Result<usize, AppError> {
    sub.trim()
        .parse::<usize>()
        .map_err(|_| AppError::Range(format!("无法解析 {token} 中的页数")))
        .and_then(|n| {
            if n == 0 {
                Err(AppError::Range(format!("{token} 中的页数必须大于 0")))
            } else {
                Ok(n)
            }
        })
}

/// 处理关键字形式：`all` / `odd` / `even` / `first` / `last`。非关键字返回 `None`。
fn parse_keyword(lower: &str, total: usize) -> Option<Vec<usize>> {
    match lower {
        "all" | "*" => Some((1..=total).collect()),
        "odd" => Some((1..=total).filter(|p| p % 2 == 1).collect()),
        "even" => Some((1..=total).filter(|p| p % 2 == 0).collect()),
        "first" => Some(vec![1]),
        "last" => Some(vec![total]),
        _ => {
            if let Some(sub) = lower.strip_prefix("first:") {
                sub.trim()
                    .parse::<usize>()
                    .ok()
                    .map(|n| take_from_start(n, total))
            } else if let Some(sub) = lower.strip_prefix("last:") {
                sub.trim()
                    .parse::<usize>()
                    .ok()
                    .map(|n| take_from_end(n, total))
            } else {
                None
            }
        }
    }
}

fn take_from_start(n: usize, total: usize) -> Vec<usize> {
    (1..=n.min(total)).collect()
}

fn take_from_end(n: usize, total: usize) -> Vec<usize> {
    let n = n.min(total);
    ((total - n + 1)..=total).collect()
}

/// 解析单个 token：`7` 或 `3-5`。
fn parse_single_token(token: &str, total: usize) -> Result<Vec<usize>, AppError> {
    if let Some((left, right)) = token.split_once('-') {
        let start = parse_page_no(left, token, total)?;
        let end = parse_page_no(right, token, total)?;
        // 用户写反区间时自动纠正，而不是直接报错
        let (lo, hi) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        Ok((lo..=hi).collect())
    } else {
        let page = parse_page_no(token, token, total)?;
        Ok(vec![page])
    }
}

fn parse_page_no(raw: &str, token: &str, total: usize) -> Result<usize, AppError> {
    let value = raw
        .trim()
        .parse::<usize>()
        .map_err(|_| AppError::Range(format!("无法解析页码 {token}")))?;

    if value == 0 {
        return Err(AppError::Range("页码从 1 开始，不能为 0".to_string()));
    }
    if value > total {
        return Err(AppError::Range(format!(
            "页码 {value} 超出范围，该文档共 {total} 页"
        )));
    }

    Ok(value)
}

trait DedupInOrder {
    fn dedup_in_order(self) -> Vec<usize>;
}

impl DedupInOrder for Vec<usize> {
    /// 保持首次出现顺序的去重。
    fn dedup_in_order(mut self) -> Vec<usize> {
        let mut seen = std::collections::HashSet::with_capacity(self.len());
        self.retain(|item| seen.insert(*item));
        self
    }
}

/// 把闭区间分组格式化为文件名后缀，如 `(1, 3)` -> `p1-3`，`(4, 4)` -> `p4`。
pub fn format_group_label(group: &(usize, usize)) -> String {
    if group.0 == group.1 {
        format!("p{}", group.0)
    } else {
        format!("p{}-{}", group.0, group.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(spec: &str, total: usize) -> Vec<usize> {
        parse_ranges(spec, total).expect("应当解析成功")
    }

    #[test]
    fn parses_explicit_pages_and_ranges() {
        assert_eq!(ok("1,3-5,8", 10), vec![1, 3, 4, 5, 8]);
        assert_eq!(ok(" 2 , 4 ", 10), vec![2, 4]);
        assert_eq!(ok("7", 10), vec![7]);
    }

    #[test]
    fn parses_keywords() {
        assert_eq!(ok("all", 5), vec![1, 2, 3, 4, 5]);
        assert_eq!(ok("*", 3), vec![1, 2, 3]);
        assert_eq!(ok("ODD", 6), vec![1, 3, 5]);
        assert_eq!(ok("even", 6), vec![2, 4, 6]);
        assert_eq!(ok("first", 6), vec![1]);
        assert_eq!(ok("last", 6), vec![6]);
    }

    #[test]
    fn parses_first_n_and_last_n() {
        assert_eq!(ok("first:3", 10), vec![1, 2, 3]);
        assert_eq!(ok("last:2", 10), vec![9, 10]);
        // 超出总页数时收敛而不是报错
        assert_eq!(ok("first:99", 4), vec![1, 2, 3, 4]);
        assert_eq!(ok("last:99", 4), vec![1, 2, 3, 4]);
    }

    #[test]
    fn dedups_while_preserving_input_order() {
        assert_eq!(ok("5,1-3,2,5", 10), vec![5, 1, 2, 3]);
    }

    #[test]
    fn corrects_reversed_range() {
        assert_eq!(ok("5-3", 10), vec![3, 4, 5]);
    }

    #[test]
    fn rejects_invalid_specs() {
        assert!(matches!(parse_ranges("", 10), Err(AppError::Range(_))));
        assert!(matches!(parse_ranges("0", 10), Err(AppError::Range(_))));
        assert!(matches!(parse_ranges("11", 10), Err(AppError::Range(_))));
        assert!(matches!(parse_ranges("1,,3", 10), Err(AppError::Range(_))));
        assert!(matches!(parse_ranges("abc", 10), Err(AppError::Range(_))));
        assert!(matches!(
            parse_ranges("first:0", 10),
            Err(AppError::Range(_))
        ));
        assert!(matches!(parse_ranges("all", 0), Err(AppError::Range(_))));
    }

    #[test]
    fn groups_each_comma_token_separately() {
        let groups = parse_ranges_to_groups("1-3,4-6", 8).expect("应当解析成功");
        assert_eq!(groups, vec![vec![1, 2, 3], vec![4, 5, 6]]);

        let single = parse_ranges_to_groups("all", 4).expect("应当解析成功");
        assert_eq!(single, vec![vec![1, 2, 3, 4]]);
    }

    #[test]
    fn chunks_pages_inclusively() {
        assert_eq!(chunk_pages(6, 2), vec![(1, 2), (3, 4), (5, 6)]);
        assert_eq!(chunk_pages(5, 2), vec![(1, 2), (3, 4), (5, 5)]);
        assert_eq!(chunk_pages(1, 5), vec![(1, 1)]);
        assert_eq!(chunk_pages(0, 2), Vec::<(usize, usize)>::new());
        // size 为 0 时退化为逐页，避免死循环
        assert_eq!(chunk_pages(3, 0), vec![(1, 1), (2, 2), (3, 3)]);
    }

    #[test]
    fn converts_to_zero_based_indices() {
        assert_eq!(to_zero_based(&[1, 3, 5]), vec![0, 2, 4]);
    }

    #[test]
    fn formats_group_labels() {
        assert_eq!(format_group_label(&(1, 3)), "p1-3");
        assert_eq!(format_group_label(&(4, 4)), "p4");
    }
}
