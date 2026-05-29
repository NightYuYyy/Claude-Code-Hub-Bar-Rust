//! Shared application state and the snapshot/refresh orchestration that the
//! Tauri commands and the background loop both drive.

use cch_core::{ApiService, CchConfig, GitHubRelease, LogsPage, MonitorState};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::settings::Settings;

pub const RELEASE_OWNER: &str = "NightYuYyy";
pub const RELEASE_REPO: &str = "Claude-Code-Hub-Bar-Rust";

pub struct AppState {
    pub api: ApiService,
    pub monitor: Mutex<MonitorState>,
    pub settings: Mutex<Settings>,
    /// Generation counter so an in-flight refresh from an old config is ignored.
    pub config_generation: std::sync::atomic::AtomicU64,
}

impl AppState {
    pub fn new(settings: Settings) -> Arc<Self> {
        let mut monitor = MonitorState::new(settings.config());
        monitor.show_status_bar_details = settings.show_status_bar_details;
        monitor.leaderboard_period = settings.leaderboard_period.clone();
        monitor.leaderboard_scope = settings.leaderboard_scope.clone();
        Arc::new(Self {
            api: ApiService::new(),
            monitor: Mutex::new(monitor),
            settings: Mutex::new(settings),
            config_generation: std::sync::atomic::AtomicU64::new(0),
        })
    }

    pub async fn current_config(&self) -> CchConfig {
        self.settings.lock().await.config()
    }

    /// Perform one full refresh against the CCH backend, updating the monitor
    /// state. Returns `Ok(())` even on partial failures (errors are recorded on
    /// the monitor's `error_message`) so the loop keeps running.
    pub async fn refresh(&self) -> Result<(), String> {
        let config = self.current_config().await;
        let (period, scope) = {
            let monitor = self.monitor.lock().await;
            (monitor.leaderboard_period.clone(), monitor.leaderboard_scope.clone())
        };

        let overview = self.api.fetch_overview(&config).await;
        let sessions = self.api.fetch_active_sessions(&config).await;
        let recent = self
            .api
            .fetch_logs(&config, 1, 50, None, "", "", "", false)
            .await;
        let leaderboard = self.api.fetch_leaderboard(&config, &period, &scope).await;
        let providers = self.api.fetch_providers(&config, true).await;

        let mut monitor = self.monitor.lock().await;
        let mut error: Option<String> = None;
        match overview {
            Ok(v) => monitor.set_overview(v),
            Err(e) => error = Some(e.to_string()),
        }
        if let Ok(v) = sessions {
            monitor.set_active_sessions(v);
        }
        match recent {
            Ok(page) => monitor.set_recent_logs(page.logs),
            Err(e) => {
                error.get_or_insert(e.to_string());
            }
        }
        if let Ok(v) = leaderboard {
            monitor.set_leaderboard(v);
        }
        if let Ok(v) = providers {
            monitor.set_providers(v);
        }
        monitor.error_message = error;
        Ok(())
    }

    /// Fetch a logs page on demand (Logs tab "apply"/"load more").
    pub async fn fetch_logs_page(
        &self,
        page: i64,
        page_size: i64,
        model: &str,
        status_code: &str,
        session_id: &str,
        include_stats: bool,
    ) -> Result<LogsPage, String> {
        let config = self.current_config().await;
        let start = cch_core::format::leaderboard_start_unix_ms("daily");
        self.api
            .fetch_logs(&config, page, page_size, start, model, status_code, session_id, include_stats)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn check_for_updates(&self) -> Result<Option<GitHubRelease>, String> {
        let release = self
            .api
            .fetch_latest_release(RELEASE_OWNER, RELEASE_REPO)
            .await
            .map_err(|e| e.to_string())?;
        let current = env!("CARGO_PKG_VERSION");
        let remote = cch_core::format::normalize_release_version(&release.tag);
        if cch_core::format::compare_semver(&remote, current) == std::cmp::Ordering::Greater {
            Ok(Some(release))
        } else {
            Ok(None)
        }
    }
}
