//! The monitor state machine, ported from `MonitorState.swift`.
//!
//! Holds the latest fetched data and derives the menu-bar snapshot, the
//! cache-rebuild detection map, the menu-bar "running logs", and the leaderboard
//! summary. The Tauri shell owns one `MonitorState` behind a mutex and serializes
//! the derived `StatusBarSnapshot` + tab data to the webview.

use crate::format::*;
use crate::models::*;
use std::collections::{HashMap, HashSet};

/// The complete serializable view-model handed to the webview each refresh.
#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewModel {
    pub overview: CchOverview,
    pub active_sessions: Vec<CchActiveSession>,
    pub leaderboard: Vec<LeaderboardEntry>,
    pub leaderboard_summary: LeaderboardSummary,
    pub logs: Vec<LogEntry>,
    pub recent_logs: Vec<LogEntry>,
    pub menu_bar_running_logs: Vec<LogEntry>,
    pub log_summary: LogSummary,
    pub log_total: i64,
    pub providers: Vec<Provider>,
    pub provider_groups: Vec<String>,
    pub cache_status: HashMap<String, CacheStatusContext>,
    pub status_bar: StatusBarSnapshot,
    pub error_message: Option<String>,
    pub has_cache_alert: bool,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusBarSnapshot {
    pub shows_details: bool,
    pub idle_primary: String,
    pub idle_detail: String,
    pub idle_cache_state: CacheVisibilityState,
    pub running_items: Vec<StatusRunningItem>,
    pub has_recent_logs: bool,
}

/// What the native tray should render right now.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum StatusBarPayload {
    Idle {
        primary: String,
        detail: String,
        cache_state: CacheVisibilityState,
    },
    Running {
        provider: String,
        detail: String,
        elapsed: String,
        is_retrying: bool,
        session_count: usize,
        cache_state: CacheVisibilityState,
    },
}

#[derive(Default)]
pub struct MonitorState {
    pub config: CchConfig,
    pub overview: CchOverview,
    pub active_sessions: Vec<CchActiveSession>,
    pub leaderboard: Vec<LeaderboardEntry>,
    pub logs: Vec<LogEntry>,
    pub recent_logs: Vec<LogEntry>,
    pub log_summary: LogSummary,
    pub log_total: i64,
    pub providers: Vec<Provider>,
    pub official_provider_groups: Vec<ProviderGroup>,
    pub error_message: Option<String>,

    pub show_status_bar_details: bool,
    pub leaderboard_period: String,
    pub leaderboard_scope: String,

    provider_multiplier_by_name: HashMap<String, f64>,
    provider_multiplier_by_id: HashMap<i64, f64>,
    cache_status: HashMap<i64, CacheStatusContext>,
    menu_bar_running_logs: Vec<LogEntry>,
    menu_bar_cache_alert_log_id: Option<i64>,
}

impl MonitorState {
    pub fn new(config: CchConfig) -> Self {
        Self {
            config,
            show_status_bar_details: true,
            leaderboard_period: "daily".to_string(),
            leaderboard_scope: "user".to_string(),
            ..Default::default()
        }
    }

    pub fn provider_multiplier(&self, provider_name: &str) -> f64 {
        let normalized = provider_name.trim();
        if normalized.is_empty() {
            return 1.0;
        }
        self.provider_multiplier_by_name
            .get(&normalized.to_lowercase())
            .copied()
            .unwrap_or(1.0)
    }

    pub fn set_overview(&mut self, overview: CchOverview) {
        self.overview = overview;
    }

    pub fn set_active_sessions(&mut self, sessions: Vec<CchActiveSession>) {
        self.active_sessions = sessions;
    }

    pub fn set_leaderboard(&mut self, leaderboard: Vec<LeaderboardEntry>) {
        self.leaderboard = leaderboard;
    }

    pub fn set_providers(&mut self, providers: Vec<Provider>) {
        self.providers = providers;
        self.rebuild_provider_lookup();
    }

    /// Update the recent-logs slice used for the status bar and dashboard.
    pub fn set_recent_logs(&mut self, logs: Vec<LogEntry>) {
        self.recent_logs = logs;
        self.rebuild_cache_status();
    }

    /// Replace the full logs page (with totals + summary).
    pub fn set_logs_page(&mut self, page: LogsPage, include_stats: bool) {
        self.logs = page.logs;
        self.log_total = page.total;
        if include_stats {
            self.log_summary = page.summary;
        }
        self.rebuild_cache_status();
    }

