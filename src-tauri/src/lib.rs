mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_default_save_path,
            commands::tags::get_tag_names,
            commands::tags::export_tags,
            commands::tags::get_tag_previews,
            commands::tags::read_tag_json,
            commands::tags::import_tags,
            commands::startgg::startgg_search,
            commands::startgg::startgg_user,
            commands::startgg::startgg_event,
            commands::site::share_tags_to_site,
            commands::site::fetch_shared_tags,
            commands::site::download_tags,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
