use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Settings {
    pub shortcut: Option<String>,
}

fn settings_path<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

pub fn load<R: tauri::Runtime>(app: &AppHandle<R>) -> Settings {
    settings_path(app)
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save<R: tauri::Runtime>(app: &AppHandle<R>, settings: &Settings) -> Result<(), String> {
    fs::write(
        settings_path(app)?,
        serde_json::to_string_pretty(settings).unwrap(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_shortcut<R: tauri::Runtime>(app: AppHandle<R>) -> Option<String> {
    load(&app).shortcut
}

/// Re-registers the global shortcut at runtime and persists it.
/// `None` clears the shortcut entirely.
#[tauri::command]
pub fn set_shortcut(app: AppHandle, shortcut: Option<String>) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let gs = app.global_shortcut();
    gs.unregister_all().map_err(|e| e.to_string())?;
    if let Some(s) = &shortcut {
        gs.register(s.as_str())
            .map_err(|e| format!("ERR_SHORTCUT|{e}"))?;
    }
    save(&app, &Settings { shortcut })
}