    fn rebuild_provider_lookup(&mut self) {
        self.provider_multiplier_by_name = self
            .providers
            .iter()
            .map(|p| (p.name.trim().to_lowercase(), p.cost_multiplier))
            .collect();
        self.provider_multiplier_by_id = self
            .providers
            .iter()
            .map(|p| (p.id, p.cost_multiplier))
            .collect();
    }

    pub fn cache_status(&self, log: &LogEntry) -> CacheStatusContext {
        if self.is_cache_alert_log(log.id) {
            return CacheStatusContext {
                state: CacheVisibilityState::Rebuilding,
                created_tokens: log.cache_creation_tokens,
                read_tokens: log.cache_read_tokens,
            };
        }
        self.cache_status.get(&log.id).copied().unwrap_or(CacheStatusContext {
            state: CacheVisibilityState::Normal,
            created_tokens: log.cache_creation_tokens,
            read_tokens: log.cache_read_tokens,
        })
    }

    fn is_cache_alert_log(&self, log_id: i64) -> bool {
        self.menu_bar_cache_alert_log_id == Some(log_id)
    }

    fn rebuild_cache_status(&mut self) {
        let combined = unique_logs(
            self.recent_logs
                .iter()
                .chain(self.logs.iter())
                .cloned()
                .collect(),
        );
        self.cache_status = build_cache_status_map(&combined);
        self.rebuild_menu_bar_running_logs();
        self.announce_latest_cache_alert(&combined);
    }

    fn rebuild_menu_bar_running_logs(&mut self) {
        let mut ordered: Vec<LogEntry> = self.recent_logs.clone();
        ordered.sort_by(|lhs, rhs| {
            if lhs.id != rhs.id {
                return rhs.id.cmp(&lhs.id);
            }
            if lhs.request_sequence != rhs.request_sequence {
                return rhs.request_sequence.cmp(&lhs.request_sequence);
            }
            let lhs_date = parse_cch_date(&lhs.created_at);
            let rhs_date = parse_cch_date(&rhs.created_at);
            rhs_date.cmp(&lhs_date)
        });

        let mut seen_sessions: HashSet<String> = HashSet::new();
        self.menu_bar_running_logs = ordered
            .into_iter()
            .filter(|log| {
                if log.status_code.is_some() {
                    return false;
                }
                let key = if log.session_id.is_empty() {
                    format!("log-{}", log.id)
                } else {
                    log.session_id.clone()
                };
                seen_sessions.insert(key)
            })
            .collect();
    }

    fn announce_latest_cache_alert(&mut self, combined: &[LogEntry]) {
        let latest = combined
            .iter()
            .filter(|log| {
                self.cache_status
                    .get(&log.id)
                    .map(|c| c.state == CacheVisibilityState::Rebuilding)
                    .unwrap_or(false)
            })
            .max_by(|a, b| {
                let a_date = parse_cch_date(&a.created_at);
                let b_date = parse_cch_date(&b.created_at);
                a_date.cmp(&b_date).then(a.id.cmp(&b.id))
            });
        if let Some(latest) = latest {
            self.menu_bar_cache_alert_log_id = Some(latest.id);
        }
    }

    pub fn menu_bar_running_logs(&self) -> &[LogEntry] {
        &self.menu_bar_running_logs
    }

    pub fn has_cache_alert(&self) -> bool {
        self.menu_bar_cache_alert_log_id.is_some()
    }

    // ---- Leaderboard summary ----

    pub fn leaderboard_summary(&self) -> LeaderboardSummary {
        let requests = self.leaderboard.iter().map(|e| e.requests).sum();
        let cost = self.leaderboard.iter().map(|e| e.cost).sum();
        let tokens = self.leaderboard.iter().map(|e| e.tokens).sum();
        let rows: Vec<&LeaderboardEntry> = self
            .leaderboard
            .iter()
            .filter(|e| e.cache_hit_rate_override.is_some() && e.input_tokens > 0)
            .collect();
        let total_input_tokens: i64 = rows.iter().map(|e| e.input_tokens).sum();
        let cache_hit_rate = if rows.is_empty() || total_input_tokens <= 0 {
            None
        } else {
            let weighted: f64 = rows
                .iter()
                .map(|e| e.cache_hit_rate_override.unwrap_or(0.0) * e.input_tokens as f64)
                .sum();
            Some((weighted / total_input_tokens as f64).clamp(0.0, 1.0))
        };
        LeaderboardSummary {
            requests,
            cost,
            tokens,
            cache_hit_rate,
        }
    }

