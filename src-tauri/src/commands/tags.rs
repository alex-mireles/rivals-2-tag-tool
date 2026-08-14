use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use uesave::{Property, PropertyKey, Save, StructValue, ValueVec};

/// Refuse a `.r2tag` that claims one of the game's own profiles. Export already
/// blocks these; without the same guard on the way in, a hand-made file
/// overwrites a Player1–Player4 profile the UI can neither list nor restore.
const BUILT_IN_REJECTED: &str = "Built-in player tags cannot be imported";

pub const DEFAULT_TAG_NAMES: [&str; 4] = ["Player1", "Player2", "Player3", "Player4"];
pub const MAX_CUSTOM_TAGS: usize = 96;

static UNIQUE_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

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

/// The `SavedPlayerTags` array, or `None` when the property is absent or has
/// another shape. Always reach for these rather than `properties["..."]`:
/// `Properties` indexes an `IndexMap`, which *panics* on a missing key, and a
/// save (or a hand-made `.r2tag`) with no tag array is an ordinary input.
fn tag_array(save: &Save) -> Option<&Vec<StructValue>> {
    match save
        .root
        .properties
        .0
        .get(&PropertyKey::from("SavedPlayerTags"))
    {
        Some(Property::Array(ValueVec::Struct(structs))) => Some(structs),
        _ => None,
    }
}

fn tag_array_mut(save: &mut Save) -> Option<&mut Vec<StructValue>> {
    match save
        .root
        .properties
        .0
        .get_mut(&PropertyKey::from("SavedPlayerTags"))
    {
        Some(Property::Array(ValueVec::Struct(structs))) => Some(structs),
        _ => None,
    }
}

const NOT_A_TAG_SAVE: &str = "SavedPlayerTags is missing or is not a struct array";

/// One `.r2tag` payload: the serialized save bytes plus the save-format version
/// they carry, which is what the destination has to match on import.
pub(crate) type TagPayload = (Vec<u8>, Option<i32>);

