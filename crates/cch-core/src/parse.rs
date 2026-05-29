//! Row-to-model parsers and the aggregation/cache helpers ported from
//! `APIService.swift` and the private helpers in `MonitorState.swift`.

use crate::jsonx::*;
use crate::models::*;
use serde_json::{Map, Value};
use std::collections::HashMap;

pub fn parse_active_session(row: &Map<String, Value>) -> CchActiveSession {
    CchActiveSession {
        session_id: string_value(row.get("sessionId"), ""),
        provider_id: int_value(row.get("providerId"), 0),
        user_name: string_value(row.get("userName"), ""),
        key_name: string_value(row.get("keyName"), ""),
        provider_name: string_value(row.get("providerName"), ""),
        model: string_value(row.get("model"), ""),
        api_type: string_value(row.get("apiType"), ""),
        start_time: int_value(row.get("startTime"), 0),
        input_tokens: int_value(row.get("inputTokens"), 0),
        output_tokens: int_value(row.get("outputTokens"), 0),
        total_tokens: int_value(row.get("totalTokens"), 0),
        cost_usd: double_value(row.get("costUsd"), 0.0),
        duration_ms: int_value(row.get("durationMs"), 0),
        request_count: int_value(row.get("requestCount"), 0),
        concurrent_count: int_value(row.get("concurrentCount"), 0),
        status: string_value(row.get("status"), ""),
    }
}

pub fn parse_overview(dict: &Map<String, Value>) -> CchOverview {
    CchOverview {
        concurrent_sessions: int_value(dict.get("concurrentSessions"), 0),
        today_requests: int_value(dict.get("todayRequests"), 0),
        today_cost: double_value(dict.get("todayCost"), 0.0),
        avg_response_time: int_value(dict.get("avgResponseTime"), 0),
        today_error_rate: double_value(dict.get("todayErrorRate"), 0.0),
        recent_minute_requests: int_value(dict.get("recentMinuteRequests"), 0),
        yesterday_same_period_requests: int_value(dict.get("yesterdaySamePeriodRequests"), 0),
        yesterday_same_period_cost: double_value(dict.get("yesterdaySamePeriodCost"), 0.0),
    }
}

/// Sum of every cache-creation token variant, matching `cacheCreationTokens`.
pub fn cache_creation_tokens(row: &Map<String, Value>, prefix: &str) -> i64 {
    int_value(row.get(&format!("{prefix}CacheCreationTokens")), 0)
        + int_value(row.get(&format!("{prefix}CacheCreation1hTokens")), 0)
        + int_value(row.get(&format!("{prefix}CacheCreation5mTokens")), 0)
        + int_value(row.get("cacheCreationInputTokens"), 0)
        + int_value(row.get("cacheCreation1hInputTokens"), 0)
        + int_value(row.get("cacheCreation5mInputTokens"), 0)
}

