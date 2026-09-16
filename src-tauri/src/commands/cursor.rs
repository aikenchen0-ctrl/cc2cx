//! Tauri commands for the isolated Cursor local integration.

use crate::{cursor::harness::CursorHarnessStatus, store::AppState};
use std::net::SocketAddr;

#[tauri::command]
pub async fn get_cursor_harness_status(
    state: tauri::State<'_, AppState>,
) -> Result<CursorHarnessStatus, String> {
    state
        .cursor_harness
        .status()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn initialize_cursor_ca(
    state: tauri::State<'_, AppState>,
) -> Result<CursorHarnessStatus, String> {
    state
        .cursor_harness
        .initialize_ca()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn install_cursor_ca(
    state: tauri::State<'_, AppState>,
) -> Result<CursorHarnessStatus, String> {
    state
        .cursor_harness
        .install_ca()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn uninstall_cursor_ca(
    state: tauri::State<'_, AppState>,
) -> Result<CursorHarnessStatus, String> {
    state
        .cursor_harness
        .uninstall_ca()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn start_cursor_integration(
    state: tauri::State<'_, AppState>,
) -> Result<CursorHarnessStatus, String> {
    state
        .cursor_harness
        .start()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn stop_cursor_integration(
    state: tauri::State<'_, AppState>,
) -> Result<CursorHarnessStatus, String> {
    state
        .cursor_harness
        .stop()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn configure_cursor_backend(
    state: tauri::State<'_, AppState>,
    address: Option<String>,
) -> Result<CursorHarnessStatus, String> {
    let parsed = address
        .map(|value| {
            value
                .parse::<SocketAddr>()
                .map_err(|_| "backend 地址必须是 host:port".to_string())
        })
        .transpose()?;
    if let Some(value) = parsed {
        if !value.ip().is_loopback() {
            return Err("Cursor backend 仅允许 loopback 地址".to_string());
        }
    }
    state
        .cursor_harness
        .configure_backend(parsed)
        .await
        .map_err(|e| e.to_string())
}
