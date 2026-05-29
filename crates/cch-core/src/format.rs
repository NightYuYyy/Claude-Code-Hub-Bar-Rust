//! Number, money, latency, duration and version formatting helpers ported
//! verbatim from the free functions in `MonitorState.swift`. Pure functions —
//! heavily unit-tested below.

use chrono::{DateTime, NaiveDateTime, Utc};
use std::cmp::Ordering;

/// Money split into a major part (always present) and an optional superscript
/// minor part, matching `moneyDisplayParts`.
pub fn money_display_parts(value: f64) -> (String, Option<String>) {
    if !value.is_finite() {
        return ("$0.00".to_string(), None);
    }
    let abs_value = value.abs();
    if abs_value >= 1000.0 {
        return (format!("${:.1}k", value / 1000.0), None);
    }
    if abs_value >= 100.0 {
        return (format!("${:.0}", value), None);
    }

    let trimmed = trim_money_suffix(&format!("${:.6}", value));
    let parts: Vec<&str> = trimmed.splitn(2, '.').collect();
    if parts.len() != 2 {
        return (trimmed, None);
    }

    let fraction = parts[1];
    if fraction.chars().count() <= 3 {
        return (trimmed, None);
    }

    let major = format!("{}.{}", parts[0], &fraction[..3]);
    let minor = &fraction[3..];
    let minor = if minor.is_empty() {
        None
    } else {
        Some(minor.to_string())
    };
    (major, minor)
}

/// Concatenated money string (major + minor), matching `formatMoney`.
pub fn format_money(value: f64) -> String {
    let (major, minor) = money_display_parts(value);
    match minor {
        Some(m) => major + &m,
        None => major,
    }
}

fn trim_money_suffix(value: &str) -> String {
    let mut text = value.to_string();
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    if text == "$0" {
        "$0.00".to_string()
    } else {
        text
    }
}

/// Compact integer (e.g. `1.2k`, `3.4M`), matching `compactNumber`.
pub fn compact_number(value: i64) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}k", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

pub fn format_percent(value: f64) -> String {
    format!("{:.1}%", value * 100.0)
}

pub fn format_latency(value: i64) -> String {
    if value <= 0 {
        "-".to_string()
    } else {
        format!("{:.1}s", value as f64 / 1000.0)
    }
}

pub fn format_milliseconds_as_seconds(value: Option<i64>) -> String {
    match value {
        Some(v) => format!("{:.2}s", v as f64 / 1000.0),
        None => "-".to_string(),
    }
}

pub fn format_probe_latency(value: Option<f64>) -> String {
    match value {
        None => "正常".to_string(),
        Some(v) if v < 1000.0 => {
            if v < 10.0 {
                format!("{:.1}ms", v)
            } else {
                format!("{:.0}ms", v)
            }
        }
        Some(v) => format!("{:.2}s", v / 1000.0),
    }
}

pub fn format_tokens_per_second(value: Option<f64>) -> String {
    match value {
        Some(v) if v > 0.0 => format!("{:.0} tok/s", v),
        _ => "-- tok/s".to_string(),
    }
}

/// Output-rate sanity check, matching `shouldHideOutputRate`.
pub fn should_hide_output_rate(
    output_rate: Option<f64>,
    duration_ms: Option<i64>,
    ttfb_ms: Option<i64>,
) -> bool {
    let (Some(output_rate), Some(duration_ms), Some(ttfb_ms)) = (output_rate, duration_ms, ttfb_ms)
    else {
        return false;
    };
    if !output_rate.is_finite() || duration_ms <= 0 {
        return false;
    }
    let generation_time_ms = duration_ms - ttfb_ms;
    if generation_time_ms <= 0 {
        return false;
    }
    let ratio = generation_time_ms as f64 / duration_ms as f64;
    ratio < 0.1 && output_rate > 5000.0
}

pub fn computed_tokens_per_second(
    output_tokens: i64,
    duration_ms: Option<i64>,
    ttfb_ms: Option<i64>,
) -> Option<f64> {
    if output_tokens <= 0 {
        return None;
    }
    let duration_ms = duration_ms?;
    if duration_ms <= 0 {
        return None;
    }
    let generation_ms = (duration_ms - ttfb_ms.unwrap_or(0)).max(1);
    let output_rate = output_tokens as f64 / (generation_ms as f64 / 1000.0);
    if should_hide_output_rate(Some(output_rate), Some(duration_ms), ttfb_ms) {
        None
    } else {
        Some(output_rate)
    }
}

