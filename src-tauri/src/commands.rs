pub mod tags;

use tauri::Manager;

#[tauri::command]
pub fn get_default_save_path(app: tauri::AppHandle) -> Result<String, String> {
    let save_path = app
        .path()
        .local_data_dir()
        .map_err(|error| error.to_string())?
        .join("Rivals2\\Saved\\SaveGames\\Rivals2_PlayerTagSaveSlot.sav");

    Ok(save_path.to_string_lossy().to_string())
}