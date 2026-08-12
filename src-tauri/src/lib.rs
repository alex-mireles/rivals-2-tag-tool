mod commands;
mod settings;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::save_file::resolve_save_file,
            commands::save_file::set_save_path,
            commands::tags::get_tag_names,
            commands::tags::export_tags,
            commands::tags::get_tag_previews,
            commands::tags::import_tags,
            commands::archive::pack_tags_from_save,
            commands::archive::pack_tag_files,
            commands::archive::unpack_r2pack,
            commands::cloud::cloud_begin_auth,
            commands::cloud::cloud_poll_auth,
            commands::cloud::cloud_end_session,
            commands::cloud::cloud_search_tags,
            commands::cloud::cloud_tournament_tags,
            commands::cloud::cloud_upload_tag,
            commands::cloud::cloud_delete_tag,
            commands::cloud::cloud_download_tags,
            commands::cloud::cleanup_cloud_files,
            commands::cloud::cleanup_stale_cloud_files,
            commands::update::check_for_update,
            commands::update::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
