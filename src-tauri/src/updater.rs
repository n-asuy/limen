//! In-app update over signed GitHub Release artifacts.
//!
//! The whole flow lives behind IPC commands (like the rest of this app) so the
//! frontend only invokes and renders. Network access happens exclusively here,
//! and only when the user explicitly checks or installs; nothing is contacted
//! in the background.

use serde::Serialize;
use tauri_plugin_updater::UpdaterExt;

#[derive(Serialize)]
pub struct AvailableUpdate {
    /// Version offered by the release manifest.
    pub version: String,
    /// Version currently running.
    pub current_version: String,
    /// Release notes from the manifest, if any.
    pub notes: Option<String>,
}

/// Ask the release endpoint whether a newer signed build exists.
/// `Ok(None)` means the app is already up to date.
#[tauri::command]
pub async fn check_for_update(app: tauri::AppHandle) -> Result<Option<AvailableUpdate>, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(AvailableUpdate {
            version: update.version.clone(),
            current_version: update.current_version.clone(),
            notes: update.body.clone(),
        })),
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Download, verify (minisign), install the pending update, then relaunch.
/// Re-checks first so the install always applies the manifest's current build.
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No update available".to_string())?;

    update
        .download_and_install(|_downloaded, _total| {}, || {})
        .await
        .map_err(|e| e.to_string())?;

    // Swap to the freshly installed bundle. This diverges (never returns).
    app.restart();
}
