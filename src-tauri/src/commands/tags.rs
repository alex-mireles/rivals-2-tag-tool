use std::fs::File;
use std::io::BufReader;
use uesave::{Property, Save, StructValue, ValueVec};

pub const DEFAULT_TAG_NAMES: [&str; 4] = ["Player1", "Player2", "Player3", "Player4"];

fn is_custom_tag(name: &str) -> bool {
    !DEFAULT_TAG_NAMES.contains(&name)
}

#[tauri::command]
pub fn get_tag_names(save_path: String) -> Result<Vec<String>, String> {
    let file = File::open(&save_path).map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(file);
    let save = Save::read(&mut reader).map_err(|error| error.to_string())?;

    let tags_property = &save.root.properties["SavedPlayerTags"];

    let tag_structs = match tags_property {
        Property::Array(ValueVec::Struct(structs)) => structs,
        Property::Array(_) => return Err("SavedPlayerTags array does not contain structs".into()),
        _ => return Err("SavedPlayerTags is not an array".into()),
    };

    let mut tag_names = Vec::new();

    for tag_value in tag_structs {
        if let StructValue::Struct(tag_properties) = tag_value {
            if let Property::Str(name) = &tag_properties["TagName"] {
                if is_custom_tag(name) {
                    tag_names.push(name.clone());
                }
            }
        }
    }

    Ok(tag_names)
}