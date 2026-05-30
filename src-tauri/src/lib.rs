//! Claude Code Hub Bar — Tauri shell entrypoint.
//!
//! Owns the app lifecycle: loads settings, builds the tray, starts the
//! background refresh loop, registers commands, and wires the panel/settings
//! windows. All business logic lives in `cch-core`.

mod app_state;
mod commands;
mod settings;
mod tray;
mod windows;

use std::sync::Arc;
use std::time::Duration;

use app_state::AppState;
use settings::Settings;
use tauri::{Emitter, Manager};

/// Run one refresh, then push the result to the tray and the panel webview.
pub(crate) async fn refresh_and_emit(app: tauri::AppHandle, state: Arc<AppState>) {
    let _ = state.refresh().await;
    push_tray(&app, state.clone()).await;
    emit_view_model(&app, state).await;
}

/// Recompute and apply the tray payload from the current monitor state.
pub(crate) async fn push_tray(app: &tauri::AppHandle, state: Arc<AppState>) {
    let payload = state.monitor.lock().await.status_bar_payload();
    tray::apply_payload(app, &payload);
}

/// Emit the latest view-model to any open windows.
pub(crate) async fn emit_view_model(app: &tauri::AppHandle, state: Arc<AppState>) {
    let model = state.monitor.lock().await.view_model();
    let _ = app.emit("view-model", model);
}

fn spawn_refresh_loop(app: tauri::AppHandle, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        loop {
            refresh_and_emit(app.clone(), state.clone()).await;
            let interval = {
                let settings = state.settings.lock().await;
                settings.refresh_interval.max(5.0)
            };
            tokio::time::sleep(Duration::from_secs_f64(interval)).await;
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings = Settings::load();
    let state = AppState::new(settings);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_view_model,
            commands::get_panel_mode,
            commands::get_settings,
            commands::save_settings,
            commands::refresh_now,
            commands::set_leaderboard,
            commands::fetch_logs,
            commands::set_provider_enabled,
            commands::reset_provider_circuit,
            commands::set_provider_group,
            commands::check_for_updates,
            commands::probe_connection,
            commands::open_settings_window,
            commands::close_panel,
            commands::get_running_logs,
            commands::quit_app,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let app_state = handle.state::<Arc<AppState>>().inner().clone();

            tray::build_tray(&handle, app_state.clone())?;

            // Create the panel window with platform-appropriate behavior:
            // a hidden menu-bar popover on macOS, a visible main window on
            // Windows/Linux.
            windows::ensure_panel(&handle)?;

            // On macOS, hide the dock icon — this is a menu-bar app.
            #[cfg(target_os = "macos")]
            {
                let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            spawn_refresh_loop(handle, app_state);
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "panel" {
                return;
            }
            match event {
                // Closing the panel never quits the app — it tucks into the
                // tray. Quit is only available from the tray menu.
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                // On macOS the popover hides as soon as it loses focus. On
                // Windows/Linux the panel is a normal window that stays open.
                tauri::WindowEvent::Focused(false) if windows::PANEL_IS_POPOVER => {
                    let _ = window.hide();
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("error while running CCHBar")
        .run(|_app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                // Allow normal exit.
            }
        });
}