/// Custom tag names in a save, or `None` when `SavedPlayerTags` is missing or
/// isn't a struct array — i.e. this parsed as a save but isn't a tag save.
pub(crate) fn custom_tag_names(save: &Save) -> Option<Vec<String>> {
    Some(
        tag_array(save)?
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
    tag_array(save)?.iter().find_map(|sv| tag_name_of(sv))
}

/// Build the single-tag save payloads for several tags from **one** parse of
/// the source save. Every tag comes out of the same immutable file, so
/// re-reading and re-parsing it per tag (which is what calling
/// `single_tag_bytes` in a loop does) is pure waste on a 40-tag export.
///
/// Only reads the source save; the mutation below is to the in-memory copy.
pub(crate) fn single_tag_bytes_batch(
    save_path: &str,
    tag_names: &[String],
) -> Result<Vec<TagPayload>, String> {
    let mut save = read_save(save_path)?;
    let version = save_version(&save);
    let all_tags = tag_array(&save).ok_or(NOT_A_TAG_SAVE)?.clone();

    let mut out = Vec::with_capacity(tag_names.len());
    for tag_name in tag_names {
        if !is_custom_tag(tag_name) {
            return Err("Built-in player tags cannot be exported".into());
        }

        let only: Vec<StructValue> = all_tags
            .iter()
            .filter(|sv| tag_name_of(sv) == Some(tag_name.as_str()))
            .cloned()
            .collect();
        if only.len() != 1 {
            return Err(format!("Tag '{tag_name}' was not found exactly once"));
        }

        // Swap the one tag into the shared parse instead of re-reading the file.
        *tag_array_mut(&mut save).ok_or(NOT_A_TAG_SAVE)? = only;
        let mut bytes = Cursor::new(Vec::new());
        save.write(&mut bytes).map_err(|e| e.to_string())?;
        out.push((bytes.into_inner(), version));
    }
    Ok(out)
}

/// Build the exact single-tag save payload used by both local export and cloud
/// upload. This only reads the source save and never writes it.
pub(crate) fn single_tag_bytes(save_path: &str, tag_name: &str) -> Result<TagPayload, String> {
    single_tag_bytes_batch(save_path, &[tag_name.to_string()])?
        .pop()
        .ok_or_else(|| format!("Tag '{tag_name}' was not found exactly once"))
}

#[tauri::command]
pub async fn get_tag_names(save_path: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        custom_tag_names(&read_save(&save_path)?).ok_or_else(|| NOT_A_TAG_SAVE.to_string())
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
        // One parse of the save covers every selected tag.
        let payloads = single_tag_bytes_batch(&save_path, &tag_names)?;
        let mut written = Vec::with_capacity(payloads.len());
        let mut used_stems = HashSet::new();

        for (tag_name, (bytes, _)) in tag_names.iter().zip(payloads) {
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
                    // Surfaced as an unimportable row, so the built-in guard in
                    // `import_tags` is a backstop rather than the only check.
                    Some(name) if !is_custom_tag(name) => TagPreview {
                        tag_name: name.to_string(),
                        version: save_version(&save),
                        path,
                        compatible: false,
                        error: Some(BUILT_IN_REJECTED.into()),
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ImportMode {
    Merge,
    ReplaceCustom,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
    /// Tags rejected because their save-format version differs from the
    /// destination save (importing them would fail to write or corrupt data).
    pub incompatible: Vec<String>,
    /// Existing custom tags removed by a replacement import.
    pub removed: Vec<String>,
    /// Byte-for-byte backup made before a replacement import changed the save.
    pub backup_path: Option<String>,
}

struct PreparedImport {
    name: String,
    value: StructValue,
    existing_pos: Option<usize>,
}

fn ensure_no_duplicate_instructions(instructions: &[ImportInstruction]) -> Result<(), String> {
    let mut names = HashSet::with_capacity(instructions.len());
    for instruction in instructions {
        if !names.insert(instruction.tag_name.as_str()) {
            return Err(format!(
                "Tag '{}' was selected more than once",
                instruction.tag_name
            ));
        }
    }
    Ok(())
}

fn ensure_custom_tag_capacity(count: usize) -> Result<(), String> {
    if count > MAX_CUSTOM_TAGS {
        return Err(format!(
            "This import would leave {count} custom tags, but Rivals II supports at most {MAX_CUSTOM_TAGS}"
        ));
    }
    Ok(())
}

fn unique_sibling_name(path: &Path, marker: &str, extension: &str) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "tag-save".into());
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = UNIQUE_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    parent.join(format!(
        "{file_name}.{marker}-{stamp}-{}-{counter}.{extension}",
        std::process::id(),
    ))
}

fn create_unique_sibling(
    path: &Path,
    marker: &str,
    extension: &str,
) -> Result<(PathBuf, File), String> {
    for _ in 0..100 {
        let candidate = unique_sibling_name(path, marker, extension);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("Could not create a unique sibling file".into())
}

fn write_temp_save(dest: &Save, save_path: &Path) -> Result<PathBuf, String> {
    let (temp_path, out) = create_unique_sibling(save_path, "import", "tmp")?;
    let write_result = (|| -> Result<(), String> {
        let mut writer = io::BufWriter::new(out);
        dest.write(&mut writer).map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
        let out = writer
            .into_inner()
            .map_err(|e| e.into_error().to_string())?;
        out.sync_all().map_err(|e| e.to_string())
    })();

    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&temp_path);
        return Err(error);
    }
    Ok(temp_path)
}

fn create_replacement_backup(save_path: &Path) -> Result<PathBuf, String> {
    let (backup_path, mut backup) = create_unique_sibling(save_path, "pre-replace", "bak")?;
    let copy_result = (|| -> Result<(), String> {
        let mut source = File::open(save_path).map_err(|e| e.to_string())?;
        io::copy(&mut source, &mut backup).map_err(|e| e.to_string())?;
        backup.flush().map_err(|e| e.to_string())?;
        backup.sync_all().map_err(|e| e.to_string())
    })();
    if let Err(error) = copy_result {
        let _ = std::fs::remove_file(&backup_path);
        return Err(error);
    }
    Ok(backup_path)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // Both paths are sibling files created by this process. MoveFileExW gives
    // Windows the replace-existing behavior that `std::fs::rename` lacks.
    let succeeded = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if succeeded == 0 {
        Err(io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    std::fs::rename(source, destination).map_err(|e| e.to_string())
}

fn empty_import_result(incompatible: Vec<String>) -> ImportResult {
    ImportResult {
        imported: Vec::new(),
        skipped: Vec::new(),
        incompatible,
        removed: Vec::new(),
        backup_path: None,
    }
}

/// Import tags from .r2tag files into save_path. Merge honors each
/// instruction's overwrite choice; replace-custom keeps the built-in profiles
/// and replaces every custom tag as one transaction.
#[tauri::command]
pub async fn import_tags(
    save_path: String,
    instructions: Vec<ImportInstruction>,
    mode: ImportMode,
) -> Result<ImportResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        ensure_no_duplicate_instructions(&instructions)?;
        if mode == ImportMode::ReplaceCustom && instructions.is_empty() {
            return Err("Select at least one tag before replacing existing custom tags".into());
        }

        let mut dest = read_save(&save_path)?;
        let dest_version = save_version(&dest);
        let mut skipped = Vec::new();
        let mut incompatible = Vec::new();
        let dest_structs = tag_array(&dest)
            .ok_or("SavedPlayerTags is missing or is not a struct array in destination save")?;

        if mode == ImportMode::ReplaceCustom
            && dest_structs.iter().any(|sv| tag_name_of(sv).is_none())
        {
            return Err(
                "Destination SavedPlayerTags contains an entry without a valid TagName".into(),
            );
        }

        let mut prepared = Vec::with_capacity(instructions.len());
        for instruction in instructions {
            // The game's own profiles are excluded from listing and export,
            // so nothing this app produces names one. A file that does is
            // rejected rather than allowed to replace a profile the user
            // can neither see in the UI nor restore afterwards.
            if !is_custom_tag(instruction.tag_name.as_str()) {
                incompatible.push(instruction.tag_name);
                continue;
            }

            let existing_pos = dest_structs
                .iter()
                .position(|sv| tag_name_of(sv) == Some(instruction.tag_name.as_str()));
            if mode == ImportMode::Merge && existing_pos.is_some() && !instruction.overwrite {
                skipped.push(instruction.tag_name);
                continue;
            }

            let r2tag_save = read_save(&instruction.path)?;
            let source_version = save_version(&r2tag_save);
            if source_version.is_none() || source_version != dest_version {
                incompatible.push(instruction.tag_name);
                continue;
            }

            let source_structs = tag_array(&r2tag_save)
                .ok_or_else(|| format!("{}: unexpected format", instruction.path))?;
            let mut matches = source_structs
                .iter()
                .filter(|sv| tag_name_of(sv) == Some(instruction.tag_name.as_str()));
            let value = matches
                .next()
                .ok_or_else(|| {
                    format!(
                        "{}: tag '{}' not found",
                        instruction.path, instruction.tag_name
                    )
                })?
                .clone();
            if matches.next().is_some() {
                return Err(format!(
                    "{}: tag '{}' appears more than once",
                    instruction.path, instruction.tag_name
                ));
            }
            prepared.push(PreparedImport {
                name: instruction.tag_name,
                value,
                existing_pos,
            });
        }

        // Replacement must never silently wipe the destination down to only
        // the subset whose files happened to pass validation.
        if mode == ImportMode::ReplaceCustom && !incompatible.is_empty() {
            return Ok(empty_import_result(incompatible));
        }

        let current_custom_count = dest_structs
            .iter()
            .filter_map(tag_name_of)
            .filter(|name| is_custom_tag(name))
            .count();
        let added_custom_count = prepared
            .iter()
            .filter(|item| item.existing_pos.is_none())
            .count();
        match mode {
            // An already-oversized hand-made save can still overwrite tags;
            // only an operation that grows it is newly violating the limit.
            ImportMode::Merge if added_custom_count > 0 => {
                ensure_custom_tag_capacity(current_custom_count + added_custom_count)?;
            }
            ImportMode::ReplaceCustom => ensure_custom_tag_capacity(prepared.len())?,
            ImportMode::Merge => {}
        }

        if prepared.is_empty() {
            return Ok(ImportResult {
                imported: Vec::new(),
                skipped,
                incompatible,
                removed: Vec::new(),
                backup_path: None,
            });
        }

        let imported: Vec<String> = prepared.iter().map(|item| item.name.clone()).collect();
        let removed = match mode {
            ImportMode::Merge => {
                let dest_structs = tag_array_mut(&mut dest).ok_or(NOT_A_TAG_SAVE)?;
                for item in prepared {
                    if let Some(pos) = item.existing_pos {
                        dest_structs[pos] = item.value;
                    } else {
                        dest_structs.push(item.value);
                    }
                }
                Vec::new()
            }
            ImportMode::ReplaceCustom => {
                let dest_structs = tag_array_mut(&mut dest).ok_or(NOT_A_TAG_SAVE)?;
                let removed: Vec<String> = dest_structs
                    .iter()
                    .filter_map(tag_name_of)
                    .filter(|name| is_custom_tag(name))
                    .map(str::to_string)
                    .collect();
                dest_structs.retain(|sv| tag_name_of(sv).is_some_and(|name| !is_custom_tag(name)));
                dest_structs.extend(prepared.into_iter().map(|item| item.value));
                removed
            }
        };

        let save_path = Path::new(&save_path);
        let temp_path = write_temp_save(&dest, save_path)?;
        let backup_path = if mode == ImportMode::ReplaceCustom {
            match create_replacement_backup(save_path) {
                Ok(path) => Some(path),
                Err(error) => {
                    let _ = std::fs::remove_file(&temp_path);
                    return Err(format!("Could not back up the destination save: {error}"));
                }
            }
        } else {
            None
        };

        if let Err(error) = replace_file(&temp_path, save_path) {
            let _ = std::fs::remove_file(&temp_path);
            let backup_note = backup_path
                .as_ref()
                .map(|path| format!(" Backup: {}.", path.to_string_lossy()))
                .unwrap_or_default();
            return Err(format!(
                "Could not replace the destination save.{backup_note} {error}"
            ));
        }

        Ok(ImportResult {
            imported,
            skipped,
            incompatible,
            removed,
            backup_path: backup_path.map(|path| path.to_string_lossy().to_string()),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::{
        create_replacement_backup, ensure_custom_tag_capacity, ensure_no_duplicate_instructions,
        is_custom_tag, replace_file, sanitize_file_stem, unique_file_stem, unique_sibling_name,
        ImportInstruction, ImportMode, DEFAULT_TAG_NAMES, MAX_CUSTOM_TAGS,
    };
    use std::collections::HashSet;

    /// Import and export must agree on which names are the game's own, or a
    /// hand-made `.r2tag` walks in through the side the guard is missing from.
    #[test]
    fn built_in_tag_names_are_not_custom() {
        for name in DEFAULT_TAG_NAMES {
            assert!(
                !is_custom_tag(name),
                "{name} must be rejected on both sides"
            );
        }
        assert!(is_custom_tag("Player5"));
        assert!(is_custom_tag("HYPER"));
        // Case-sensitive by design: the game writes these names exactly.
        assert!(is_custom_tag("player1"));
    }

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

    #[test]
    fn import_mode_uses_the_frontend_wire_names() {
        assert_eq!(
            serde_json::from_str::<ImportMode>(r#""merge""#).unwrap(),
            ImportMode::Merge
        );
        assert_eq!(
            serde_json::from_str::<ImportMode>(r#""replace-custom""#).unwrap(),
            ImportMode::ReplaceCustom
        );
    }

    #[test]
    fn capacity_accepts_96_custom_tags_and_rejects_97() {
        assert!(ensure_custom_tag_capacity(MAX_CUSTOM_TAGS).is_ok());
        let error = ensure_custom_tag_capacity(MAX_CUSTOM_TAGS + 1).unwrap_err();
        assert!(error.contains("97 custom tags"));
        assert!(error.contains("at most 96"));
    }

    #[test]
    fn duplicate_import_names_are_rejected_in_input_order() {
        let instruction = |name: &str| ImportInstruction {
            path: format!("{name}.r2tag"),
            tag_name: name.into(),
            overwrite: false,
        };
        let instructions = vec![instruction("A"), instruction("B"), instruction("A")];
        assert_eq!(
            ensure_no_duplicate_instructions(&instructions).unwrap_err(),
            "Tag 'A' was selected more than once"
        );
    }

    #[test]
    fn replacement_backup_is_an_exact_sibling_copy() {
        let base = std::env::temp_dir().join("rivals-2-tag-tool-backup-test.sav");
        let source = unique_sibling_name(&base, "source", "sav");
        std::fs::write(&source, b"exact save bytes").unwrap();

        let backup = create_replacement_backup(&source).unwrap();
        assert_eq!(backup.parent(), source.parent());
        assert_eq!(std::fs::read(&backup).unwrap(), b"exact save bytes");
        assert_eq!(backup.extension().unwrap(), "bak");

        std::fs::remove_file(source).unwrap();
        std::fs::remove_file(backup).unwrap();
    }

    #[test]
    fn file_replacement_overwrites_without_deleting_first() {
        let base = std::env::temp_dir().join("rivals-2-tag-tool-replace-test.sav");
        let destination = unique_sibling_name(&base, "destination", "sav");
        let source = unique_sibling_name(&base, "source", "tmp");
        std::fs::write(&destination, b"old").unwrap();
        std::fs::write(&source, b"new").unwrap();

        replace_file(&source, &destination).unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"new");
        assert!(!source.exists());

        std::fs::remove_file(destination).unwrap();
    }

    /// Exercises the real export path, which unit tests can't otherwise reach:
    /// `uesave`'s types can't be constructed outside their crate, so there is no
    /// way to hand-build a save fixture. Opt in against your own save with:
    ///
    /// ```text
    /// R2_SAVE="$LOCALAPPDATA/Rivals2/Saved/SaveGames/Rivals2_PlayerTagSaveSlot.sav" \
    ///   cargo test --manifest-path src-tauri/Cargo.toml -- --ignored
    /// ```
    #[test]
    #[ignore = "needs a real save; set R2_SAVE"]
    fn exported_payloads_carry_exactly_their_own_tag() {
        use super::{custom_tag_names, first_tag_name, read_save, single_tag_bytes_batch};
        use std::io::Cursor;
        use uesave::Save;

        let Ok(path) = std::env::var("R2_SAVE") else {
            eprintln!("skipped: set R2_SAVE to a Rivals2_PlayerTagSaveSlot.sav");
            return;
        };
        let names = custom_tag_names(&read_save(&path).unwrap()).unwrap();
        assert!(!names.is_empty(), "save has no custom tags to export");

        let payloads = single_tag_bytes_batch(&path, &names).unwrap();
        assert_eq!(payloads.len(), names.len());
        for (name, (bytes, version)) in names.iter().zip(&payloads) {
            assert!(version.is_some(), "save version lost for {name}");
            // A .r2tag is a complete save carrying exactly one tag — its own.
            let parsed = Save::read(&mut Cursor::new(bytes)).unwrap();
            assert_eq!(first_tag_name(&parsed), Some(name.as_str()));
            assert_eq!(custom_tag_names(&parsed).unwrap(), vec![name.clone()]);
        }
    }

    #[test]
    #[ignore = "needs a real save; set R2_SAVE"]
    fn replace_custom_round_trip_preserves_the_save_and_creates_a_backup() {
        use super::{
            custom_tag_names, import_tags, read_save, single_tag_bytes_batch, tag_array,
            tag_name_of, ImportInstruction, ImportMode, UNIQUE_FILE_COUNTER,
        };
        use std::sync::atomic::Ordering;
        use uesave::{PropertyKey, Save};

        let Ok(source_path) = std::env::var("R2_SAVE") else {
            eprintln!("skipped: set R2_SAVE to a Rivals II tag save");
            return;
        };
        let source_bytes = std::fs::read(&source_path).unwrap();
        let source_save = read_save(&source_path).unwrap();
        let source_names = custom_tag_names(&source_save).unwrap();
        assert!(
            !source_names.is_empty(),
            "save has no custom tags to import"
        );

        let selected: Vec<String> = source_names.iter().take(2).cloned().collect();
        let payloads = single_tag_bytes_batch(&source_path, &selected).unwrap();
        let dir = std::env::temp_dir().join(format!(
            "r2tt-replace-round-trip-{}-{}",
            std::process::id(),
            UNIQUE_FILE_COUNTER.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&dir).unwrap();
        let destination = dir.join("Tournament Core.sav");
        std::fs::write(&destination, &source_bytes).unwrap();

        let mut instructions = Vec::new();
        for (index, (name, (bytes, _))) in selected.iter().zip(payloads).enumerate() {
            let path = dir.join(format!("tag-{index}.r2tag"));
            std::fs::write(&path, bytes).unwrap();
            instructions.push(ImportInstruction {
                path: path.to_string_lossy().to_string(),
                tag_name: name.clone(),
                overwrite: true,
            });
        }

        let destination_path = destination.to_string_lossy().to_string();
        let result = tauri::async_runtime::block_on(import_tags(
            destination_path.clone(),
            instructions,
            ImportMode::ReplaceCustom,
        ))
        .unwrap();
        assert_eq!(result.imported, selected);
        assert_eq!(result.removed, source_names);
        assert!(result.skipped.is_empty());
        assert!(result.incompatible.is_empty());

        let backup_path = result
            .backup_path
            .expect("replacement did not create a backup");
        assert_eq!(std::fs::read(&backup_path).unwrap(), source_bytes);
        assert_eq!(std::fs::read(&source_path).unwrap(), source_bytes);

        let replaced = read_save(&destination_path).unwrap();
        assert_eq!(custom_tag_names(&replaced).unwrap(), selected);
        let built_ins = |save: &Save| {
            tag_array(save)
                .unwrap()
                .iter()
                .filter(|value| tag_name_of(value).is_some_and(|name| !is_custom_tag(name)))
                .cloned()
                .collect::<Vec<_>>()
        };
        assert_eq!(built_ins(&replaced), built_ins(&source_save));
        assert_eq!(replaced.header, source_save.header);
        assert_eq!(replaced.schemas, source_save.schemas);
        assert_eq!(replaced.extra, source_save.extra);
        let mut replaced_properties = replaced.root.properties.0.clone();
        let mut source_properties = source_save.root.properties.0.clone();
        replaced_properties.shift_remove(&PropertyKey::from("SavedPlayerTags"));
        source_properties.shift_remove(&PropertyKey::from("SavedPlayerTags"));
        assert_eq!(replaced_properties, source_properties);

        std::fs::remove_dir_all(dir).unwrap();
    }
}
