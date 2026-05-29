//! Domain models ported from the Swift `APIService.swift` and `MonitorState.swift`.
//!
//! All types derive `Serialize` so they can be sent to the webview, and use
//! `camelCase` field names to match the JS frontend and the original UI.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CchConfig {
    pub base_url: String,
    pub token: String,
    pub env_path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CchOverview {
    pub concurrent_sessions: i64,
    pub today_requests: i64,
    pub today_cost: f64,
    pub avg_response_time: i64,
    pub today_error_rate: f64,
    pub recent_minute_requests: i64,
    pub yesterday_same_period_requests: i64,
    pub yesterday_same_period_cost: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CchActiveSession {
    pub session_id: String,
    pub provider_id: i64,
    pub user_name: String,
    pub key_name: String,
    pub provider_name: String,
    pub model: String,
    pub api_type: String,
    pub start_time: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub cost_usd: f64,
    pub duration_ms: i64,
    pub request_count: i64,
    pub concurrent_count: i64,
    pub status: String,
}

/// Cache visibility state for the breathing menu-bar indicator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CacheVisibilityState {
    #[default]
    Normal,
    Rebuilding,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStatusContext {
    pub state: CacheVisibilityState,
    pub created_tokens: i64,
    pub read_tokens: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusRunningItem {
    pub id: String,
    pub log_id: Option<i64>,
    pub provider_name: String,
    pub model: String,
    pub multiplier: f64,
    pub is_retrying: bool,
    /// Unix epoch milliseconds for the start time, or `None`.
    pub started_at_ms: Option<i64>,
    pub cache_state: CacheVisibilityState,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardModelStat {
    pub id: String,
    pub model: String,
    pub requests: i64,
    pub cost: f64,
    pub tokens: i64,
    pub input_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_hit_rate_override: Option<f64>,
}

impl LeaderboardModelStat {
    pub fn cache_hit_rate(&self) -> Option<f64> {
        self.cache_hit_rate_override
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardEntry {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub requests: i64,
    pub cost: f64,
    pub tokens: i64,
    pub input_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_hit_rate_override: Option<f64>,
    pub success_rate: Option<f64>,
    pub model_stats: Vec<LeaderboardModelStat>,
}

impl LeaderboardEntry {
    pub fn cache_hit_rate(&self) -> Option<f64> {
        self.cache_hit_rate_override
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSummary {
    pub total_requests: i64,
    pub total_cost: f64,
    pub total_tokens: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderChainItem {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub reason: String,
    pub circuit_state: String,
    pub priority: i64,
    pub weight: i64,
    pub group_tag: String,
    pub cost_multiplier: f64,
    pub status_code: Option<i64>,
    pub attempt_number: Option<i64>,
    pub error_message: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: i64,
    pub created_at: String,
    pub session_id: String,
    pub request_sequence: i64,
    pub user_name: String,
    pub key_name: String,
    pub provider_name: String,
    pub model: String,
    pub original_model: String,
    pub endpoint: String,
    pub status_code: Option<i64>,
    pub messages_count: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub cost_usd: f64,
    pub duration_ms: Option<i64>,
    pub ttfb_ms: Option<i64>,
    pub tokens_per_second: Option<f64>,
    pub is_fast_tier: bool,
    pub error_message: String,
    pub provider_chain: Vec<ProviderChainItem>,
}

/// Provider circuit-breaker health. `circuit_state` defaults to `"closed"`,
/// matching the original Swift default.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderHealth {
    pub circuit_state: String,
    pub failure_count: i64,
    pub last_failure_time: Option<i64>,
    pub circuit_open_until: Option<i64>,
    pub recovery_minutes: Option<i64>,
}

impl Default for ProviderHealth {
    fn default() -> Self {
        Self {
            circuit_state: "closed".to_string(),
            failure_count: 0,
            last_failure_time: None,
            circuit_open_until: None,
            recovery_minutes: None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: i64,
    pub name: String,
    pub provider_type: String,
    pub vendor_id: Option<i64>,
    pub api_url: String,
    pub website_url: String,
    pub is_enabled: bool,
    pub priority: i64,
    pub weight: i64,
    pub group_tag: String,
    pub cost_multiplier: f64,
    pub today_calls: i64,
    pub today_cost: f64,
    pub last_call_time: String,
    pub last_call_model: String,
    pub allowed_models: String,
    pub allowed_clients: String,
    pub model_redirects: String,
    pub limit_text: String,
    pub health: ProviderHealth,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderGroup {
    pub id: String,
    pub name: String,
    pub provider_count: Option<i64>,
    pub cost_multiplier: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsPage {
    pub logs: Vec<LogEntry>,
    pub total: i64,
    pub summary: LogSummary,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub ok: bool,
    pub method: String,
    pub status_code: Option<i64>,
    pub latency_ms: Option<f64>,
    pub error_message: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubRelease {
    pub tag: String,
    pub name: String,
    pub body: String,
    pub html_url: String,
    /// Unix epoch milliseconds, or `None` if unparsed.
    pub published_at_ms: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardSummary {
    pub requests: i64,
    pub cost: f64,
    pub tokens: i64,
    pub cache_hit_rate: Option<f64>,
}
