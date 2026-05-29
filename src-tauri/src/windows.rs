//! Window management: the popover "panel" window and the "settings" window.

use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

/// Show (or create) and focus the popover panel window.
pub fn show_panel_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("panel") {
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }
    let window = WebviewWindowBuilder::new(app, "panel", WebviewUrl::App("index.html".into()))
        .title("Claude Code Hub Bar")
        .inner_size(420.0, 640.0)
        .min_inner_size(380.0, 480.0)
        .resizable(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()?;
    window.show()?;
    window.set_focus()?;
    Ok(())
}

/// Toggle visibility of the panel window.
pub fn toggle_panel_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("panel") {
        if window.is_visible().unwrap_or(false) {
            window.hide()?;
        } else {
            window.show()?;
            window.set_focus()?;
        }
        Ok(())
    } else {
        show_panel_window(app)
    }
}

/// Show (or create) and focus the settings window.
pub fn show_settings_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("settings") {
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }
    let window = WebviewWindowBuilder::new(
        app,
        "settings",
        WebviewUrl::App("settings.html".into()),
    )
    .title("CCHBar 设置")
    .inner_size(560.0, 640.0)
    .min_inner_size(480.0, 520.0)
    .resizable(true)
    .build()?;
    window.show()?;
    window.set_focus()?;
    Ok(())
}
