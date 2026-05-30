//! Cross-platform tray integration.
//!
//! Renders the tray title/tooltip from the monitor's `StatusBarPayload` and owns
//! the right-click menu. On macOS the tray supports a text title next to the
//! icon (like the original menu-bar item); on Windows/Linux we fold the same
//! information into the tooltip since those trays don't show inline text.

use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle,
};

use crate::app_state::AppState;
use cch_core::state::StatusBarPayload;

pub const TRAY_ID: &str = "cch-bar-tray";

pub fn build_tray(app: &AppHandle, state: Arc<AppState>) -> tauri::Result<()> {
    let open_item = MenuItem::with_id(app, "open", "显示窗口", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "设置…", true, None::<&str>)?;
    let refresh_item = MenuItem::with_id(app, "refresh", "立即刷新", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出 CCHBar", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&open_item, &settings_item, &refresh_item, &sep, &quit_item],
    )?;

    let icon = app.default_window_icon().cloned();
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Claude Code Hub Bar");
    if let Some(icon) = icon {
        builder = builder.icon(icon);
    }
    #[cfg(target_os = "macos")]
    {
        builder = builder.title("TTL $0.00");
    }

    let menu_state = state.clone();
    let tray = builder
        .on_menu_event(move |app, event| {
            let app = app.clone();
            let state = menu_state.clone();
            match event.id.as_ref() {
                "open" => {
                    let _ = crate::windows::show_panel_window(&app);
                }
                "settings" => {
                    let _ = crate::windows::show_settings_window(&app);
                }
                "refresh" => {
                    tauri::async_runtime::spawn(async move {
                        crate::refresh_and_emit(app, state).await;
                    });
                }
                "quit" => app.exit(0),
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                let _ = crate::windows::toggle_panel_window(app);
            }
        })
        .build(app)?;

    // The tray icon is kept alive by Tauri's internal tray registry (looked up
    // by `TRAY_ID`), so we simply drop our handle here.
    let _ = tray;
    Ok(())
}

/// Update the tray title (macOS) and tooltip (all platforms) from a payload.
pub fn apply_payload(app: &AppHandle, payload: &StatusBarPayload) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let (title, tooltip) = match payload {
        StatusBarPayload::Idle { primary, detail, cache_state } => {
            let badge = cache_badge(cache_state);
            (
                format!("{primary}{badge}"),
                format!("{primary} · {detail}{badge}"),
            )
        }
        StatusBarPayload::Running {
            provider,
            detail,
            elapsed,
            session_count,
            cache_state,
            ..
        } => {
            let badge = cache_badge(cache_state);
            let count = if *session_count > 1 {
                format!(" +{}", session_count - 1)
            } else {
                String::new()
            };
            (
                format!("● {provider} {elapsed}{count}{badge}"),
                format!("{provider} · {detail} · {elapsed}{count}{badge}"),
            )
        }
    };
    #[cfg(target_os = "macos")]
    {
        let _ = tray.set_title(Some(title.clone()));
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = &title; // title only used for tooltip off-mac
    }
    let _ = tray.set_tooltip(Some(tooltip));
}

fn cache_badge(state: &cch_core::CacheVisibilityState) -> &'static str {
    match state {
        cch_core::CacheVisibilityState::Rebuilding => " ⟳",
        cch_core::CacheVisibilityState::Normal => "",
    }
}
