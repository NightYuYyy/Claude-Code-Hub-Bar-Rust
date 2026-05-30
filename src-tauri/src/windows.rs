//! Window management: the main "panel" window and the "settings" window.
//!
//! The panel adapts to the host platform:
//! - **macOS**: a borderless, always-on-top popover that lives next to the
//!   menu-bar item, is hidden on launch, and hides when it loses focus.
//! - **Windows / Linux**: a normal, always-visible application window with a
//!   title bar and taskbar entry. Closing or minimizing tucks it into the tray
//!   instead of quitting; the tray icon brings it back.

use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

/// Whether the panel behaves as a macOS-style popover (vs. a normal window).
pub const PANEL_IS_POPOVER: bool = cfg!(target_os = "macos");

/// Create the panel window with platform-appropriate chrome. Hidden on macOS
/// (shown via the tray), visible on desktop platforms.
fn build_panel<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<tauri::WebviewWindow<R>> {
    let mut builder = WebviewWindowBuilder::new(app, "panel", WebviewUrl::App("index.html".into()))
        .title("Claude Code Hub Bar")
        .inner_size(420.0, 640.0)
        .min_inner_size(380.0, 480.0)
        .resizable(true);

    if PANEL_IS_POPOVER {
        // macOS menu-bar popover.
        builder = builder
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .visible(false);
    } else {
        // Windows / Linux: a normal, always-on-screen application window.
        builder = builder
            .decorations(true)
            .always_on_top(false)
            .skip_taskbar(false)
            .visible(true)
            .center();
    }
    builder.build()
}

/// Ensure the panel window exists, returning a handle to it.
pub fn ensure_panel<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<tauri::WebviewWindow<R>> {
    if let Some(window) = app.get_webview_window("panel") {
        Ok(window)
    } else {
        build_panel(app)
    }
}

/// Show (or create) and focus the panel window. On desktop platforms this also
/// un-minimizes the window if it was minimized to the tray.
pub fn show_panel_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let window = ensure_panel(app)?;
    if window.is_minimized().unwrap_or(false) {
        window.unminimize()?;
    }
    window.show()?;
    window.set_focus()?;
    Ok(())
}

/// Toggle visibility of the panel window.
///
/// On macOS this mimics a popover (visible -> hide, hidden -> show). On desktop
/// platforms the tray click should reliably *reveal* the window, so a hidden or
/// minimized window is always restored; only an already-foreground window is
/// hidden back to the tray.
pub fn toggle_panel_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window("panel") else {
        return show_panel_window(app);
    };
    let visible = window.is_visible().unwrap_or(false);
    let minimized = window.is_minimized().unwrap_or(false);
    let focused = window.is_focused().unwrap_or(false);

    if PANEL_IS_POPOVER {
        if visible {
            window.hide()?;
        } else {
            window.show()?;
            window.set_focus()?;
        }
        return Ok(());
    }

    // Desktop: reveal unless it is already the foreground window.
    if visible && !minimized && focused {
        window.hide()?;
    } else {
        if minimized {
            window.unminimize()?;
        }
        window.show()?;
        window.set_focus()?;
    }
    Ok(())
}

/// Hide the panel to the tray (used by the close/minimize interception).
pub fn hide_panel_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("panel") {
        window.hide()?;
    }
    Ok(())
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