pub fn parse_log(row: &Map<String, Value>) -> LogEntry {
    let chain_rows = row
        .get("providerChain")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_object)
                .enumerate()
                .map(|(idx, item)| ProviderChainItem {
                    id: format!("chain-{idx}"),
                    name: string_value(item.get("name"), "Provider"),
                    provider_type: string_value(item.get("providerType"), ""),
                    reason: string_value(item.get("reason"), ""),
                    circuit_state: string_value(item.get("circuitState"), ""),
                    priority: int_value(item.get("priority"), 0),
                    weight: int_value(item.get("weight"), 0),
                    group_tag: string_value(item.get("groupTag"), ""),
                    cost_multiplier: double_value(item.get("costMultiplier"), 1.0),
                    status_code: optional_int(item.get("statusCode")),
                    attempt_number: optional_int(item.get("attemptNumber")),
                    error_message: string_value(item.get("errorMessage"), ""),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let tokens_per_second = optional_double(row.get("tokensPerSecond"))
        .or_else(|| optional_double(row.get("tokensPerSecondTokens")))
        .or_else(|| optional_double(row.get("outputTokensPerSecond")));

    LogEntry {
        id: int_value(row.get("id"), 0),
        created_at: string_value(row.get("createdAt"), ""),
        session_id: string_value(row.get("sessionId"), ""),
        request_sequence: int_value(row.get("requestSequence"), 0),
        user_name: string_value(row.get("userName"), ""),
        key_name: string_value(row.get("keyName"), ""),
        provider_name: string_value(row.get("providerName"), ""),
        model: string_value(row.get("model"), ""),
        original_model: string_value(row.get("originalModel"), ""),
        endpoint: string_value(row.get("endpoint"), ""),
        status_code: optional_int(row.get("statusCode")),
        messages_count: int_value(row.get("messagesCount"), 0),
        input_tokens: int_value(row.get("inputTokens"), 0),
        output_tokens: int_value(row.get("outputTokens"), 0),
        total_tokens: int_value(row.get("totalTokens"), 0),
        cache_creation_tokens: cache_creation_tokens(row, ""),
        cache_read_tokens: int_value(row.get("cacheReadInputTokens"), 0),
        cost_usd: double_value(row.get("costUsd"), 0.0),
        duration_ms: optional_int(row.get("durationMs")),
        ttfb_ms: optional_int(row.get("ttfbMs")),
        tokens_per_second,
        is_fast_tier: is_fast_tier_log(row),
        error_message: string_value(row.get("errorMessage"), ""),
        provider_chain: chain_rows,
    }
}

fn is_fast_tier_text(value: &str) -> bool {
    let normalized = value.trim().to_lowercase();
    matches!(
        normalized.as_str(),
        "priority" | "fast" | "service_tier:priority" | "service-tier:priority"
    )
}

fn is_fast_tier_change(change: &Map<String, Value>) -> bool {
    let path = string_value(change.get("path"), "").to_lowercase();
    if !matches!(path.as_str(), "service_tier" | "service-tier" | "servicetier") {
        return false;
    }
    is_fast_tier_text(&string_value(change.get("after"), ""))
        || is_fast_tier_text(&string_value(change.get("value"), ""))
}

fn has_priority_service_tier_special_setting(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Array(arr)) => arr
            .iter()
            .any(|v| has_priority_service_tier_special_setting(Some(v))),
        Some(Value::Object(setting)) => {
            let type_str = string_value(setting.get("type"), "").to_lowercase();
            if type_str == "codex_service_tier_result"
                || type_str.contains("service_tier")
                || type_str.contains("service-tier")
            {
                let tier_values = [
                    string_value(setting.get("requestedServiceTier"), ""),
                    string_value(setting.get("serviceTier"), ""),
                    string_value(setting.get("resolvedServiceTier"), ""),
                    string_value(setting.get("service_tier"), ""),
                    string_value(setting.get("value"), ""),
                    string_value(setting.get("after"), ""),
                ];
                if tier_values.iter().any(|v| is_fast_tier_text(v)) {
                    return true;
                }
            }
            if let Some(changes) = setting.get("changes").and_then(Value::as_array) {
                if changes
                    .iter()
                    .filter_map(Value::as_object)
                    .any(is_fast_tier_change)
                {
                    return true;
                }
            }
            setting
                .values()
                .any(|v| has_priority_service_tier_special_setting(Some(v)))
        }
        Some(Value::String(raw)) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return false;
            }
            if let Ok(json) = serde_json::from_str::<Value>(trimmed) {
                if has_priority_service_tier_special_setting(Some(&json)) {
                    return true;
                }
            }
            let lower = trimmed.to_lowercase();
            (lower.contains("service_tier")
                || lower.contains("service-tier")
                || lower.contains("servicetier"))
                && (lower.contains("priority") || lower.contains("fast"))
        }
        _ => false,
    }
}

pub fn is_fast_tier_log(row: &Map<String, Value>) -> bool {
    let bool_keys = [
        "isFastTier",
        "fastTier",
        "isPriorityTier",
        "priorityServiceTier",
    ];
    if bool_keys.iter().any(|k| bool_value(row.get(*k), false)) {
        return true;
    }
    let tier_keys = [
        "serviceTier",
        "service_tier",
        "requestedServiceTier",
        "resolvedServiceTier",
        "codexServiceTier",
        "codexServiceTierPreference",
        "openaiServiceTier",
    ];
    if tier_keys
        .iter()
        .any(|k| is_fast_tier_text(&string_value(row.get(*k), "")))
    {
        return true;
    }
    has_priority_service_tier_special_setting(row.get("specialSettings"))
}

