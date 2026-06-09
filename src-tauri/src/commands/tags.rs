use std::fs::File;
use std::io::BufReader;
use uesave::{Property, PropertyKey, Save, StructValue, ValueVec};
use serde::{Deserialize, Serialize};

pub const DEFAULT_TAG_NAMES: [&str; 4] = ["Player1", "Player2", "Player3", "Player4"];

fn is_custom_tag(name: &str) -> bool {
    !DEFAULT_TAG_NAMES.contains(&name)
}

/// Windows reserved device names that cannot be used as a file stem.
const RESERVED_FILE_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Make a tag name safe to use as a file stem on both Windows and macOS.
fn sanitize_file_stem(name: &str) -> String {
    let mut stem: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();

    // Windows rejects file names ending in a dot or space.
    while stem.ends_with('.') || stem.ends_with(' ') {
        stem.pop();
    }

    if RESERVED_FILE_NAMES.contains(&stem.to_ascii_uppercase().as_str()) {
        stem.insert(0, '_');
    }

    if stem.is_empty() {
        stem = "tag".into();
    }

    stem
}

fn tag_name_of(sv: &StructValue) -> Option<&str> {
    if let StructValue::Struct(props) = sv {
        if let Some(Property::Str(name)) = props.0.get(&PropertyKey::from("TagName")) {
            return Some(name.as_str());
        }
    }
    None
}

