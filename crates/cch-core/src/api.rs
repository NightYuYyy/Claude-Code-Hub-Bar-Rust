//! HTTP API client, ported from the `APIService` actor in `APIService.swift`.
//!
//! Uses `reqwest` with two clients: a default one and a "direct" one that
//! bypasses any system proxy for local/private hosts (matching the Swift
//! `directSession`). Endpoints prefer the `/api/v1/...` REST surface and fall
//! back to the legacy `/api/actions/...` RPC surface on 404/405/410/parse
//! errors, exactly like the original.

use crate::format::{leaderboard_start_unix_ms, parse_cch_date};
use crate::jsonx::*;
use crate::models::*;
use crate::parse::*;
use chrono::Utc;
use reqwest::Client;
use serde_json::{json, Map, Value};
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("无效的 CCH 地址")]
    InvalidUrl,
    #[error("CCH 响应无效")]
    InvalidResponse,
    #[error("缺少 CCH API Key")]
    MissingToken,
    #[error("HTTP 错误 {0}")]
    HttpError(u16),
    #[error("数据解析失败")]
    ParseError,
    #[error("{0}")]
    ActionError(String),
    #[error("{0}")]
    Network(String),
}

impl ApiError {
    fn should_fallback_to_actions(&self) -> bool {
        match self {
            ApiError::HttpError(code) => *code == 404 || *code == 405 || *code == 410,
            ApiError::ParseError | ApiError::InvalidResponse => true,
            _ => false,
        }
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

struct CachedToken {
    path: String,
    modified_at: Option<std::time::SystemTime>,
    token: String,
}

pub struct ApiService {
    client: Client,
    direct_client: Client,
    cached_token: Mutex<Option<CachedToken>>,
}

impl Default for ApiService {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiService {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(15))
            .user_agent("CCHBar-Rust")
            .build()
            .expect("default reqwest client");
        let direct_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(15))
            .user_agent("CCHBar-Rust")
            .no_proxy()
            .build()
            .expect("direct reqwest client");
        Self {
            client,
            direct_client,
            cached_token: Mutex::new(None),
        }
    }

    // ---- Public endpoints ----

    pub async fn fetch_overview(&self, config: &CchConfig) -> ApiResult<CchOverview> {
        let data = match self.get_v1(config, "/api/v1/dashboard/overview", &[]).await {
            Ok(v) => v,
            Err(e) if e.should_fallback_to_actions() => {
                self.post_action(config, "overview", "getOverviewData", &Map::new())
                    .await?
            }
            Err(e) => return Err(e),
        };
        let dict = data.as_object().ok_or(ApiError::ParseError)?;
        Ok(parse_overview(dict))
    }

    pub async fn fetch_active_sessions(&self, config: &CchConfig) -> ApiResult<Vec<CchActiveSession>> {
        let data = match self
            .get_v1(
                config,
                "/api/v1/sessions",
                &[("state", "active"), ("pageSize", "100")],
            )
            .await
        {
            Ok(v) => v,
            Err(e) if e.should_fallback_to_actions() => {
                self.post_action(config, "active-sessions", "getActiveSessions", &Map::new())
                    .await?
            }
            Err(e) => return Err(e),
        };
        Ok(item_rows(&data).iter().map(parse_active_session).collect())
    }

