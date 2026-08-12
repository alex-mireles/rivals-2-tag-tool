//! Persisted user preferences.
//!
//! Deliberately tiny: the only thing worth remembering between launches is a
//! save-file path the user picked by hand. In particular the cloud session
//! token is **never** written here — the app is expected to run on shared
//! tournament PCs, where a plaintext bearer token under %APPDATA% would be
//! readable by any other process on the machine.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// `#[serde(default)]` plus no `deny_unknown_fields` keeps a file written by a
/// newer build readable by an older one, and vice versa.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Save file the user chose explicitly. `None` means "use the default location".
    pub save_path: Option<String>,
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?
        .join("settings.json"))
}

/// Read settings, falling back to defaults for anything unreadable. A corrupt
/// settings file must not stop the app from starting.
pub fn load(app: &AppHandle) -> Settings {
    settings_path(app)
        .ok()
        .and_then(|path| fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Write settings atomically. A half-written settings.json (power loss, kill)
/// would otherwise read back as "your chosen path silently vanished".
pub fn store(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let json = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, json).map_err(|e| e.to_string())?;
    fs::rename(&temp, &path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn corrupt_or_empty_json_falls_back_to_default() {
        // `load` swallows these via `.ok()`; assert the parse step it relies on.
        assert!(serde_json::from_slice::<Settings>(b"").is_err());
        assert!(serde_json::from_slice::<Settings>(b"{\"savePath\":").is_err());
        assert!(Settings::default().save_path.is_none());
    }

    #[test]
    fn unknown_and_missing_fields_are_tolerated() {
        let forward: Settings = serde_json::from_slice(br#"{"savePath":"C:\\x.sav","future":1}"#)
            .expect("unknown fields must not break older builds");
        assert_eq!(forward.save_path.as_deref(), Some(r"C:\x.sav"));

        let empty: Settings = serde_json::from_slice(b"{}").expect("missing fields default");
        assert!(empty.save_path.is_none());
    }

    #[test]
    fn round_trips_through_camel_case() {
        let json = serde_json::to_string(&Settings {
            save_path: Some("path".into()),
        })
        .unwrap();
        assert!(json.contains("savePath"), "got {json}");
    }
}