    pub fn leaderboard_cache_hit_rate(&self, entry: &LeaderboardEntry) -> Option<f64> {
        entry.cache_hit_rate_override
    }

    // ---- Provider groups ----

    pub fn provider_groups(&self) -> Vec<String> {
        let mut set: HashSet<String> = HashSet::new();
        for provider in &self.providers {
            for title in self.display_group_titles(provider) {
                set.insert(title);
            }
        }
        let mut groups: Vec<String> = set.into_iter().filter(|g| g != "全部").collect();
        groups.sort();
        let mut result = vec!["全部".to_string()];
        result.extend(groups);
        result
    }

    pub fn display_group_titles(&self, provider: &Provider) -> Vec<String> {
        let mut titles: Vec<String> = provider_group_titles(&provider.group_tag)
            .into_iter()
            .filter(|t| !is_default_provider_group(t))
            .collect();
        titles.sort_by_key(|a| a.to_lowercase());
        if titles.is_empty() {
            vec!["默认".to_string()]
        } else {
            titles
        }
    }

    // ---- Status bar snapshot ----

    pub fn menu_bar_text(&self) -> String {
        format!("TTL {}", format_money(self.overview.today_cost))
    }

    pub fn menu_bar_idle_detail(&self) -> String {
        format!("{} req", compact_number(self.overview.today_requests))
    }

    pub fn status_bar_snapshot(&self) -> StatusBarSnapshot {
        let running_items: Vec<StatusRunningItem> = self
            .menu_bar_running_logs
            .iter()
            .map(|log| {
                let model = if log.model.trim().is_empty() {
                    log.original_model.clone()
                } else {
                    log.model.clone()
                };
                StatusRunningItem {
                    id: if log.session_id.is_empty() {
                        format!("log-{}", log.id)
                    } else {
                        log.session_id.clone()
                    },
                    log_id: Some(log.id),
                    provider_name: log.provider_name.clone(),
                    model,
                    multiplier: self.provider_multiplier(&log.provider_name),
                    is_retrying: false,
                    started_at_ms: parse_cch_date(&log.created_at).map(|d| d.timestamp_millis()),
                    cache_state: self.cache_status(log).state,
                }
            })
            .collect();

        StatusBarSnapshot {
            shows_details: self.show_status_bar_details,
            idle_primary: self.menu_bar_text(),
            idle_detail: self.menu_bar_idle_detail(),
            idle_cache_state: self.status_bar_cache_state(),
            running_items,
            has_recent_logs: !self.recent_logs.is_empty(),
        }
    }

    fn status_bar_cache_state(&self) -> CacheVisibilityState {
        if self.has_cache_alert() {
            CacheVisibilityState::Rebuilding
        } else {
            CacheVisibilityState::Normal
        }
    }

    /// Build the current tray payload (idle vs the highest-priority running item),
    /// matching `applyStatusSnapshot` in the Swift controller.
    pub fn status_bar_payload(&self) -> StatusBarPayload {
        let snapshot = self.status_bar_snapshot();
        if let Some(item) = snapshot.running_items.first() {
            let provider = compact_provider_name(&item.provider_name);
            let billing = item.model.trim();
            let billing_text = if billing.is_empty() { "model" } else { billing };
            let elapsed = match item.started_at_ms {
                Some(ms) => {
                    let now = chrono::Utc::now().timestamp_millis();
                    format_duration((now - ms).max(0) as f64 / 1000.0)
                }
                None => "--".to_string(),
            };
            StatusBarPayload::Running {
                provider,
                detail: format!(
                    "{}{} {}",
                    if item.is_retrying { "retrying " } else { "" },
                    billing_text,
                    format_multiplier(item.multiplier)
                ),
                elapsed,
                is_retrying: item.is_retrying,
                session_count: snapshot.running_items.len(),
                cache_state: item.cache_state,
            }
        } else {
            StatusBarPayload::Idle {
                primary: snapshot.idle_primary,
                detail: snapshot.idle_detail,
                cache_state: snapshot.idle_cache_state,
            }
        }
    }

