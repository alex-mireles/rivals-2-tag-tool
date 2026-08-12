//! Resolving the Rivals 2 tag save file.
//!
//! The save lives at a fixed, known location, so the app resolves and reads it
//! on startup rather than making the user pick it from a dialog every launch.
//! A path the user chose by hand wins over the default and persists.
//!
//! `Err` here is reserved for a broken environment (the OS can't tell us where
//! local app data lives). "File is missing", "wrong file", and "can't be read"
//! are ordinary *states* the UI renders with a recovery action — not errors.

use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::tags::{custom_tag_names, read_save, save_version};
use crate::settings;

pub const EXPECTED_SAVE_FILE_NAME: &str = "Rivals2_PlayerTagSaveSlot.sav";

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SaveStatus {
    /// Parsed, holds a tag array; `tag_names` is authoritative.
    Ready,
    /// Path known (chosen or default) but nothing is there.
    Missing,
    /// Basename isn't the expected save file.
    WrongFile,
    /// Present, but open/parse failed — often the game holding the handle.
    Unreadable,
    /// Parsed as a save, but has no `SavedPlayerTags` array.
    Unsupported,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveFileInfo {
    pub path: String,
    /// Where `path` came from: `"saved"`, `"default"`, or `"none"`.
    pub source: &'static str,
    pub status: SaveStatus,
    /// Directory to open file dialogs in. Populated even when the save is
    /// absent, so a machine without Rivals 2 still opens somewhere sensible.
    pub default_dir: String,
    /// Only populated when `status` is `Ready`.
    pub tag_names: Vec<String>,
    pub save_version: Option<i32>,
    /// Underlying failure text for `Unreadable`.
    pub error: Option<String>,
}

fn default_save_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .local_data_dir()
        .map_err(|error| error.to_string())?
        .join("Rivals2")
        .join("Saved")
        .join("SaveGames")
        .join(EXPECTED_SAVE_FILE_NAME))
}

/// Inspect `path` and build the full state the UI needs. Blocking: parses the save.
fn inspect(path: String, source: &'static str, default_dir: String) -> SaveFileInfo {
    let base = |status: SaveStatus| SaveFileInfo {
        path: path.clone(),
        source,
        status,
        default_dir: default_dir.clone(),
        tag_names: Vec::new(),
        save_version: None,
        error: None,
    };

    if path.is_empty() {
        return base(SaveStatus::Missing);
    }

    let file = std::path::Path::new(&path);
    if !file.is_file() {
        return base(SaveStatus::Missing);
    }

    // Checked before parsing: this is the guard against pointing at some other
    // Rivals 2 save slot, which would parse fine and then be written back wrong.
    let name_matches = file
        .file_name()
        .map(|name| name.eq_ignore_ascii_case(EXPECTED_SAVE_FILE_NAME))
        .unwrap_or(false);
    if !name_matches {
        return base(SaveStatus::WrongFile);
    }

    let save = match read_save(&path) {
        Ok(save) => save,
        Err(error) => {
            let mut info = base(SaveStatus::Unreadable);
            info.error = Some(error);
            return info;
        }
    };

    match custom_tag_names(&save) {
        None => base(SaveStatus::Unsupported),
        Some(tag_names) => SaveFileInfo {
            tag_names,
            save_version: save_version(&save),
            ..base(SaveStatus::Ready)
        },
    }
}

fn resolve(app: &AppHandle) -> Result<SaveFileInfo, String> {
    let default_path = default_save_path(app)?;
    let default_dir = default_path
        .parent()
        .map(|dir| dir.to_string_lossy().to_string())
        .unwrap_or_default();

    // A path the user picked by hand is kept even when it currently points at
    // nothing — the drive may just be unplugged. Reverting silently would be
    // far more confusing than reporting `Missing` with a reset action.
    let (path, source) = match settings::load(app).save_path {
        Some(saved) if !saved.trim().is_empty() => (saved, "saved"),
        _ => (default_path.to_string_lossy().to_string(), "default"),
    };

    Ok(inspect(path, source, default_dir))
}

/// Resolve the save file and read its tags in one round trip. Also serves as
/// "reload" — there is no separate refresh command.
#[tauri::command]
pub async fn resolve_save_file(app: AppHandle) -> Result<SaveFileInfo, String> {
    tauri::async_runtime::spawn_blocking(move || resolve(&app))
        .await
        .map_err(|e| e.to_string())?
}

/// Persist a hand-picked save path and re-resolve. An empty `path` clears the
/// override, falling back to the default location.
#[tauri::command]
pub async fn set_save_path(app: AppHandle, path: String) -> Result<SaveFileInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let trimmed = path.trim();
        settings::store(
            &app,
            &settings::Settings {
                save_path: (!trimmed.is_empty()).then(|| trimmed.to_string()),
            },
        )?;
        resolve(&app)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_absent_paths_report_missing() {
        let info = inspect(String::new(), "none", "D".into());
        assert!(matches!(info.status, SaveStatus::Missing));

        let info = inspect(
            r"Z:\definitely\not\here\Rivals2_PlayerTagSaveSlot.sav".into(),
            "default",
            "D".into(),
        );
        assert!(matches!(info.status, SaveStatus::Missing));
        // The path is still reported so the UI can show where it looked.
        assert!(info.path.ends_with(EXPECTED_SAVE_FILE_NAME));
    }

    #[test]
    fn wrong_name_is_detected_before_parsing() {
        let dir = std::env::temp_dir().join("r2tt-save-file-test");
        std::fs::create_dir_all(&dir).unwrap();
        // Not a valid save at all: if the name check ran after parsing, this
        // would come back Unreadable instead of WrongFile.
        let path = dir.join("SomeOtherSlot.sav");
        std::fs::write(&path, b"not a save").unwrap();

        let info = inspect(path.to_string_lossy().to_string(), "saved", "D".into());
        assert!(matches!(info.status, SaveStatus::WrongFile));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn correctly_named_but_corrupt_file_is_unreadable() {
        let dir = std::env::temp_dir().join("r2tt-save-file-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(EXPECTED_SAVE_FILE_NAME);
        std::fs::write(&path, b"not a save").unwrap();

        let info = inspect(path.to_string_lossy().to_string(), "default", "D".into());
        assert!(matches!(info.status, SaveStatus::Unreadable));
        assert!(info.error.is_some(), "failure text should reach the UI");

        std::fs::remove_file(&path).ok();
    }
}