pub fn is_endpoint_enabled(row: &Map<String, Value>) -> bool {
    for key in ["isEnabled", "enabled"] {
        if row.get(key).is_some() {
            return bool_value(row.get(key), false);
        }
    }
    true
}

/// Normalize a cache-hit-rate value to a `0..=1` fraction, matching
/// `optionalCacheHitRate`.
pub fn optional_cache_hit_rate(row: &Map<String, Value>) -> Option<f64> {
    let keys = [
        "cacheHitRate",
        "cacheHitRatio",
        "cacheReadRate",
        "cacheRate",
        "cacheHitPercentage",
    ];
    for key in keys {
        if let Some(raw) = optional_double(row.get(key)) {
            let normalized = if raw > 1.0 { raw / 100.0 } else { raw };
            return Some(normalized.clamp(0.0, 1.0));
        }
    }
    None
}

pub fn parse_provider_group_string(value: &str) -> Option<ProviderGroup> {
    let name = value.trim();
    if name.is_empty() {
        return None;
    }
    Some(ProviderGroup {
        id: name.to_string(),
        name: name.to_string(),
        provider_count: None,
        cost_multiplier: None,
    })
}

pub fn parse_provider_group_row(row: &Map<String, Value>) -> Option<ProviderGroup> {
    let name = first_string_value(row, &["name", "group", "groupTag", "group_tag", "title"], "默认");
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    let id = first_string_value(row, &["id", "name", "group", "groupTag", "group_tag"], trimmed);
    Some(ProviderGroup {
        id,
        name: trimmed.to_string(),
        provider_count: first_optional_int(row, &["providerCount", "count"]),
        cost_multiplier: first_optional_double(row, &["costMultiplier", "cost_multiplier"]),
    })
}

pub fn build_limit_text(row: &Map<String, Value>) -> String {
    let mut pieces: Vec<String> = Vec::new();
    let daily = double_value(row.get("limitDailyUsd"), 0.0);
    let total = double_value(row.get("limitTotalUsd"), 0.0);
    let rpm = int_value(row.get("rpm"), 0);
    if daily > 0.0 {
        pieces.push(format!("日 ${:.0}", daily));
    }
    if total > 0.0 {
        pieces.push(format!("总 ${:.0}", total));
    }
    if rpm > 0 {
        pieces.push(format!("RPM {}", rpm));
    }
    if pieces.is_empty() {
        "无限制".to_string()
    } else {
        pieces.join(" · ")
    }
}

// ---- Leaderboard parsing/aggregation ----

fn leaderboard_stable_id(row: &Map<String, Value>, scope: &str, title: &str) -> String {
    match scope {
        "provider" | "providerCacheHitRate" => {
            let provider_id = int_value(row.get("providerId"), 0);
            if provider_id > 0 {
                format!("provider-{provider_id}")
            } else {
                format!("provider-{}", title.to_lowercase())
            }
        }
        "user" | "userCacheHitRate" => {
            let user_id = int_value(row.get("userId"), 0);
            if user_id > 0 {
                format!("user-{user_id}")
            } else {
                format!("user-{}", title.to_lowercase())
            }
        }
        "model" => format!("model-{}", title.to_lowercase()),
        _ => title.to_lowercase(),
    }
}