    /// Serialize the full view-model for the webview.
    pub fn view_model(&self) -> ViewModel {
        let cache_status: HashMap<String, CacheStatusContext> = self
            .recent_logs
            .iter()
            .chain(self.logs.iter())
            .map(|log| (log.id.to_string(), self.cache_status(log)))
            .collect();
        ViewModel {
            overview: self.overview.clone(),
            active_sessions: self.active_sessions.clone(),
            leaderboard: self.leaderboard.clone(),
            leaderboard_summary: self.leaderboard_summary(),
            logs: self.logs.clone(),
            recent_logs: self.recent_logs.clone(),
            menu_bar_running_logs: self.menu_bar_running_logs.clone(),
            log_summary: self.log_summary.clone(),
            log_total: self.log_total,
            providers: self.providers.clone(),
            provider_groups: self.provider_groups(),
            cache_status,
            status_bar: self.status_bar_snapshot(),
            error_message: self.error_message.clone(),
            has_cache_alert: self.has_cache_alert(),
        }
    }
}

fn unique_logs(values: Vec<LogEntry>) -> Vec<LogEntry> {
    let mut seen: HashSet<i64> = HashSet::new();
    values.into_iter().filter(|log| seen.insert(log.id)).collect()
}

fn cache_session_key(log: &LogEntry) -> String {
    if log.session_id.is_empty() {
        format!("log-{}", log.id)
    } else {
        log.session_id.clone()
    }
}

fn is_compact_cache_request(log: &LogEntry) -> bool {
    let model_text = format!("{} {}", log.model, log.original_model).to_lowercase();
    if model_text.contains("compact") {
        return true;
    }
    log.messages_count > 0 && log.messages_count < 12
}

/// Whether `log` shows a large cache drop relative to `previous`, matching
/// `isLargeCacheDrop`.
fn is_large_cache_drop(log: &LogEntry, previous: Option<&LogEntry>) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    if log.input_tokens < 20_000
        || log.cache_read_tokens > (2_500).max(log.input_tokens / 18)
        || previous.cache_read_tokens < 15_000
        || is_compact_cache_request(log)
    {
        return false;
    }
    let previous_cached_context = previous.input_tokens + previous.cache_read_tokens;
    if previous_cached_context < 20_000 {
        return false;
    }
    let ratio = log.input_tokens as f64 / previous_cached_context as f64;
    (0.55..=1.55).contains(&ratio)
}