pub fn normalized_tokens_per_second(
    raw: Option<f64>,
    output_tokens: i64,
    duration_ms: Option<i64>,
    ttfb_ms: Option<i64>,
) -> Option<f64> {
    if let Some(raw) = raw {
        if should_hide_output_rate(Some(raw), duration_ms, ttfb_ms) {
            return None;
        }
        if raw > 0.0 {
            return Some(raw);
        }
    }
    computed_tokens_per_second(output_tokens, duration_ms, ttfb_ms)
}

pub fn cache_hit_rate(cache_read_tokens: i64, input_tokens: i64) -> f64 {
    let total = input_tokens + cache_read_tokens;
    if total <= 0 {
        0.0
    } else {
        (cache_read_tokens as f64 / total as f64).clamp(0.0, 1.0)
    }
}

pub fn format_multiplier(value: f64) -> String {
    if (value - 1.0).abs() < 0.001 {
        "x1".to_string()
    } else if value.round() == value {
        format!("x{:.0}", value)
    } else {
        format!("x{:.2}", value)
    }
}

pub fn multiplier_level(value: f64) -> i32 {
    if value <= 0.5 {
        0
    } else if value <= 1.0 {
        1
    } else if value <= 2.0 {
        2
    } else {
        3
    }
}

pub fn format_duration(seconds: f64) -> String {
    let total = seconds.floor().max(0.0) as i64;
    if total < 60 {
        return format!("{}s", total);
    }
    let minutes = total / 60;
    let secs = total % 60;
    if minutes < 60 {
        return format!("{}m{:02}s", minutes, secs);
    }
    let hours = minutes / 60;
    format!("{}h{:02}m", hours, minutes % 60)
}

pub fn normalize_release_version(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.to_lowercase().starts_with('v') {
        trimmed[1..].to_string()
    } else {
        trimmed.to_string()
    }
}

fn semver_components(value: &str) -> Vec<i64> {
    let main = value.split('-').next().unwrap_or(value);
    main.split('.').map(|c| c.parse::<i64>().unwrap_or(0)).collect()
}

/// Compare two semver-ish strings, matching `compareSemver`.
pub fn compare_semver(lhs: &str, rhs: &str) -> Ordering {
    let lhs_parts = semver_components(lhs);
    let rhs_parts = semver_components(rhs);
    let count = lhs_parts.len().max(rhs_parts.len());
    for index in 0..count {
        let l = lhs_parts.get(index).copied().unwrap_or(0);
        let r = rhs_parts.get(index).copied().unwrap_or(0);
        match l.cmp(&r) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    Ordering::Equal
}

/// Parse a CCH timestamp into a UTC datetime. Accepts fractional ISO8601, plain
/// ISO8601, and `yyyy-MM-dd HH:mm:ss` (interpreted as UTC for stable ordering).
pub fn parse_cch_date(raw: &str) -> Option<DateTime<Utc>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Some(dt.with_timezone(&Utc));
    }
    // Try `yyyy-MM-dd HH:mm:ss`
    if let Ok(naive) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
        return Some(DateTime::from_naive_utc_and_offset(naive, Utc));
    }
    // Try with fractional seconds in space form
    if let Ok(naive) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(DateTime::from_naive_utc_and_offset(naive, Utc));
    }
    None
}

/// Period start time as Unix epoch milliseconds, matching `leaderboardStartDate`.
/// `daily` = start of today (UTC), `weekly` = start of ISO week (Monday),
/// `monthly` = first of the month; anything else (e.g. `allTime`) = `None`.
pub fn leaderboard_start_unix_ms(period: &str) -> Option<i64> {
    use chrono::{Datelike, Duration, NaiveTime};
    let now = Utc::now();
    let start_of_day = now
        .date_naive()
        .and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    let naive = match period {
        "daily" => start_of_day,
        "weekly" => {
            let weekday = now.date_naive().weekday();
            let days_from_monday = weekday.num_days_from_monday() as i64;
            start_of_day - Duration::days(days_from_monday)
        }
        "monthly" => now
            .date_naive()
            .with_day(1)
            .unwrap_or_else(|| now.date_naive())
            .and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
        _ => return None,
    };
    Some(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc).timestamp_millis())
}

/// Provider group default check, matching `isDefaultProviderGroup`.
pub fn is_default_provider_group(value: &str) -> bool {
    let normalized = value.trim().to_lowercase();
    normalized.is_empty() || normalized == "默认" || normalized == "default"
}

/// Split a comma-separated `group_tag` into display titles, matching
/// `providerGroupTitles`.
pub fn provider_group_titles(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || is_default_provider_group(trimmed) {
        return vec!["默认".to_string()];
    }
    let groups: Vec<String> = trimmed
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && !is_default_provider_group(s))
        .collect();
    if groups.is_empty() {
        vec!["默认".to_string()]
    } else {
        groups
    }
}