pub fn parse_leaderboard_model_stats(value: Option<&Value>, parent_id: &str) -> Vec<LeaderboardModelStat> {
    let Some(rows) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    let parsed: Vec<LeaderboardModelStat> = rows
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|row| {
            let model = string_value(row.get("model"), "Model");
            if model.trim().is_empty() {
                return None;
            }
            let total_input_tokens =
                int_value(row.get("totalInputTokens"), int_value(row.get("totalTokens"), 0));
            Some(LeaderboardModelStat {
                id: format!("{parent_id}-model-{}", model.to_lowercase()),
                model: model.clone(),
                requests: int_value(row.get("totalRequests"), 0),
                cost: double_value(row.get("totalCost"), 0.0),
                tokens: int_value(row.get("totalTokens"), total_input_tokens),
                input_tokens: total_input_tokens,
                cache_creation_tokens: int_value(row.get("cacheCreationTokens"), 0),
                cache_read_tokens: int_value(row.get("cacheReadTokens"), 0),
                cache_hit_rate_override: optional_cache_hit_rate(row),
            })
        })
        .collect();

    // Dedup by model (lowercase), merging, then sort by cost desc, requests desc.
    let mut by_model: HashMap<String, LeaderboardModelStat> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for stat in parsed {
        let key = stat.model.to_lowercase();
        match by_model.get_mut(&key) {
            Some(existing) => *existing = merge_leaderboard_model_stat(existing, &stat),
            None => {
                order.push(key.clone());
                by_model.insert(key, stat);
            }
        }
    }
    let mut result: Vec<LeaderboardModelStat> = order
        .into_iter()
        .filter_map(|k| by_model.remove(&k))
        .collect();
    result.sort_by(|a, b| {
        b.cost
            .partial_cmp(&a.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.requests.cmp(&a.requests))
    });
    result
}

pub fn parse_official_leaderboard_rows(rows: &[Map<String, Value>], scope: &str) -> Vec<LeaderboardEntry> {
    rows.iter()
        .map(|row| {
            let (title, subtitle) = match scope {
                "providerCacheHitRate" => {
                    (string_value(row.get("providerName"), "Provider"), "缓存命中榜".to_string())
                }
                "userCacheHitRate" => {
                    (string_value(row.get("userName"), "User"), "缓存命中榜".to_string())
                }
                "provider" => {
                    let success = double_value(row.get("successRate"), 0.0) * 100.0;
                    (
                        string_value(row.get("providerName"), "Provider"),
                        format!("success {:.1}%", success),
                    )
                }
                "model" => (string_value(row.get("model"), "Model"), "model".to_string()),
                _ => (string_value(row.get("userName"), "User"), "user".to_string()),
            };
            let stable_id = leaderboard_stable_id(row, scope, &title);
            let total_tokens = int_value(row.get("totalTokens"), int_value(row.get("totalInputTokens"), 0));
            let total_input_tokens = int_value(row.get("totalInputTokens"), total_tokens);
            let cache_hit_rate_override = if matches!(scope, "user" | "provider" | "model") {
                None
            } else {
                optional_cache_hit_rate(row)
            };
            let success_rate = if row.get("successRate").is_none() {
                None
            } else {
                Some(double_value(row.get("successRate"), 0.0))
            };
            LeaderboardEntry {
                id: stable_id.clone(),
                title,
                subtitle,
                requests: int_value(row.get("totalRequests"), 0),
                cost: double_value(row.get("totalCost"), 0.0),
                tokens: total_tokens,
                input_tokens: total_input_tokens,
                cache_creation_tokens: int_value(row.get("cacheCreationTokens"), 0),
                cache_read_tokens: int_value(row.get("cacheReadTokens"), 0),
                cache_hit_rate_override,
                success_rate,
                model_stats: parse_leaderboard_model_stats(row.get("modelStats"), &stable_id),
            }
        })
        .collect()
}

pub fn leaderboard_extra_query_items(scope: &str) -> Vec<(&'static str, &'static str)> {
    match scope {
        "user" | "userCacheHitRate" => vec![("includeUserModelStats", "1")],
        "provider" => vec![("includeModelStats", "1")],
        _ => Vec::new(),
    }
}

