use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, Cursor};
use uesave::{Property, PropertyKey, Save, StructValue, ValueVec};

pub const DEFAULT_TAG_NAMES: [&str; 4] = ["Player1", "Player2", "Player3", "Player4"];

fn is_custom_tag(name: &str) -> bool {
    !DEFAULT_TAG_NAMES.contains(&name)
}

/// Windows reserved device names that cannot be used as a file stem.
const RESERVED_FILE_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Bidi overrides and zero-width characters. A tag containing U+202E renders
/// `evil<RLO>gnp.r2tag` as `evilgat2r.png` in a file listing, so they are
/// dropped rather than mapped to `_`.
fn is_deceptive_char(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// Make a tag name safe to use as a file stem on both Windows and macOS.
pub(crate) fn sanitize_file_stem(name: &str) -> String {
    let mut stem: String = name
        .chars()
        .filter(|c| !is_deceptive_char(*c))
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

/// Sanitize `name` into a file stem that has not been handed out before,
/// suffixing ` (n)` on collision. Distinct tag names can collide after
/// sanitizing (e.g. "a/b" and "a:b" both become "a_b"), and one export must
/// never silently overwrite another.
pub(crate) fn unique_file_stem(name: &str, used: &mut HashSet<String>) -> String {
    let base = sanitize_file_stem(name);
    let mut stem = base.clone();
    let mut counter = 1;
    while !used.insert(stem.clone()) {
        stem = format!("{base} ({counter})");
        counter += 1;
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

/// Read the root `SaveVersion` (save-format version) from a save. Both `.sav`
/// files and `.r2tag` files (which are full saves) carry this. Returns `None`
/// if the property is absent or not an integer.
pub(crate) fn save_version(save: &Save) -> Option<i32> {
    match save
        .root
        .properties
        .0
        .get(&PropertyKey::from("SaveVersion"))
    {
        Some(Property::Int(v)) => Some(*v),
        _ => None,
    }
}

/// Open and parse a save (or `.r2tag`, which is a full save) from disk.
pub(crate) fn read_save(path: &str) -> Result<Save, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(file);
    Save::read(&mut reader).map_err(|e| e.to_string())
}

/// Custom tag names in a save, or `None` when `SavedPlayerTags` is missing or
/// isn't a struct array — i.e. this parsed as a save but isn't a tag save.
pub(crate) fn custom_tag_names(save: &Save) -> Option<Vec<String>> {
    let tag_structs = match save.root.properties.0.get(&PropertyKey::from("SavedPlayerTags")) {
        Some(Property::Array(ValueVec::Struct(structs))) => structs,
        _ => return None,
    };

    Some(
        tag_structs
            .iter()
            .filter_map(|sv| tag_name_of(sv))
            .filter(|name| is_custom_tag(name))
            .map(|name| name.to_string())
            .collect(),
    )
}

/// First tag name in a save, whether custom or built-in. A `.r2tag` holds
/// exactly one tag, so this identifies it.
pub(crate) fn first_tag_name(save: &Save) -> Option<&str> {
    match save.root.properties.0.get(&PropertyKey::from("SavedPlayerTags")) {
        Some(Property::Array(ValueVec::Struct(structs))) => {
            structs.iter().find_map(|sv| tag_name_of(sv))
        }
        _ => None,
    }
}

/// Build the exact single-tag save payload used by both local export and cloud
/// upload. This only reads the source save and never writes it.
pub(crate) fn single_tag_bytes(
    save_path: &str,
    tag_name: &str,
) -> Result<(Vec<u8>, Option<i32>), String> {
    if !is_custom_tag(tag_name) {
        return Err("Built-in player tags cannot be exported".into());
    }

    let mut save = read_save(save_path)?;
    let version = save_version(&save);

    let tag_structs = match &mut save.root.properties["SavedPlayerTags"] {
        Property::Array(ValueVec::Struct(structs)) => structs,
        _ => return Err("SavedPlayerTags is not a struct array".into()),
    };
    tag_structs.retain(|sv| tag_name_of(sv) == Some(tag_name));
    if tag_structs.len() != 1 {
        return Err(format!("Tag '{tag_name}' was not found exactly once"));
    }

    let mut bytes = Cursor::new(Vec::new());
    save.write(&mut bytes).map_err(|e| e.to_string())?;
    Ok((bytes.into_inner(), version))
}

#[tauri::command]
pub async fn get_tag_names(save_path: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        custom_tag_names(&read_save(&save_path)?)
            .ok_or_else(|| "SavedPlayerTags is missing or is not a struct array".to_string())
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
        let mut used_stems = HashSet::new();

        for tag_name in &tag_names {
            let (bytes, _) = single_tag_bytes(&save_path, tag_name)?;

            let stem = unique_file_stem(tag_name, &mut used_stems);
            let out_path = std::path::Path::new(&output_dir).join(format!("{stem}.r2tag"));
            std::fs::write(&out_path, bytes).map_err(|e| e.to_string())?;
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
    /// Save-format version embedded in the .r2tag, or `None` if unreadable.
    pub version: Option<i32>,
    /// True only when the .r2tag's version matches the loaded save's version.
    pub compatible: bool,
    /// Why this entry can't be imported, when the file itself was unreadable.
    /// Set per entry so one bad file in a 40-tag pack doesn't sink the batch.
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreviewResult {
    /// Version of the currently loaded save, used as the compatibility target.
    pub save_version: Option<i32>,
    pub previews: Vec<TagPreview>,
}

/// Read .r2tag files and return the tag name + save-format version stored in
/// each, flagging whether each matches the loaded save's version.
#[tauri::command]
pub async fn get_tag_previews(
    r2tag_paths: Vec<String>,
    save_path: String,
) -> Result<PreviewResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dest = read_save(&save_path)?;
        let dest_version = save_version(&dest);

        let mut previews = Vec::new();

        for path in r2tag_paths {
            // A single corrupt or truncated file becomes one unimportable row
            // rather than an error that discards the whole batch.
            let preview = match read_save(&path) {
                Err(error) => TagPreview {
                    tag_name: file_stem_of(&path),
                    path,
                    version: None,
                    compatible: false,
                    error: Some(error),
                },
                Ok(save) => match first_tag_name(&save) {
                    None => TagPreview {
                        tag_name: file_stem_of(&path),
                        path,
                        version: None,
                        compatible: false,
                        error: Some("No player tag found in this file".into()),
                    },
                    Some(name) => {
                        let version = save_version(&save);
                        TagPreview {
                            path,
                            tag_name: name.to_string(),
                            version,
                            compatible: version.is_some() && version == dest_version,
                            error: None,
                        }
                    }
                },
            };
            previews.push(preview);
        }

        Ok(PreviewResult {
            save_version: dest_version,
            previews,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Fallback label for a file we couldn't read a tag name out of.
fn file_stem_of(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
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
    /// Tags rejected because their save-format version differs from the
    /// destination save (importing them would fail to write or corrupt data).
    pub incompatible: Vec<String>,
}

/// Import tags from .r2tag files into save_path.
/// Each instruction says whether to overwrite if the name already exists.
#[tauri::command]
pub async fn import_tags(
    save_path: String,
    instructions: Vec<ImportInstruction>,
) -> Result<ImportResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut dest = read_save(&save_path)?;
        let dest_version = save_version(&dest);

        let mut imported = Vec::new();
        let mut skipped = Vec::new();
        let mut incompatible = Vec::new();

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

                let r2tag_save = read_save(&instruction.path)?;

                // Reject cross-version imports: a tag from a different save format
                // can't be written into this save (or would lose/garble settings).
                let source_version = save_version(&r2tag_save);
                if source_version.is_none() || source_version != dest_version {
                    incompatible.push(instruction.tag_name);
                    continue;
                }

                let source_structs = match &r2tag_save.root.properties["SavedPlayerTags"] {
                    Property::Array(ValueVec::Struct(structs)) => structs,
                    _ => return Err(format!("{}: unexpected format", instruction.path)),
                };

                let tag_sv = source_structs
                    .iter()
                    .find(|sv| tag_name_of(sv) == Some(instruction.tag_name.as_str()))
                    .ok_or_else(|| {
                        format!(
                            "{}: tag '{}' not found",
                            instruction.path, instruction.tag_name
                        )
                    })?
                    .clone();

                if let Some(pos) = existing_pos {
                    dest_structs[pos] = tag_sv;
                } else {
                    dest_structs.push(tag_sv);
                }

                imported.push(instruction.tag_name);
            }
        }

        // Serialize into a sibling temp file and rename over the original, so a
        // failure part-way through (disk full, antivirus, the game holding the
        // handle) leaves the existing save untouched instead of truncated.
        // `fs::rename` replaces the destination on both Windows and Unix.
        let temp_path = std::path::Path::new(&save_path).with_extension("sav.tmp");
        let write_result = (|| -> Result<(), String> {
            let out = File::create(&temp_path).map_err(|e| e.to_string())?;
            let mut writer = std::io::BufWriter::new(out);
            dest.write(&mut writer).map_err(|e| e.to_string())?;
            // Surface a failed flush rather than letting BufWriter swallow it on drop.
            writer.into_inner().map_err(|e| e.to_string())?;
            Ok(())
        })();

        if let Err(error) = write_result {
            let _ = std::fs::remove_file(&temp_path);
            return Err(error);
        }
        if let Err(error) = std::fs::rename(&temp_path, &save_path) {
            let _ = std::fs::remove_file(&temp_path);
            return Err(error.to_string());
        }

        Ok(ImportResult {
            imported,
            skipped,
            incompatible,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::{sanitize_file_stem, unique_file_stem};
    use std::collections::HashSet;

    #[test]
    fn distinct_names_that_sanitize_alike_get_unique_stems() {
        let mut used = HashSet::new();
        assert_eq!(unique_file_stem("a/b", &mut used), "a_b");
        assert_eq!(unique_file_stem("a:b", &mut used), "a_b (1)");
        assert_eq!(unique_file_stem("a*b", &mut used), "a_b (2)");
        assert_eq!(unique_file_stem("other", &mut used), "other");
    }

    #[test]
    fn strips_bidi_and_zero_width_characters() {
        // U+202E would otherwise make "evil<RLO>gnp.r2tag" display as an image.
        assert_eq!(sanitize_file_stem("evil\u{202E}gnp"), "evilgnp");
        assert_eq!(sanitize_file_stem("a\u{200B}b"), "ab");
        assert_eq!(sanitize_file_stem("\u{202E}"), "tag");
    }

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