pub fn compact_provider_name(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "Provider".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_under_one_dollar_splits_minor() {
        // $0.001234 -> major "$0.001", minor "234"
        let (major, minor) = money_display_parts(0.001234);
        assert_eq!(major, "$0.001");
        assert_eq!(minor.as_deref(), Some("234"));
    }

    #[test]
    fn money_thousands_use_k() {
        assert_eq!(format_money(1234.0), "$1.2k");
    }

    #[test]
    fn money_hundreds_no_decimals() {
        assert_eq!(format_money(150.0), "$150");
    }

    #[test]
    fn money_zero() {
        assert_eq!(format_money(0.0), "$0.00");
    }

    #[test]
    fn money_small_no_minor() {
        // $1.25 -> fraction has <=3 digits, no minor
        let (major, minor) = money_display_parts(1.25);
        assert_eq!(major, "$1.25");
        assert_eq!(minor, None);
    }

    #[test]
    fn compact_number_thresholds() {
        assert_eq!(compact_number(999), "999");
        assert_eq!(compact_number(1500), "1.5k");
        assert_eq!(compact_number(2_500_000), "2.5M");
    }

    #[test]
    fn percent_and_latency() {
        assert_eq!(format_percent(0.8567), "85.7%");
        assert_eq!(format_latency(0), "-");
        assert_eq!(format_latency(1500), "1.5s");
        assert_eq!(format_milliseconds_as_seconds(Some(2340)), "2.34s");
        assert_eq!(format_milliseconds_as_seconds(None), "-");
    }

    #[test]
    fn multiplier_formatting_and_levels() {
        assert_eq!(format_multiplier(1.0), "x1");
        assert_eq!(format_multiplier(2.0), "x2");
        assert_eq!(format_multiplier(1.25), "x1.25");
        assert_eq!(multiplier_level(0.4), 0);
        assert_eq!(multiplier_level(1.0), 1);
        assert_eq!(multiplier_level(2.0), 2);
        assert_eq!(multiplier_level(5.0), 3);
    }

    #[test]
    fn duration_formatting() {
        assert_eq!(format_duration(45.0), "45s");
        assert_eq!(format_duration(125.0), "2m05s");
        assert_eq!(format_duration(3725.0), "1h02m");
    }

    #[test]
    fn semver_comparison() {
        assert_eq!(compare_semver("1.2.0", "1.1.9"), Ordering::Greater);
        assert_eq!(compare_semver("1.1.12", "1.1.12"), Ordering::Equal);
        assert_eq!(compare_semver("1.0", "1.0.1"), Ordering::Less);
        assert_eq!(normalize_release_version("v1.2.3"), "1.2.3");
        assert_eq!(normalize_release_version("1.2.3"), "1.2.3");
    }

    #[test]
    fn hide_output_rate_for_implausible_values() {
        // generation tiny vs duration, huge rate -> hide
        assert!(should_hide_output_rate(Some(9000.0), Some(1000), Some(950)));
        // normal -> keep
        assert!(!should_hide_output_rate(Some(80.0), Some(1000), Some(200)));
    }

    #[test]
    fn tokens_per_second_normalization() {
        // raw present and plausible
        assert_eq!(
            normalized_tokens_per_second(Some(50.0), 100, Some(2000), Some(100)),
            Some(50.0)
        );
        // raw zero, compute from tokens
        let computed = normalized_tokens_per_second(Some(0.0), 100, Some(1000), Some(0));
        assert!(computed.is_some());
        assert!((computed.unwrap() - 100.0).abs() < 0.01);
    }

    #[test]
    fn cache_hit_rate_clamped() {
        assert_eq!(cache_hit_rate(0, 0), 0.0);
        assert_eq!(cache_hit_rate(50, 50), 0.5);
    }

    #[test]
    fn provider_group_helpers() {
        assert!(is_default_provider_group("default"));
        assert!(is_default_provider_group("默认"));
        assert!(is_default_provider_group(""));
        assert!(!is_default_provider_group("prod"));
        assert_eq!(provider_group_titles("default"), vec!["默认".to_string()]);
        assert_eq!(
            provider_group_titles("a, b ,default"),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn date_parsing_orders_correctly() {
        let a = parse_cch_date("2024-01-01T10:00:00Z").unwrap();
        let b = parse_cch_date("2024-01-01 11:00:00").unwrap();
        assert!(b > a);
        assert!(parse_cch_date("").is_none());
        assert!(parse_cch_date("not-a-date").is_none());
    }
}