pub fn aggregate_leaderboard(rows: &[Map<String, Value>], scope: &str) -> Vec<LeaderboardEntry> {
    let mut entries: HashMap<String, LeaderboardEntry> = HashMap::new();
    for row in rows {
        let (title, subtitle, id) = match scope {
            "provider" => {
                let title = string_value(row.get("providerName"), "Provider");
                let provider_id = int_value(row.get("providerId"), 0);
                let id = if provider_id > 0 {
                    format!("provider-{provider_id}")
                } else {
                    format!("provider-{}", title.to_lowercase())
                };
                (title, "provider".to_string(), id)
            }
            "model" => {
                let title = string_value(
                    row.get("model"),
                    &string_value(row.get("originalModel"), "Model"),
                );
                let id = format!("model-{}", title.to_lowercase());
                (title, "model".to_string(), id)
            }
            _ => {
                let title = string_value(row.get("userName"), "User");
                let user_id = int_value(row.get("userId"), 0);
                let id = if user_id > 0 {
                    format!("user-{user_id}")
                } else {
                    format!("user-{}", title.to_lowercase())
                };
                (title, "user".to_string(), id)
            }
        };
        let entry = LeaderboardEntry {
            id: id.clone(),
            title,
            subtitle,
            requests: 1,
            cost: double_value(row.get("costUsd"), 0.0),
            tokens: int_value(row.get("totalTokens"), 0),
            input_tokens: int_value(row.get("inputTokens"), 0),
            cache_creation_tokens: cache_creation_tokens(row, ""),
            cache_read_tokens: int_value(row.get("cacheReadInputTokens"), 0),
            cache_hit_rate_override: None,
            success_rate: None,
            model_stats: Vec::new(),
        };
        match entries.get_mut(&id) {
            Some(existing) => *existing = merge_leaderboard_entries(existing, &entry),
            None => {
                entries.insert(id, entry);
            }
        }
    }
    let mut result: Vec<LeaderboardEntry> = entries.into_values().collect();
    result.sort_by(|a, b| {
        b.cost
            .partial_cmp(&a.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.tokens.cmp(&a.tokens))
            .then(b.requests.cmp(&a.requests))
    });
    result
}

pub fn merge_leaderboard_model_stat(
    lhs: &LeaderboardModelStat,
    rhs: &LeaderboardModelStat,
) -> LeaderboardModelStat {
    LeaderboardModelStat {
        id: lhs.id.clone(),
        model: lhs.model.clone(),
        requests: lhs.requests + rhs.requests,
        cost: lhs.cost + rhs.cost,
        tokens: lhs.tokens + rhs.tokens,
        input_tokens: lhs.input_tokens + rhs.input_tokens,
        cache_creation_tokens: lhs.cache_creation_tokens + rhs.cache_creation_tokens,
        cache_read_tokens: lhs.cache_read_tokens + rhs.cache_read_tokens,
        cache_hit_rate_override: rhs.cache_hit_rate_override.or(lhs.cache_hit_rate_override),
    }
}

pub fn merge_leaderboard_model_stats(
    primary: &[LeaderboardModelStat],
    cache: &[LeaderboardModelStat],
) -> Vec<LeaderboardModelStat> {
    if cache.is_empty() {
        return primary.to_vec();
    }
    let mut cache_by_model: HashMap<String, LeaderboardModelStat> = HashMap::new();
    for stat in cache {
        let key = stat.model.to_lowercase();
        match cache_by_model.get_mut(&key) {
            Some(existing) => *existing = merge_leaderboard_model_stat(existing, stat),
            None => {
                cache_by_model.insert(key, stat.clone());
            }
        }
    }
    if primary.is_empty() {
        return cache.to_vec();
    }
    primary
        .iter()
        .map(|stat| {
            let Some(cache_stat) = cache_by_model.get(&stat.model.to_lowercase()) else {
                return stat.clone();
            };
            let prefer_cache = cache_stat.input_tokens > 0 || cache_stat.cache_hit_rate_override.is_some();
            LeaderboardModelStat {
                id: stat.id.clone(),
                model: stat.model.clone(),
                requests: stat.requests,
                cost: stat.cost,
                tokens: stat.tokens,
                input_tokens: if cache_stat.input_tokens > 0 {
                    cache_stat.input_tokens
                } else {
                    stat.input_tokens
                },
                cache_creation_tokens: if prefer_cache {
                    cache_stat.cache_creation_tokens
                } else {
                    stat.cache_creation_tokens
                },
                cache_read_tokens: if prefer_cache {
                    cache_stat.cache_read_tokens
                } else {
                    stat.cache_read_tokens
                },
                cache_hit_rate_override: cache_stat
                    .cache_hit_rate_override
                    .or(stat.cache_hit_rate_override),
            }
        })
        .collect()
}

