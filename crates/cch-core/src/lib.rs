//! Core logic for Claude Code Hub Bar.
//!
//! This crate is a faithful Rust port of the original macOS (SwiftUI) app's
//! business logic: the lenient JSON parsing layer, the CCH API client, the
//! number/money/date formatters, and the monitor state machine that builds the
//! menu-bar snapshot, cache-rebuild detection, and leaderboard aggregation.
//!
//! It is UI-framework agnostic so it can be driven by the Tauri shell on both
//! macOS and Windows.

pub mod api;
pub mod format;
pub mod jsonx;
pub mod models;
pub mod parse;
pub mod state;

pub use api::{ApiError, ApiService};
pub use models::*;
pub use state::{MonitorState, StatusBarPayload};