#[tauri::command]
pub async fn get_tag_names(save_path: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let file = File::open(&save_path).map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(file);
        let save = Save::read(&mut reader).map_err(|e| e.to_string())?;

        let tag_structs = match &save.root.properties["SavedPlayerTags"] {
            Property::Array(ValueVec::Struct(structs)) => structs,
            Property::Array(_) => return Err("SavedPlayerTags array does not contain structs".into()),
            _ => return Err("SavedPlayerTags is not an array".into()),
        };

        let tag_names = tag_structs
            .iter()
            .filter_map(|sv| tag_name_of(sv))
            .filter(|name| is_custom_tag(name))
            .map(|name| name.to_string())
            .collect();

        Ok(tag_names)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Export the named tags as individual .r2tag files (binary save format) into output_dir.
/// Returns the list of paths that were written.
#[tauri::command]
pub async fn export_tags(
    save_path: String,
    tag_names: Vec<String>,
    output_dir: String,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut written = Vec::new();
        let mut used_stems = std::collections::HashSet::new();

        for tag_name in &tag_names {
            let file = File::open(&save_path).map_err(|e| e.to_string())?;
            let mut reader = BufReader::new(file);
            let mut save = Save::read(&mut reader).map_err(|e| e.to_string())?;

            if let Property::Array(ValueVec::Struct(structs)) =
                &mut save.root.properties["SavedPlayerTags"]
            {
                structs.retain(|sv| tag_name_of(sv) == Some(tag_name.as_str()));
            } else {
                return Err("SavedPlayerTags is not a struct array".into());
            }

            // Distinct tag names can collide after sanitizing (e.g. "a/b" and "a:b");
            // suffix a counter so one export doesn't overwrite another.
            let base_stem = sanitize_file_stem(tag_name);
            let mut stem = base_stem.clone();
            let mut counter = 1;
            while !used_stems.insert(stem.clone()) {
                stem = format!("{base_stem} ({counter})");
                counter += 1;
            }

            let out_path = std::path::Path::new(&output_dir).join(format!("{stem}.r2tag"));
            let mut out_file = File::create(&out_path).map_err(|e| e.to_string())?;
            save.write(&mut out_file).map_err(|e| e.to_string())?;
            written.push(out_path.to_string_lossy().to_string());
        }

        Ok(written)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TagPreview {
    pub path: String,
    pub tag_name: String,
}

/// Read .r2tag files and return the tag name stored in each.
#[tauri::command]
pub async fn get_tag_previews(r2tag_paths: Vec<String>) -> Result<Vec<TagPreview>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut previews = Vec::new();

        for path in r2tag_paths {
            let file = File::open(&path).map_err(|e| e.to_string())?;
            let mut reader = BufReader::new(file);
            let save = Save::read(&mut reader).map_err(|e| e.to_string())?;

            let tag_structs = match &save.root.properties["SavedPlayerTags"] {
                Property::Array(ValueVec::Struct(structs)) => structs,
                _ => return Err(format!("{path}: unexpected SavedPlayerTags format")),
            };

            let name = tag_structs
                .iter()
                .find_map(|sv| tag_name_of(sv))
                .ok_or_else(|| format!("{path}: no tag name found"))?;

            previews.push(TagPreview {
                path,
                tag_name: name.to_string(),
            });
        }

        Ok(previews)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportInstruction {
    pub path: String,
    pub tag_name: String,
    pub overwrite: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
}

/// Import tags from .r2tag files into save_path.
/// Each instruction says whether to overwrite if the name already exists.
#[tauri::command]
pub async fn import_tags(
    save_path: String,
    instructions: Vec<ImportInstruction>,
) -> Result<ImportResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let file = File::open(&save_path).map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(file);
        let mut dest = Save::read(&mut reader).map_err(|e| e.to_string())?;

        let mut imported = Vec::new();
        let mut skipped = Vec::new();

        // Scope the mutable borrow of dest so dest.write() can proceed after the loop.
        {
            let dest_structs = match &mut dest.root.properties["SavedPlayerTags"] {
                Property::Array(ValueVec::Struct(structs)) => structs,
                _ => return Err("SavedPlayerTags is not a struct array in destination save".into()),
            };

            for instruction in instructions {
                let existing_pos = dest_structs
                    .iter()
                    .position(|sv| tag_name_of(sv) == Some(instruction.tag_name.as_str()));

                if existing_pos.is_some() && !instruction.overwrite {
                    skipped.push(instruction.tag_name);
                    continue;
                }

                let r2tag_file = File::open(&instruction.path).map_err(|e| e.to_string())?;
                let mut r2tag_reader = BufReader::new(r2tag_file);
                let r2tag_save = Save::read(&mut r2tag_reader).map_err(|e| e.to_string())?;

                let source_structs = match &r2tag_save.root.properties["SavedPlayerTags"] {
                    Property::Array(ValueVec::Struct(structs)) => structs,
                    _ => return Err(format!("{}: unexpected format", instruction.path)),
                };

                let tag_sv = source_structs
                    .iter()
                    .find(|sv| tag_name_of(sv) == Some(instruction.tag_name.as_str()))
                    .ok_or_else(|| format!("{}: tag '{}' not found", instruction.path, instruction.tag_name))?
                    .clone();

                if let Some(pos) = existing_pos {
                    dest_structs[pos] = tag_sv;
                } else {
                    dest_structs.push(tag_sv);
                }

                imported.push(instruction.tag_name);
            }
        }

        let out = File::create(&save_path).map_err(|e| e.to_string())?;
        dest.write(&mut std::io::BufWriter::new(out))
            .map_err(|e| e.to_string())?;

        Ok(ImportResult { imported, skipped })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::sanitize_file_stem;

    #[test]
    fn replaces_path_separators_and_reserved_chars() {
        assert_eq!(sanitize_file_stem("test/lower"), "test_lower");
        assert_eq!(sanitize_file_stem(r"back\slash"), "back_slash");
        assert_eq!(sanitize_file_stem("a:b*c?d\"e<f>g|h"), "a_b_c_d_e_f_g_h");
    }

    #[test]
    fn trims_trailing_dots_and_spaces() {
        assert_eq!(sanitize_file_stem("name. . "), "name");
    }

    #[test]
    fn prefixes_windows_reserved_names() {
        assert_eq!(sanitize_file_stem("CON"), "_CON");
        assert_eq!(sanitize_file_stem("com1"), "_com1");
    }

    #[test]
    fn falls_back_when_nothing_survives() {
        assert_eq!(sanitize_file_stem("..."), "tag");
        assert_eq!(sanitize_file_stem(""), "tag");
    }

    #[test]
    fn leaves_ordinary_names_untouched() {
        assert_eq!(sanitize_file_stem("Player Tag_42"), "Player Tag_42");
    }
}