pub fn merge_leaderboard_entries(lhs: &LeaderboardEntry, rhs: &LeaderboardEntry) -> LeaderboardEntry {
    LeaderboardEntry {
        id: lhs.id.clone(),
        title: lhs.title.clone(),
        subtitle: lhs.subtitle.clone(),
        requests: lhs.requests + rhs.requests,
        cost: lhs.cost + rhs.cost,
        tokens: lhs.tokens + rhs.tokens,
        input_tokens: lhs.input_tokens + rhs.input_tokens,
        cache_creation_tokens: lhs.cache_creation_tokens + rhs.cache_creation_tokens,
        cache_read_tokens: lhs.cache_read_tokens + rhs.cache_read_tokens,
        cache_hit_rate_override: rhs.cache_hit_rate_override.or(lhs.cache_hit_rate_override),
        success_rate: lhs.success_rate.or(rhs.success_rate),
        model_stats: merge_leaderboard_model_stats(&lhs.model_stats, &rhs.model_stats),
    }
}

/// Merge cache-scope data into a primary entry, matching `mergingCacheData`.
pub fn merge_cache_data(entry: &LeaderboardEntry, cache: &LeaderboardEntry) -> LeaderboardEntry {
    let prefer_cache = cache.input_tokens > 0 || cache.cache_hit_rate_override.is_some();
    LeaderboardEntry {
        id: entry.id.clone(),
        title: entry.title.clone(),
        subtitle: entry.subtitle.clone(),
        requests: entry.requests,
        cost: entry.cost,
        tokens: entry.tokens,
        input_tokens: if cache.input_tokens > 0 {
            cache.input_tokens
        } else {
            entry.input_tokens
        },
        cache_creation_tokens: if prefer_cache {
            cache.cache_creation_tokens
        } else {
            entry.cache_creation_tokens
        },
        cache_read_tokens: if prefer_cache {
            cache.cache_read_tokens
        } else {
            entry.cache_read_tokens
        },
        cache_hit_rate_override: cache.cache_hit_rate_override.or(entry.cache_hit_rate_override),
        success_rate: entry.success_rate,
        model_stats: merge_leaderboard_model_stats(&entry.model_stats, &cache.model_stats),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn fast_tier_detected_from_bool_and_tier_and_special() {
        assert!(is_fast_tier_log(&obj(json!({"isFastTier": true}))));
        assert!(is_fast_tier_log(&obj(json!({"serviceTier": "priority"}))));
        assert!(!is_fast_tier_log(&obj(json!({"serviceTier": "standard"}))));
        let special = json!({
            "specialSettings": [{"type": "codex_service_tier_result", "requestedServiceTier": "priority"}]
        });
        assert!(is_fast_tier_log(&obj(special)));
    }

    #[test]
    fn cache_hit_rate_normalizes_percent() {
        assert_eq!(optional_cache_hit_rate(&obj(json!({"cacheHitRate": 85.0}))), Some(0.85));
        assert_eq!(optional_cache_hit_rate(&obj(json!({"cacheHitRate": 0.5}))), Some(0.5));
        assert_eq!(optional_cache_hit_rate(&obj(json!({}))), None);
    }

    #[test]
    fn aggregate_leaderboard_groups_and_sorts() {
        let rows = vec![
            obj(json!({"providerName": "A", "providerId": 1, "costUsd": 1.0, "totalTokens": 10})),
            obj(json!({"providerName": "A", "providerId": 1, "costUsd": 2.0, "totalTokens": 20})),
            obj(json!({"providerName": "B", "providerId": 2, "costUsd": 5.0, "totalTokens": 5})),
        ];
        let result = aggregate_leaderboard(&rows, "provider");
        assert_eq!(result.len(), 2);
        // B has higher cost -> first
        assert_eq!(result[0].title, "B");
        assert_eq!(result[0].cost, 5.0);
        // A merged: 2 requests, cost 3.0
        assert_eq!(result[1].title, "A");
        assert_eq!(result[1].requests, 2);
        assert_eq!(result[1].cost, 3.0);
    }

    #[test]
    fn build_limit_text_combines_pieces() {
        assert_eq!(build_limit_text(&obj(json!({}))), "无限制");
        assert_eq!(
            build_limit_text(&obj(json!({"limitDailyUsd": 10.0, "rpm": 60}))),
            "日 $10 · RPM 60"
        );
    }

    #[test]
    fn limit_text_total_only() {
        assert_eq!(
            build_limit_text(&obj(json!({"limitTotalUsd": 500.0}))),
            "总 $500"
        );
    }
}