fn build_cache_status_map(logs: &[LogEntry]) -> HashMap<i64, CacheStatusContext> {
    let mut result: HashMap<i64, CacheStatusContext> = HashMap::new();

    // Group by session key.
    let mut groups: HashMap<String, Vec<LogEntry>> = HashMap::new();
    for log in logs {
        groups.entry(cache_session_key(log)).or_default().push(log.clone());
    }

    for group in groups.values() {
        let mut ordered: Vec<&LogEntry> = group
            .iter()
            .filter(|log| log.status_code.map(|c| (200..300).contains(&c)).unwrap_or(false))
            .collect();
        ordered.sort_by(|lhs, rhs| {
            if lhs.session_id == rhs.session_id && lhs.request_sequence != rhs.request_sequence {
                return lhs.request_sequence.cmp(&rhs.request_sequence);
            }
            let lhs_date = parse_cch_date(&lhs.created_at);
            let rhs_date = parse_cch_date(&rhs.created_at);
            match (lhs_date, rhs_date) {
                (Some(a), Some(b)) => a.cmp(&b),
                _ => lhs.id.cmp(&rhs.id),
            }
        });

        let mut previous: Option<&LogEntry> = None;
        for log in ordered {
            let state = if is_large_cache_drop(log, previous) {
                CacheVisibilityState::Rebuilding
            } else {
                CacheVisibilityState::Normal
            };
            result.insert(
                log.id,
                CacheStatusContext {
                    state,
                    created_tokens: log.cache_creation_tokens,
                    read_tokens: log.cache_read_tokens,
                },
            );
            if log.input_tokens > 0 || log.cache_read_tokens > 0 || log.total_tokens > 0 {
                previous = Some(log);
            }
        }
    }

    for log in logs {
        result.entry(log.id).or_insert(CacheStatusContext {
            state: CacheVisibilityState::Normal,
            created_tokens: log.cache_creation_tokens,
            read_tokens: log.cache_read_tokens,
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log(id: i64, session: &str, seq: i64, input: i64, cache_read: i64, status: Option<i64>) -> LogEntry {
        LogEntry {
            id,
            session_id: session.to_string(),
            request_sequence: seq,
            input_tokens: input,
            cache_read_tokens: cache_read,
            status_code: status,
            created_at: format!("2024-01-01T10:00:{:02}Z", id.min(59)),
            messages_count: 30,
            ..Default::default()
        }
    }

    #[test]
    fn cache_drop_detected_across_sequence() {
        // previous request had big cache read; next request has big input but
        // tiny cache read -> rebuilding.
        let logs = vec![
            log(1, "s1", 1, 30_000, 25_000, Some(200)),
            log(2, "s1", 2, 40_000, 1_000, Some(200)),
        ];
        let map = build_cache_status_map(&logs);
        assert_eq!(map[&1].state, CacheVisibilityState::Normal);
        assert_eq!(map[&2].state, CacheVisibilityState::Rebuilding);
    }

    #[test]
    fn no_cache_drop_when_cache_read_healthy() {
        let logs = vec![
            log(1, "s1", 1, 30_000, 25_000, Some(200)),
            log(2, "s1", 2, 28_000, 24_000, Some(200)),
        ];
        let map = build_cache_status_map(&logs);
        assert_eq!(map[&2].state, CacheVisibilityState::Normal);
    }

    #[test]
    fn running_logs_dedup_by_session_and_exclude_finished() {
        let mut state = MonitorState::new(CchConfig::default());
        state.set_recent_logs(vec![
            log(3, "s1", 2, 0, 0, None), // running, newest in s1
            log(2, "s1", 1, 0, 0, None), // running, older in s1 (deduped)
            log(1, "s2", 1, 0, 0, Some(200)), // finished (excluded)
        ]);
        let running = state.menu_bar_running_logs();
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].id, 3);
    }

    #[test]
    fn idle_payload_when_no_running() {
        let mut state = MonitorState::new(CchConfig::default());
        state.set_overview(CchOverview {
            today_cost: 12.5,
            today_requests: 1500,
            ..Default::default()
        });
        match state.status_bar_payload() {
            StatusBarPayload::Idle { primary, detail, .. } => {
                assert_eq!(primary, "TTL $12.5");
                assert_eq!(detail, "1.5k req");
            }
            _ => panic!("expected idle"),
        }
    }

    #[test]
    fn running_payload_uses_provider_and_multiplier() {
        let mut state = MonitorState::new(CchConfig::default());
        state.set_providers(vec![Provider {
            id: 1,
            name: "Acme".into(),
            cost_multiplier: 2.0,
            ..Default::default()
        }]);
        let mut running = log(5, "s1", 1, 0, 0, None);
        running.provider_name = "Acme".into();
        running.model = "claude-3".into();
        state.set_recent_logs(vec![running]);
        match state.status_bar_payload() {
            StatusBarPayload::Running { provider, detail, .. } => {
                assert_eq!(provider, "Acme");
                assert_eq!(detail, "claude-3 x2");
            }
            _ => panic!("expected running"),
        }
    }

    #[test]
    fn leaderboard_summary_weights_cache_hit_rate() {
        let mut state = MonitorState::new(CchConfig::default());
        state.set_leaderboard(vec![
            LeaderboardEntry {
                requests: 10,
                cost: 1.0,
                tokens: 100,
                input_tokens: 100,
                cache_hit_rate_override: Some(0.8),
                ..Default::default()
            },
            LeaderboardEntry {
                requests: 20,
                cost: 2.0,
                tokens: 200,
                input_tokens: 300,
                cache_hit_rate_override: Some(0.4),
                ..Default::default()
            },
        ]);
        let summary = state.leaderboard_summary();
        assert_eq!(summary.requests, 30);
        assert_eq!(summary.cost, 3.0);
        // weighted: (0.8*100 + 0.4*300)/400 = (80+120)/400 = 0.5
        assert!((summary.cache_hit_rate.unwrap() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn provider_groups_include_all_first() {
        let mut state = MonitorState::new(CchConfig::default());
        state.set_providers(vec![
            Provider { id: 1, group_tag: "prod".into(), ..Default::default() },
            Provider { id: 2, group_tag: "test".into(), ..Default::default() },
        ]);
        let groups = state.provider_groups();
        assert_eq!(groups[0], "全部");
        assert!(groups.contains(&"prod".to_string()));
        assert!(groups.contains(&"test".to_string()));
    }
}
