use std::fs::File;
use uesave::{Property, StructValue, ValueVec, Save};

#[tauri::command]
fn get_tag_names(save_path: String) -> Result<Vec<String>, String> {
    let mut file = File::open(&save_path).map_err(|error| error.to_string())?;
    let save = Save::read(&mut file).map_err(|error| error.to_string())?;

    let tags_property = &save.root.properties["SavedPlayerTags"];

    let tag_structs = match tags_property {
        Property::Array(ValueVec::Struct(structs)) => structs,
        Property::Array(_) => {
            return Err("SavedPlayerTags array does not contain structs".into())
        } _ => return Err("SavedPlayerTags is not an array".into()),
    };

    let mut tag_names = Vec::new();

    for tag_value in tag_structs {
        if let StructValue::Struct(tag_properties) = tag_value {
            if let Property::Str(name) = &tag_properties["TagName"] {
                tag_names.push(name.clone());
            }
        }
    }

    Ok(tag_names)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_tag_names])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}