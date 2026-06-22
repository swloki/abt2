//! 付款条件解析工具
//!
//! 从客户/供应商的 payment_terms 自由文本中提取付款天数。
//! 支持常见格式：Net 30, 月结30天, 30, 60 days 等。

/// 解析付款条件文本，返回付款天数。无法解析时默认 30 天。
pub fn parse_payment_terms_days(terms: Option<&str>) -> i64 {
    let text = match terms {
        Some(t) => t.trim(),
        None => return 30,
    };
    if text.is_empty() {
        return 30;
    }

    // 尝试提取数字
    // 模式1: "Net 30" / "net 60"
    if let Some(days) = try_parse_net_format(text) {
        return days;
    }

    // 模式2: "月结30天" / "月结 30 天" / "月结60"
    if let Some(days) = try_parse_chinese_format(text) {
        return days;
    }

    // 模式3: 纯数字 "30", "30天", "30 days", "60d"
    if let Some(days) = try_parse_numeric(text) {
        return days;
    }

    // 默认
    30
}

fn try_parse_net_format(text: &str) -> Option<i64> {
    let lower = text.to_lowercase();
    if lower.starts_with("net ") {
        let num_str = &text[4..].trim();
        return num_str.parse::<i64>().ok();
    }
    None
}

fn try_parse_chinese_format(text: &str) -> Option<i64> {
    if text.contains("月结") {
        // "月结30天" → 提取 "30"
        let after: String = text.chars().skip_while(|c| *c != '结').skip(1).collect();
        return extract_first_number(&after);
    }
    None
}

fn try_parse_numeric(text: &str) -> Option<i64> {
    extract_first_number(text)
}

fn extract_first_number(text: &str) -> Option<i64> {
    let digits: String = text.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<i64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        assert_eq!(parse_payment_terms_days(None), 30);
        assert_eq!(parse_payment_terms_days(Some("")), 30);
        assert_eq!(parse_payment_terms_days(Some("unknown")), 30);
    }

    #[test]
    fn test_net_format() {
        assert_eq!(parse_payment_terms_days(Some("Net 30")), 30);
        assert_eq!(parse_payment_terms_days(Some("net 60")), 60);
        assert_eq!(parse_payment_terms_days(Some("Net 90")), 90);
    }

    #[test]
    fn test_chinese_format() {
        assert_eq!(parse_payment_terms_days(Some("月结30天")), 30);
        assert_eq!(parse_payment_terms_days(Some("月结 60 天")), 60);
        assert_eq!(parse_payment_terms_days(Some("月结90")), 90);
    }

    #[test]
    fn test_numeric() {
        assert_eq!(parse_payment_terms_days(Some("30")), 30);
        assert_eq!(parse_payment_terms_days(Some("30天")), 30);
        assert_eq!(parse_payment_terms_days(Some("30 days")), 30);
        assert_eq!(parse_payment_terms_days(Some("60d")), 60);
    }
}
