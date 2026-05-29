//! Tauri command handlers — the bridge the webview calls via `invoke(...)`.

use std::sync::Arc;
use tauri::State;

use crate::app_state::AppState;
use crate::settings::Settings;
use cch_core::state::ViewModel;
use cch_core::{GitHubRelease, LogEntry, ProbeResult};

type Cmd<T> = Result<T, String>;

/// Full serialized view-model for the current monitor state.
#[tauri::command]
pub async fn get_view_model(state: State<'_, Arc<AppState>>) -> Cmd<ViewModel> {
    Ok(state.monitor.lock().await.view_model())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, Arc<AppState>>) -> Cmd<Settings> {
    Ok(state.settings.lock().await.clone())
}

/// Persist settings and apply the connection/display fields to the monitor.
#[tauri::command]
pub async fn save_settings(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    settings: Settings,
) -> Cmd<()> {
    {
        let mut current = state.settings.lock().await;
        *current = settings.clone();
        current.save().map_err(|e| e.to_string())?;
    }
    {
        let mut monitor = state.monitor.lock().await;
        monitor.config = settings.config();
        monitor.show_status_bar_details = settings.show_status_bar_details;
        monitor.leaderboard_period = settings.leaderboard_period.clone();
        monitor.leaderboard_scope = settings.leaderboard_scope.clone();
    }
    state
        .config_generation
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    // Trigger an immediate refresh and push the result to the UI/tray.
    crate::refresh_and_emit(app, state.inner().clone()).await;
    Ok(())
}

/// Force a refresh now.
#[tauri::command]
pub async fn refresh_now(app: tauri::AppHandle, state: State<'_, Arc<AppState>>) -> Cmd<ViewModel> {
    crate::refresh_and_emit(app, state.inner().clone()).await;
    Ok(state.monitor.lock().await.view_model())
}

/// Change leaderboard period/scope and refresh just the leaderboard.
#[tauri::command]
pub async fn set_leaderboard(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    period: String,
    scope: String,
) -> Cmd<ViewModel> {
    {
        let mut monitor = state.monitor.lock().await;
        monitor.leaderboard_period = period.clone();
        monitor.leaderboard_scope = scope.clone();
    }
    {
        let mut settings = state.settings.lock().await;
        settings.leaderboard_period = period.clone();
        settings.leaderboard_scope = scope.clone();
        let _ = settings.save();
    }
    let config = state.current_config().await;
    match state.api.fetch_leaderboard(&config, &period, &scope).await {
        Ok(entries) => {
            let mut monitor = state.monitor.lock().await;
            monitor.set_leaderboard(entries);
            monitor.error_message = None;
        }
        Err(e) => {
            state.monitor.lock().await.error_message = Some(e.to_string());
        }
    }
    crate::push_tray(&app, state.inner().clone()).await;
    Ok(state.monitor.lock().await.view_model())
}

/// Load a logs page on demand (filters + pagination from the Logs tab).
#[tauri::command]
pub async fn fetch_logs(
    state: State<'_, Arc<AppState>>,
    page: i64,
    page_size: i64,
    model: String,
    status_code: String,
    session_id: String,
    include_stats: bool,
) -> Cmd<cch_core::LogsPage> {
    state
        .fetch_logs_page(page, page_size, &model, &status_code, &session_id, include_stats)
        .await
}

#[tauri::command]
pub async fn set_provider_enabled(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    provider_id: i64,
    enabled: bool,
) -> Cmd<()> {
    let config = state.current_config().await;
    state
        .api
        .set_provider_enabled(&config, provider_id, enabled)
        .await
        .map_err(|e| e.to_string())?;
    crate::refresh_and_emit(app, state.inner().clone()).await;
    Ok(())
}

#[tauri::command]
pub async fn reset_provider_circuit(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    provider_id: i64,
) -> Cmd<()> {
    let config = state.current_config().await;
    state
        .api
        .reset_provider_circuit(&config, provider_id)
        .await
        .map_err(|e| e.to_string())?;
    crate::refresh_and_emit(app, state.inner().clone()).await;
    Ok(())
}

#[tauri::command]
pub async fn set_provider_group(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    provider_id: i64,
    group_tag: Option<String>,
) -> Cmd<()> {
    let config = state.current_config().await;
    state
        .api
        .set_provider_groups(&config, provider_id, group_tag.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    crate::refresh_and_emit(app, state.inner().clone()).await;
    Ok(())
}

#[tauri::command]
pub async fn check_for_updates(state: State<'_, Arc<AppState>>) -> Cmd<Option<GitHubRelease>> {
    state.check_for_updates().await
}

/// Lightweight connectivity probe used by the Settings window "test" button.
#[tauri::command]
pub async fn probe_connection(state: State<'_, Arc<AppState>>) -> Cmd<ProbeResult> {
    let config = state.current_config().await;
    let started = std::time::Instant::now();
    match state.api.fetch_overview(&config).await {
        Ok(_) => Ok(ProbeResult {
            ok: true,
            method: "overview".to_string(),
            status_code: Some(200),
            latency_ms: Some(started.elapsed().as_secs_f64() * 1000.0),
            error_message: String::new(),
        }),
        Err(e) => Ok(ProbeResult {
            ok: false,
            method: "overview".to_string(),
            status_code: None,
            latency_ms: Some(started.elapsed().as_secs_f64() * 1000.0),
            error_message: e.to_string(),
        }),
    }
}

/// Open the Settings window (creates it if missing).
#[tauri::command]
pub async fn open_settings_window(app: tauri::AppHandle) -> Cmd<()> {
    crate::windows::show_settings_window(&app).map_err(|e| e.to_string())
}

/// Close the popover panel window.
#[tauri::command]
pub async fn close_panel(app: tauri::AppHandle) -> Cmd<()> {
    if let Some(window) = tauri::Manager::get_webview_window(&app, "panel") {
        let _ = window.hide();
    }
    Ok(())
}

/// Return the menu-bar running logs only (used for quick polling if desired).
#[tauri::command]
pub async fn get_running_logs(state: State<'_, Arc<AppState>>) -> Cmd<Vec<LogEntry>> {
    Ok(state.monitor.lock().await.menu_bar_running_logs().to_vec())
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}