    pub async fn fetch_leaderboard(
        &self,
        config: &CchConfig,
        period: &str,
        scope: &str,
    ) -> ApiResult<Vec<LeaderboardEntry>> {
        match self.fetch_official_leaderboard(config, period, scope).await {
            Ok(v) => Ok(v),
            Err(e) if e.should_fallback_to_actions() => {
                let rows = self.fetch_usage_log_rows_for_leaderboard(config, period).await?;
                Ok(aggregate_leaderboard(&rows, scope))
            }
            Err(e) => Err(e),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn fetch_logs(
        &self,
        config: &CchConfig,
        page: i64,
        page_size: i64,
        start_unix_ms: Option<i64>,
        model: &str,
        status_code: &str,
        session_id: &str,
        include_stats: bool,
    ) -> ApiResult<LogsPage> {
        let query = usage_log_query_items(page, page_size, start_unix_ms, model, status_code, session_id);
        let query_refs: Vec<(&str, &str)> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let data = match self.get_v1(config, "/api/v1/usage-logs", &query_refs).await {
            Ok(v) => v,
            Err(e) if e.should_fallback_to_actions() => {
                return self
                    .fetch_legacy_logs(config, page, page_size, start_unix_ms, model, status_code, session_id)
                    .await;
            }
            Err(e) => return Err(e),
        };
        let dict = data.as_object().ok_or(ApiError::ParseError)?;
        let rows = item_rows(&data);
        let stats: Map<String, Value> = if include_stats {
            self.get_v1(config, "/api/v1/usage-logs/stats", &query_refs)
                .await
                .ok()
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default()
        } else {
            Map::new()
        };
        let page_info = dict.get("pageInfo").and_then(Value::as_object).cloned().unwrap_or_default();
        let total = int_value(
            page_info.get("total"),
            int_value(dict.get("total"), rows.len() as i64),
        );
        Ok(LogsPage {
            logs: rows.iter().map(parse_log).collect(),
            total,
            summary: LogSummary {
                total_requests: int_value(stats.get("totalRequests"), 0),
                total_cost: double_value(stats.get("totalCost"), 0.0),
                total_tokens: int_value(stats.get("totalTokens"), 0),
                input_tokens: int_value(stats.get("totalInputTokens"), 0),
                output_tokens: int_value(stats.get("totalOutputTokens"), 0),
                cache_creation_tokens: cache_creation_tokens(&stats, "total"),
                cache_read_tokens: int_value(stats.get("totalCacheReadTokens"), 0),
            },
        })
    }

    pub async fn fetch_providers(
        &self,
        config: &CchConfig,
        include_usage: bool,
    ) -> ApiResult<Vec<Provider>> {
        let providers_data;
        let health_data: Option<Value>;
        let usage_rows: Vec<LeaderboardEntry>;
        match self.get_v1(config, "/api/v1/providers", &[]).await {
            Ok(v) => {
                providers_data = v;
                health_data = self.get_v1(config, "/api/v1/providers/health", &[]).await.ok();
                usage_rows = if include_usage {
                    self.fetch_official_leaderboard_rows(config, "daily", "provider", false)
                        .await
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
            }
            Err(e) if e.should_fallback_to_actions() => {
                providers_data = self
                    .post_action(config, "providers", "getProviders", &Map::new())
                    .await?;
                health_data = self
                    .post_action(config, "providers", "getProvidersHealthStatus", &Map::new())
                    .await
                    .ok();
                usage_rows = if include_usage {
                    let usage_log_rows = self
                        .fetch_usage_log_rows_for_leaderboard(config, "daily")
                        .await
                        .unwrap_or_default();
                    aggregate_leaderboard(&usage_log_rows, "provider")
                } else {
                    Vec::new()
                };
            }
            Err(e) => return Err(e),
        }

        let rows = item_rows(&providers_data);
        let health_map = health_data
            .as_ref()
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        let mut usage_by_id: std::collections::HashMap<String, LeaderboardEntry> = std::collections::HashMap::new();
        let mut usage_by_name: std::collections::HashMap<String, LeaderboardEntry> = std::collections::HashMap::new();
        for entry in &usage_rows {
            usage_by_id.entry(entry.id.clone()).or_insert_with(|| entry.clone());
            usage_by_name
                .entry(entry.title.to_lowercase())
                .and_modify(|existing| *existing = merge_leaderboard_entries(existing, entry))
                .or_insert_with(|| entry.clone());
        }

        Ok(rows
            .iter()
            .map(|row| {
                let id = int_value(row.get("id"), 0);
                let name = string_value(row.get("name"), "Provider");
                let usage = usage_by_id
                    .get(&format!("provider-{id}"))
                    .or_else(|| usage_by_name.get(&name.to_lowercase()));
                let health_dict = health_map
                    .get(&id.to_string())
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                let health = ProviderHealth {
                    circuit_state: string_value(health_dict.get("circuitState"), "closed"),
                    failure_count: int_value(health_dict.get("failureCount"), 0),
                    last_failure_time: optional_int(health_dict.get("lastFailureTime")),
                    circuit_open_until: optional_int(health_dict.get("circuitOpenUntil")),
                    recovery_minutes: optional_int(health_dict.get("recoveryMinutes")),
                };
                Provider {
                    id,
                    name: name.clone(),
                    provider_type: string_value(row.get("providerType"), ""),
                    vendor_id: first_optional_int(row, &["providerVendorId", "vendorId"]),
                    api_url: first_string_value(row, &["url", "endpointUrl", "apiUrl", "apiURL"], ""),
                    website_url: first_string_value(
                        row,
                        &["websiteUrl", "websiteURL", "homepageUrl", "homepageURL"],
                        "",
                    ),
                    is_enabled: bool_value(row.get("isEnabled"), false),
                    priority: int_value(row.get("priority"), 0),
                    weight: int_value(row.get("weight"), 0),
                    group_tag: first_string_value(
                        row,
                        &["groupTag", "group_tag", "providerGroup"],
                        "default",
                    ),
                    cost_multiplier: double_value(row.get("costMultiplier"), 1.0),
                    today_calls: usage.map(|u| u.requests).unwrap_or_else(|| int_value(row.get("todayCallCount"), 0)),
                    today_cost: usage.map(|u| u.cost).unwrap_or_else(|| double_value(row.get("todayTotalCostUsd"), 0.0)),
                    last_call_time: string_value(row.get("lastCallTime"), ""),
                    last_call_model: string_value(row.get("lastCallModel"), ""),
                    allowed_models: compact_array_description(row.get("allowedModels")),
                    allowed_clients: compact_array_description(row.get("allowedClients")),
                    model_redirects: compact_array_description(row.get("modelRedirects")),
                    limit_text: build_limit_text(row),
                    health,
                }
            })
            .collect())
    }

    pub async fn fetch_provider_groups(&self, config: &CchConfig) -> ApiResult<Vec<ProviderGroup>> {
        let data = self.get_v1(config, "/api/v1/provider-groups", &[]).await?;
        if let Some(values) = provider_group_string_rows(&data) {
            return Ok(values.iter().filter_map(|v| parse_provider_group_string(v)).collect());
        }
        Ok(item_rows(&data)
            .iter()
            .filter_map(parse_provider_group_row)
            .collect())
    }

    pub async fn set_provider_groups(
        &self,
        config: &CchConfig,
        provider_id: i64,
        group_tag: Option<&str>,
    ) -> ApiResult<()> {
        let mut body = Map::new();
        body.insert(
            "group_tag".to_string(),
            match group_tag {
                Some(t) => Value::String(t.to_string()),
                None => Value::Null,
            },
        );
        self.patch_v1(config, &format!("/api/v1/providers/{provider_id}"), &body)
            .await?;
        Ok(())
    }

    pub async fn set_provider_enabled(
        &self,
        config: &CchConfig,
        provider_id: i64,
        enabled: bool,
    ) -> ApiResult<()> {
        let mut body = Map::new();
        body.insert("is_enabled".to_string(), Value::Bool(enabled));
        match self
            .patch_v1(config, &format!("/api/v1/providers/{provider_id}"), &body)
            .await
        {
            Ok(_) => Ok(()),
            Err(e) if e.should_fallback_to_actions() => {
                let mut legacy = Map::new();
                legacy.insert("providerId".to_string(), json!(provider_id));
                legacy.insert("is_enabled".to_string(), Value::Bool(enabled));
                self.post_action(config, "providers", "editProvider", &legacy).await?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub async fn reset_provider_circuit(&self, config: &CchConfig, provider_id: i64) -> ApiResult<()> {
        match self
            .post_v1(
                config,
                &format!("/api/v1/providers/{provider_id}/circuit:reset"),
                &Map::new(),
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(e) if e.should_fallback_to_actions() => {
                let mut legacy = Map::new();
                legacy.insert("providerId".to_string(), json!(provider_id));
                self.post_action(config, "providers", "resetProviderCircuit", &legacy)
                    .await?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub async fn fetch_latest_release(&self, owner: &str, repo: &str) -> ApiResult<GitHubRelease> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
        let response = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(ApiError::HttpError(status.as_u16()));
        }
        let dict: Value = response.json().await.map_err(|_| ApiError::ParseError)?;
        let dict = dict.as_object().ok_or(ApiError::ParseError)?;
        let html_url = string_value(dict.get("html_url"), "");
        if html_url.is_empty() {
            return Err(ApiError::ParseError);
        }
        let published_at_ms = dict
            .get("published_at")
            .and_then(Value::as_str)
            .and_then(parse_cch_date)
            .map(|d| d.timestamp_millis());
        Ok(GitHubRelease {
            tag: string_value(dict.get("tag_name"), ""),
            name: string_value(dict.get("name"), ""),
            body: string_value(dict.get("body"), ""),
            html_url,
            published_at_ms,
        })
    }

    // ---- Leaderboard internals ----

    async fn fetch_official_leaderboard(
        &self,
        config: &CchConfig,
        period: &str,
        scope: &str,
    ) -> ApiResult<Vec<LeaderboardEntry>> {
        let primary = self
            .fetch_official_leaderboard_rows(config, period, scope, false)
            .await?;
        let cache = self
            .fetch_official_leaderboard_rows(config, period, scope, true)
            .await
            .unwrap_or_default();
        if cache.is_empty() {
            return Ok(primary);
        }
        let mut cache_by_title: std::collections::HashMap<String, LeaderboardEntry> = std::collections::HashMap::new();
        for entry in &cache {
            cache_by_title
                .entry(entry.title.to_lowercase())
                .and_modify(|existing| *existing = merge_leaderboard_entries(existing, entry))
                .or_insert_with(|| entry.clone());
        }
        Ok(primary
            .iter()
            .map(|entry| match cache_by_title.get(&entry.title.to_lowercase()) {
                Some(cache_entry) => merge_cache_data(entry, cache_entry),
                None => entry.clone(),
            })
            .collect())
    }

    async fn fetch_official_leaderboard_rows(
        &self,
        config: &CchConfig,
        period: &str,
        scope: &str,
        cache_hit_mode: bool,
    ) -> ApiResult<Vec<LeaderboardEntry>> {
        let api_scope = if cache_hit_mode {
            match scope {
                "user" => "userCacheHitRate",
                "provider" => "providerCacheHitRate",
                other => other,
            }
        } else {
            scope
        };
        let mut query: Vec<(&str, &str)> = vec![("period", period), ("scope", api_scope)];
        query.extend(leaderboard_extra_query_items(api_scope));
        let data = self.get_raw(config, "/api/leaderboard", &query).await?;
        let rows = item_rows(&data);
        Ok(parse_official_leaderboard_rows(&rows, api_scope))
    }

    async fn fetch_usage_log_rows_for_leaderboard(
        &self,
        config: &CchConfig,
        period: &str,
    ) -> ApiResult<Vec<Map<String, Value>>> {
        let page_size = 100;
        let start = leaderboard_start_unix_ms(period);
        let first_query = usage_log_query_items(1, page_size, start, "", "", "");
        let first_refs: Vec<(&str, &str)> = first_query.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let first_page = self.get_v1(config, "/api/v1/usage-logs", &first_refs).await?;
        let mut rows = item_rows(&first_page);
        let total_pages = usage_log_total_pages(&first_page);
        if total_pages <= 1 {
            return Ok(rows);
        }
        for page in 2..=total_pages {
            let query = usage_log_query_items(page, page_size, start, "", "", "");
            let refs: Vec<(&str, &str)> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();
            let data = self.get_v1(config, "/api/v1/usage-logs", &refs).await?;
            rows.extend(item_rows(&data));
        }
        Ok(rows)
    }

    #[allow(clippy::too_many_arguments)]
    async fn fetch_legacy_logs(
        &self,
        config: &CchConfig,
        page: i64,
        page_size: i64,
        start_unix_ms: Option<i64>,
        model: &str,
        status_code: &str,
        session_id: &str,
    ) -> ApiResult<LogsPage> {
        let mut body = Map::new();
        body.insert("page".to_string(), json!(page));
        body.insert("pageSize".to_string(), json!(page_size));
        if let Some(start) = start_unix_ms {
            let start_dt = chrono::DateTime::<Utc>::from_timestamp_millis(start).unwrap_or_else(Utc::now);
            body.insert("startDate".to_string(), json!(start_dt.to_rfc3339()));
            body.insert("endDate".to_string(), json!(Utc::now().to_rfc3339()));
        }
        let trimmed_model = model.trim();
        if !trimmed_model.is_empty() {
            body.insert("model".to_string(), json!(trimmed_model));
        }
        if let Ok(code) = status_code.trim().parse::<i64>() {
            body.insert("statusCode".to_string(), json!(code));
        }
        let trimmed_session = session_id.trim();
        if !trimmed_session.is_empty() {
            body.insert("sessionId".to_string(), json!(trimmed_session));
        }
        let data = self.post_action(config, "usage-logs", "getUsageLogs", &body).await?;
        let dict = data.as_object().ok_or(ApiError::ParseError)?;
        let rows = item_rows(&data);
        let summary = dict.get("summary").and_then(Value::as_object).cloned().unwrap_or_default();
        Ok(LogsPage {
            logs: rows.iter().map(parse_log).collect(),
            total: int_value(dict.get("total"), rows.len() as i64),
            summary: LogSummary {
                total_requests: int_value(summary.get("totalRequests"), 0),
                total_cost: double_value(summary.get("totalCost"), 0.0),
                total_tokens: int_value(summary.get("totalTokens"), 0),
                input_tokens: int_value(summary.get("totalInputTokens"), 0),
                output_tokens: int_value(summary.get("totalOutputTokens"), 0),
                cache_creation_tokens: cache_creation_tokens(&summary, "total"),
                cache_read_tokens: int_value(summary.get("totalCacheReadTokens"), 0),
            },
        })
    }

    // ---- Low-level request plumbing ----

    async fn get_v1(&self, config: &CchConfig, path: &str, query: &[(&str, &str)]) -> ApiResult<Value> {
        let url = self.v1_url(config, path, query)?;
        self.request_json(config, &url, reqwest::Method::GET, None).await
    }

    async fn get_raw(&self, config: &CchConfig, path: &str, query: &[(&str, &str)]) -> ApiResult<Value> {
        let url = self.v1_url(config, path, query)?;
        self.request_json(config, &url, reqwest::Method::GET, None).await
    }

    async fn post_v1(&self, config: &CchConfig, path: &str, body: &Map<String, Value>) -> ApiResult<Value> {
        let url = self.v1_url(config, path, &[])?;
        self.request_json(config, &url, reqwest::Method::POST, Some(body)).await
    }

    async fn patch_v1(&self, config: &CchConfig, path: &str, body: &Map<String, Value>) -> ApiResult<Value> {
        let url = self.v1_url(config, path, &[])?;
        self.request_json(config, &url, reqwest::Method::PATCH, Some(body)).await
    }

    async fn post_action(
        &self,
        config: &CchConfig,
        module: &str,
        action: &str,
        body: &Map<String, Value>,
    ) -> ApiResult<Value> {
        let base = self.normalized_base_url(config)?;
        let url = format!("{base}/api/actions/{module}/{action}");
        let value = self.request_json(config, &url, reqwest::Method::POST, Some(body)).await?;
        let dict = value.as_object().ok_or(ApiError::ParseError)?;
        if bool_value(dict.get("ok"), false) {
            Ok(dict.get("data").cloned().unwrap_or(Value::Null))
        } else {
            Err(ApiError::ActionError(string_value(dict.get("error"), "CCH 操作失败")))
        }
    }

    fn v1_url(&self, config: &CchConfig, path: &str, query: &[(&str, &str)]) -> ApiResult<String> {
        let base = self.normalized_base_url(config)?;
        let mut url = format!("{base}{path}");
        if !query.is_empty() {
            let qs: Vec<String> = query
                .iter()
                .map(|(k, v)| format!("{}={}", urlencode(k), urlencode(v)))
                .collect();
            url.push('?');
            url.push_str(&qs.join("&"));
        }
        Ok(url)
    }

    fn normalized_base_url(&self, config: &CchConfig) -> ApiResult<String> {
        let trimmed = config.base_url.trim().trim_matches('/').to_string();
        if trimmed.is_empty() || reqwest::Url::parse(&trimmed).is_err() {
            return Err(ApiError::InvalidUrl);
        }
        Ok(trimmed)
    }

    async fn request_json(
        &self,
        config: &CchConfig,
        url: &str,
        method: reqwest::Method,
        body: Option<&Map<String, Value>>,
    ) -> ApiResult<Value> {
        let token = self.resolve_token(config)?;
        let parsed = reqwest::Url::parse(url).map_err(|_| ApiError::InvalidUrl)?;
        let client = if should_bypass_proxy(&parsed) {
            &self.direct_client
        } else {
            &self.client
        };
        let mut req = client
            .request(method, url)
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {token}"))
            .header("X-Api-Key", &token)
            .header("Cookie", format!("auth-token={token}"))
            .timeout(Duration::from_secs(15));
        if let Some(body) = body {
            req = req.header("Content-Type", "application/json").json(body);
        }
        let response = req.send().await.map_err(|e| ApiError::Network(e.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(ApiError::HttpError(status.as_u16()));
        }
        response.json::<Value>().await.map_err(|_| ApiError::ParseError)
    }

    fn resolve_token(&self, config: &CchConfig) -> ApiResult<String> {
        let explicit = config.token.trim();
        if !explicit.is_empty() {
            return Ok(explicit.to_string());
        }
        let path = config.env_path.trim();
        if path.is_empty() {
            return Err(ApiError::MissingToken);
        }
        let expanded = expand_tilde(path);
        let modified_at = std::fs::metadata(&expanded).ok().and_then(|m| m.modified().ok());
        {
            let cache = self.cached_token.lock().unwrap();
            if let Some(cached) = cache.as_ref() {
                if cached.path == expanded && cached.modified_at == modified_at {
                    return Ok(cached.token.clone());
                }
            }
        }
        let content = std::fs::read_to_string(&expanded).map_err(|_| ApiError::MissingToken)?;
        let token_keys = [
            "CCH_API_KEY",
            "CCH_TOKEN",
            "API_KEY",
            "AUTH_TOKEN",
            "TOKEN",
            "KEY",
        ];
        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(eq) = line.find('=') {
                let key = line[..eq].trim();
                let value = line[eq + 1..].trim();
                if token_keys.contains(&key.to_uppercase().as_str()) && !value.is_empty() {
                    let token = value.trim_matches(|c| c == '"' || c == '\'').to_string();
                    *self.cached_token.lock().unwrap() = Some(CachedToken {
                        path: expanded,
                        modified_at,
                        token: token.clone(),
                    });
                    return Ok(token);
                }
            } else {
                let token = line.trim_matches(|c| c == '"' || c == '\'').to_string();
                *self.cached_token.lock().unwrap() = Some(CachedToken {
                    path: expanded,
                    modified_at,
                    token: token.clone(),
                });
                return Ok(token);
            }
        }
        Err(ApiError::MissingToken)
    }
}

fn usage_log_query_items(
    page: i64,
    page_size: i64,
    start_unix_ms: Option<i64>,
    model: &str,
    status_code: &str,
    session_id: &str,
) -> Vec<(&'static str, String)> {
    let mut items: Vec<(&'static str, String)> = vec![
        ("page", page.to_string()),
        ("pageSize", page_size.to_string()),
    ];
    if let Some(start) = start_unix_ms {
        items.push(("startTime", start.to_string()));
        items.push(("endTime", Utc::now().timestamp_millis().to_string()));
    }
    let trimmed_model = model.trim();
    if !trimmed_model.is_empty() {
        items.push(("model", trimmed_model.to_string()));
    }
    let trimmed_status = status_code.trim();
    if trimmed_status.parse::<i64>().is_ok() {
        items.push(("statusCode", trimmed_status.to_string()));
    }
    let trimmed_session = session_id.trim();
    if !trimmed_session.is_empty() {
        items.push(("sessionId", trimmed_session.to_string()));
    }
    items
}

fn should_bypass_proxy(url: &reqwest::Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_lowercase();
    if host == "localhost" || host.ends_with(".local") {
        return true;
    }
    if host.starts_with("10.") || host.starts_with("192.168.") {
        return true;
    }
    let parts: Vec<u8> = host.split('.').filter_map(|p| p.parse::<u8>().ok()).collect();
    if parts.len() == 4 {
        if parts[0] == 127 {
            return true;
        }
        if parts[0] == 172 && (16..=31).contains(&parts[1]) {
            return true;
        }
    }
    false
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return format!("{home}/{rest}");
        }
    } else if path == "~" {
        if let Some(home) = home_dir() {
            return home;
        }
    }
    path.to_string()
}

fn home_dir() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
}

fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_bypass_for_local_hosts() {
        assert!(should_bypass_proxy(&reqwest::Url::parse("http://localhost:8080").unwrap()));
        assert!(should_bypass_proxy(&reqwest::Url::parse("http://127.0.0.1/x").unwrap()));
        assert!(should_bypass_proxy(&reqwest::Url::parse("http://192.168.1.5/x").unwrap()));
        assert!(should_bypass_proxy(&reqwest::Url::parse("http://172.16.0.1/x").unwrap()));
        assert!(should_bypass_proxy(&reqwest::Url::parse("http://10.0.0.1/x").unwrap()));
        assert!(!should_bypass_proxy(&reqwest::Url::parse("https://cch.example.com/x").unwrap()));
        assert!(!should_bypass_proxy(&reqwest::Url::parse("http://172.32.0.1/x").unwrap()));
    }

    #[test]
    fn urlencode_escapes_reserved() {
        assert_eq!(urlencode("a b&c"), "a%20b%26c");
        assert_eq!(urlencode("safe-._~"), "safe-._~");
    }

    #[test]
    fn missing_token_when_no_token_or_path() {
        let svc = ApiService::new();
        let config = CchConfig::default();
        assert!(matches!(svc.resolve_token(&config), Err(ApiError::MissingToken)));
    }

    #[test]
    fn explicit_token_wins() {
        let svc = ApiService::new();
        let config = CchConfig {
            base_url: "https://x".into(),
            token: "  secret  ".into(),
            env_path: String::new(),
        };
        assert_eq!(svc.resolve_token(&config).unwrap(), "secret");
    }
}
