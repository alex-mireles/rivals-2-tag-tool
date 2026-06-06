pub mod tags;

use tauri::Manager;

#[tauri::command]
pub async fn get_default_save_path(app: tauri::AppHandle) -> Result<String, String> {
    let save_path = app
        .path()
        .local_data_dir()
        .map_err(|error| error.to_string())?
        .join("Rivals2")
        .join("Saved")
        .join("SaveGames")
        .join("Rivals2_PlayerTagSaveSlot.sav");

    if save_path.exists() {
        Ok(save_path.to_string_lossy().to_string())
    } else {
        Ok(String::new())
    }
}