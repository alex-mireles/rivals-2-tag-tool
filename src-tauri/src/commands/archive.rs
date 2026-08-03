//! `.r2pack` tag archives.
//!
//! A `.r2pack` is a zip holding a `manifest.json` plus one `.r2tag` per tag. It
//! exists for the tournament-organiser workflow: download a whole bracket's
//! tags once on any machine, carry the pack around on a USB stick, and import
//! it onto each setup in a few clicks.
//!
//! Packs travel on removable media between machines the app does not control,
//! so unpacking treats every archive as hostile. Most importantly, on-disk
//! names are derived entirely from data we generate — the names inside the
//! archive are never joined onto a path, which removes zip-slip as a category
//! rather than checking for it.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::AppHandle;
use uesave::Save;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::cloud::{staging_dir, MAX_UNCOMPRESSED_BYTES};
use super::tags::{first_tag_name, save_version, single_tag_bytes, unique_file_stem};

/// Bumped only for changes an older build must refuse to read.
const PACK_FORMAT_VERSION: u32 = 1;
const MANIFEST_NAME: &str = "manifest.json";
const TAG_EXTENSION: &str = ".r2tag";

/// Caps applied while unpacking. The per-entry cap mirrors the cloud download
/// limit; the total is the actual zip-bomb defense, since 512 x 8 MiB would
/// otherwise be 4 GiB.
const MAX_PACK_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PACK_ENTRIES: usize = 512;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_PACK_TOTAL_BYTES: u64 = 128 * 1024 * 1024;

// ---------------------------------------------------------------- manifest

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackManifest {
    /// Integer, not semver. A pack from a future format is refused outright
    /// rather than half-understood.
    pub format_version: u32,
    #[serde(default)]
    pub app: String,
    #[serde(default)]
    pub app_version: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Common save version across all entries, or `None` when they disagree.
    /// Lets the UI warn about a version mismatch before listing every tag.
    #[serde(default)]
    pub save_version: Option<i32>,
    #[serde(default)]
    pub entries: Vec<PackManifestEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackManifestEntry {
    /// Correlation key against the zip entry's own name. Never a path.
    pub file: String,
    #[serde(default)]
    pub tag_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gamer_tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startgg_slug: Option<String>,
    #[serde(default)]
    pub save_version: Option<i32>,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub size: u64,
}

// ------------------------------------------------------------------ packing

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackEntryInput {
    pub path: String,
    pub gamer_tag: Option<String>,
    pub startgg_slug: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackSummary {
    pub output_path: String,
    pub entry_count: usize,
    pub bytes: u64,
    /// Entry stems written into the archive, in order.
    pub names: Vec<String>,
}

struct PackSource {
    bytes: Vec<u8>,
    /// Preferred human name for the file inside the archive.
    display_name: String,
    tag_name: String,
    gamer_tag: Option<String>,
    startgg_slug: Option<String>,
    save_version: Option<i32>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Days-since-epoch to a civil date (Howard Hinnant's algorithm), so one
/// cosmetic manifest field doesn't require a date crate.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn format_rfc3339(unix_secs: u64) -> String {
    let (year, month, day) = civil_from_days((unix_secs / 86_400) as i64);
    let rem = unix_secs % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// Serialize sources into a `.r2pack` at `output_path`.
///
/// Writes to a `.part` sibling and renames, so a USB stick running out of room
/// leaves no valid-looking half archive.
fn write_pack(
    sources: Vec<PackSource>,
    output_path: &str,
    label: Option<String>,
    source: Option<String>,
) -> Result<PackSummary, String> {
    if sources.is_empty() {
        return Err("Select at least one tag to pack".into());
    }

    let mut used_stems = HashSet::new();
    let mut entries = Vec::with_capacity(sources.len());
    let mut named = Vec::with_capacity(sources.len());

    for item in sources {
        let stem = unique_file_stem(&item.display_name, &mut used_stems);
        let file = format!("{stem}{TAG_EXTENSION}");
        entries.push(PackManifestEntry {
            file: file.clone(),
            tag_name: item.tag_name,
            gamer_tag: item.gamer_tag,
            startgg_slug: item.startgg_slug,
            save_version: item.save_version,
            sha256: sha256_hex(&item.bytes),
            size: item.bytes.len() as u64,
        });
        named.push((file, stem, item.bytes));
    }

    // Only a version every entry agrees on is meaningful as a pack-wide target.
    let mut versions = entries.iter().map(|entry| entry.save_version);
    let first = versions.next().flatten();
    let common = if first.is_some() && entries.iter().all(|e| e.save_version == first) {
        first
    } else {
        None
    };

    let manifest = PackManifest {
        format_version: PACK_FORMAT_VERSION,
        app: "rivals-2-tag-tool".into(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        created_at: format_rfc3339(now_unix_secs()),
        label,
        source,
        save_version: common,
        entries,
    };
    let manifest_json = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;

    let part_path = PathBuf::from(format!("{output_path}.part"));
    let build = || -> Result<(), String> {
        let file = File::create(&part_path).map_err(|e| e.to_string())?;
        let mut zip = ZipWriter::new(BufWriter::new(file));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        zip.start_file(MANIFEST_NAME, options)
            .map_err(|e| e.to_string())?;
        zip.write_all(&manifest_json).map_err(|e| e.to_string())?;

        for (file_name, _, bytes) in &named {
            zip.start_file(file_name.as_str(), options)
                .map_err(|e| e.to_string())?;
            zip.write_all(bytes).map_err(|e| e.to_string())?;
        }

        let mut writer = zip.finish().map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
        Ok(())
    };

    if let Err(error) = build() {
        let _ = fs::remove_file(&part_path);
        return Err(error);
    }
    if let Err(error) = fs::rename(&part_path, output_path) {
        let _ = fs::remove_file(&part_path);
        return Err(error.to_string());
    }

    let bytes = fs::metadata(output_path).map(|m| m.len()).unwrap_or(0);
    Ok(PackSummary {
        output_path: output_path.to_string(),
        entry_count: named.len(),
        bytes,
        names: named.into_iter().map(|(_, stem, _)| stem).collect(),
    })
}

/// Pack tags straight out of the loaded save — no temp files involved.
#[tauri::command]
pub async fn pack_tags_from_save(
    save_path: String,
    tag_names: Vec<String>,
    output_path: String,
    label: Option<String>,
) -> Result<PackSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut sources = Vec::with_capacity(tag_names.len());
        for tag_name in tag_names {
            let (bytes, save_version) = single_tag_bytes(&save_path, &tag_name)?;
            sources.push(PackSource {
                bytes,
                display_name: tag_name.clone(),
                tag_name,
                gamer_tag: None,
                startgg_slug: None,
                save_version,
            });
        }
        write_pack(sources, &output_path, label, None)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Pack already-staged `.r2tag` files (cloud downloads) into one archive.
#[tauri::command]
pub async fn pack_tag_files(
    entries: Vec<PackEntryInput>,
    output_path: String,
    label: Option<String>,
    source: Option<String>,
) -> Result<PackSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut sources = Vec::with_capacity(entries.len());
        for entry in entries {
            let bytes = fs::read(&entry.path).map_err(|e| e.to_string())?;
            let save =
                Save::read(&mut Cursor::new(&bytes)).map_err(|error| format!("{}", error))?;
            let tag_name = first_tag_name(&save)
                .ok_or_else(|| format!("{}: no player tag found", entry.path))?
                .to_string();

            sources.push(PackSource {
                // A TO recognises the start.gg handle, not the in-game tag.
                display_name: entry.gamer_tag.clone().unwrap_or_else(|| tag_name.clone()),
                tag_name,
                gamer_tag: entry.gamer_tag,
                startgg_slug: entry.startgg_slug,
                save_version: save_version(&save),
                bytes,
            });
        }
        write_pack(sources, &output_path, label, source)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- unpacking

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UnpackResult {
    /// Staged `.r2tag` paths, ready to hand to `get_tag_previews`.
    pub paths: Vec<String>,
    pub label: Option<String>,
    pub source: Option<String>,
    pub created_at: Option<String>,
    /// Pack-wide target save version, when the manifest declared one.
    pub declared_save_version: Option<i32>,
    /// False when the manifest was absent or unreadable and tag details were
    /// read from the `.r2tag` files directly.
    pub manifest_ok: bool,
    pub entry_count: usize,
    /// Entries dropped for failing validation, by display name.
    pub skipped: Vec<String>,
}

/// Read and validate `manifest.json`, if present.
///
/// A missing or corrupt manifest is *not* fatal: each `.r2tag` is a complete
/// save carrying its own tag name and version, so the manifest is a
/// convenience. Failing here would break a pack someone innocently re-zipped
/// with another tool, for no safety gain. A pack from a future format version
/// is the one hard failure, because that is an explicit signal that reading it
/// as v1 would misinterpret the contents.
fn read_manifest<R: Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
) -> Result<Option<PackManifest>, String> {
    let mut entry = match zip.by_name(MANIFEST_NAME) {
        Ok(entry) => entry,
        Err(_) => return Ok(None),
    };

    let mut bytes = Vec::new();
    if entry
        .by_ref()
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 > MAX_MANIFEST_BYTES
    {
        return Ok(None);
    }

    let manifest: PackManifest = match serde_json::from_slice(&bytes) {
        Ok(manifest) => manifest,
        Err(_) => return Ok(None),
    };

    if manifest.format_version > PACK_FORMAT_VERSION {
        return Err(format!(
            "This .r2pack was made by a newer version of the tool (format {}). Update to open it.",
            manifest.format_version
        ));
    }

    Ok(Some(manifest))
}

/// A payload is a usable tag if it parses as a save and names a tag.
fn is_valid_tag_payload(bytes: &[u8]) -> bool {
    Save::read(&mut Cursor::new(bytes))
        .ok()
        .map(|save| first_tag_name(&save).is_some())
        .unwrap_or(false)
}

/// Extraction, parameterised over payload validation. `uesave` types cannot be
/// constructed outside their crate, so tests substitute a trivial validator to
/// exercise the archive hardening without hand-encoding GVAS fixtures.
fn unpack_with(
    staging: &Path,
    archive_path: &str,
    is_valid: impl Fn(&[u8]) -> bool,
) -> Result<UnpackResult, String> {
    // Reject an oversized archive before allocating anything for it.
    let file_size = fs::metadata(archive_path)
        .map_err(|e| e.to_string())?
        .len();
    if file_size > MAX_PACK_FILE_BYTES {
        return Err("This .r2pack is too large to open".into());
    }

    let file = File::open(archive_path).map_err(|e| e.to_string())?;
    let mut zip = ZipArchive::new(BufReader::new(file))
        .map_err(|_| "This file is not a readable .r2pack archive".to_string())?;

    if zip.len() > MAX_PACK_ENTRIES {
        return Err(format!(
            "This .r2pack holds more than {MAX_PACK_ENTRIES} files"
        ));
    }

    let manifest = read_manifest(&mut zip)?;
    let by_file: HashMap<String, &PackManifestEntry> = manifest
        .as_ref()
        .map(|m| {
            m.entries
                .iter()
                .map(|entry| (entry.file.clone(), entry))
                .collect()
        })
        .unwrap_or_default();

    // Distinguishes concurrent unpacks of the same pack, whose staged files
    // would otherwise collide and let one session delete the other's inputs.
    let run_token = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();

    let mut written: Vec<PathBuf> = Vec::new();
    let mut paths = Vec::new();
    let mut skipped = Vec::new();
    let mut total_bytes: u64 = 0;
    let mut tag_entries = 0usize;

    let mut extract = || -> Result<(), String> {
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index).map_err(|e| e.to_string())?;
            if !entry.is_file() {
                continue;
            }

            let name = entry.name().to_string();
            if name == MANIFEST_NAME {
                continue;
            }
            // Anything else riding along (README, .DS_Store) is ignored quietly.
            if !name.to_ascii_lowercase().ends_with(TAG_EXTENSION) {
                continue;
            }

            tag_entries += 1;
            if tag_entries > MAX_PACK_ENTRIES {
                return Err(format!(
                    "This .r2pack holds more than {MAX_PACK_ENTRIES} tags"
                ));
            }
            if entry.encrypted() {
                return Err("This .r2pack is encrypted and cannot be opened".into());
            }
            if !matches!(
                entry.compression(),
                CompressionMethod::Stored | CompressionMethod::Deflated
            ) {
                return Err("This .r2pack uses an unsupported compression method".into());
            }

            // The header's declared size is attacker-controlled, so the read is
            // capped independently of `entry.size()`.
            let mut bytes = Vec::new();
            entry
                .by_ref()
                .take(MAX_UNCOMPRESSED_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > MAX_UNCOMPRESSED_BYTES {
                return Err(format!("{name}: tag file exceeds the size limit"));
            }

            total_bytes += bytes.len() as u64;
            if total_bytes > MAX_PACK_TOTAL_BYTES {
                return Err("This .r2pack expands to more data than the tool will open".into());
            }

            let meta = by_file.get(&name);
            let display = meta
                .and_then(|m| m.gamer_tag.clone())
                .unwrap_or_else(|| name.trim_end_matches(TAG_EXTENSION).to_string());

            // Validate before writing: a truncated USB copy becomes one skipped
            // row rather than a file the import step later chokes on.
            if !is_valid(&bytes) {
                skipped.push(display);
                continue;
            }

            let hash = sha256_hex(&bytes);
            if let Some(expected) = meta.map(|m| m.sha256.as_str()).filter(|s| !s.is_empty()) {
                if hash != expected.to_ascii_lowercase() {
                    skipped.push(display);
                    continue;
                }
            }

            // On-disk name comes only from values we generate. The archive's
            // own name is never joined onto a path, so a traversal entry like
            // `../../evil.r2tag` simply lands in staging under our name.
            let path = staging.join(format!("{run_token}-{index:04}-{}.r2tag", &hash[..12]));
            fs::write(&path, &bytes).map_err(|e| e.to_string())?;
            written.push(path.clone());
            paths.push(path.to_string_lossy().to_string());
        }
        Ok(())
    };

    if let Err(error) = extract() {
        // Don't leak partial extractions until the 24h sweeper runs.
        for path in &written {
            let _ = fs::remove_file(path);
        }
        return Err(error);
    }

    if paths.is_empty() && skipped.is_empty() {
        return Err("This .r2pack contains no tag files".into());
    }

    let manifest_ok = manifest.is_some();
    Ok(UnpackResult {
        entry_count: paths.len(),
        paths,
        label: manifest.as_ref().and_then(|m| m.label.clone()),
        source: manifest.as_ref().and_then(|m| m.source.clone()),
        created_at: manifest
            .as_ref()
            .map(|m| m.created_at.clone())
            .filter(|s| !s.is_empty()),
        declared_save_version: manifest.as_ref().and_then(|m| m.save_version),
        manifest_ok,
        skipped,
    })
}

/// Extract a `.r2pack` into the staging cache and return the staged paths.
#[tauri::command]
pub async fn unpack_r2pack(
    app: AppHandle,
    archive_path: String,
) -> Result<UnpackResult, String> {
    let staging = staging_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
        unpack_with(&staging, &archive_path, is_valid_tag_payload)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stands in for a `.r2tag` payload. Real saves cannot be built here (see
    /// `unpack_with`), so tests use a marker the substitute validator accepts.
    fn sample_save_bytes(tag: &str) -> Vec<u8> {
        format!("TAG:{tag}").into_bytes()
    }

    /// Substitute for `is_valid_tag_payload` — accepts the marker above.
    fn accepts_marker(bytes: &[u8]) -> bool {
        bytes.starts_with(b"TAG:")
    }

    fn unpack(staging: &Path, archive_path: &str) -> Result<UnpackResult, String> {
        unpack_with(staging, archive_path, accepts_marker)
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("r2tt-archive-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn pack_sources(tags: &[&str]) -> Vec<PackSource> {
        tags.iter()
            .map(|tag| PackSource {
                bytes: sample_save_bytes(tag),
                display_name: (*tag).to_string(),
                tag_name: (*tag).to_string(),
                gamer_tag: Some(format!("{tag}Player")),
                startgg_slug: Some(format!("user/{tag}")),
                save_version: Some(13),
            })
            .collect()
    }

    #[test]
    fn rfc3339_formatting_matches_known_timestamps() {
        assert_eq!(format_rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_rfc3339(1_735_689_600), "2025-01-01T00:00:00Z");
        // Leap day, to exercise the civil-date conversion.
        assert_eq!(format_rfc3339(1_709_209_845), "2024-02-29T12:30:45Z");
    }

    #[test]
    fn pack_round_trips_bytes_and_metadata() {
        let dir = temp_dir("round-trip");
        let out = dir.join("pack.r2pack");
        let out_str = out.to_string_lossy().to_string();
        let original = sample_save_bytes("HYPER");

        let summary = write_pack(
            pack_sources(&["HYPER", "ZETTER"]),
            &out_str,
            Some("Genesis X2".into()),
            Some("tournament/genesis-x2".into()),
        )
        .unwrap();
        assert_eq!(summary.entry_count, 2);
        assert!(!out.with_extension("r2pack.part").exists(), "temp left behind");

        let staging = dir.join("staging");
        fs::create_dir_all(&staging).unwrap();
        let result = unpack(&staging, &out_str).unwrap();

        assert!(result.manifest_ok);
        assert_eq!(result.entry_count, 2);
        assert!(result.skipped.is_empty());
        assert_eq!(result.label.as_deref(), Some("Genesis X2"));
        assert_eq!(result.declared_save_version, Some(13));
        // Byte-identical, which is what makes an imported tag match the original.
        assert_eq!(fs::read(&result.paths[0]).unwrap(), original);
    }

    #[test]
    fn empty_selection_is_refused() {
        let dir = temp_dir("empty");
        let out = dir.join("empty.r2pack").to_string_lossy().to_string();
        assert!(write_pack(Vec::new(), &out, None, None).is_err());
        assert!(!Path::new(&out).exists(), "no archive should be created");
    }

    /// A pack re-zipped by another tool loses the manifest; tags must survive.
    #[test]
    fn missing_manifest_falls_back_to_reading_entries() {
        let dir = temp_dir("no-manifest");
        let out = dir.join("bare.r2pack");
        let out_str = out.to_string_lossy().to_string();

        let file = File::create(&out).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file("HYPER.r2tag", options).unwrap();
        zip.write_all(&sample_save_bytes("HYPER")).unwrap();
        zip.finish().unwrap();

        let staging = dir.join("staging");
        fs::create_dir_all(&staging).unwrap();
        let result = unpack(&staging, &out_str).unwrap();

        assert!(!result.manifest_ok, "should report the manifest was absent");
        assert_eq!(result.entry_count, 1);
    }

    #[test]
    fn future_format_version_is_refused() {
        let dir = temp_dir("future");
        let out = dir.join("future.r2pack");
        let out_str = out.to_string_lossy().to_string();

        let file = File::create(&out).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file(MANIFEST_NAME, options).unwrap();
        zip.write_all(br#"{"formatVersion":2,"entries":[]}"#).unwrap();
        zip.start_file("HYPER.r2tag", options).unwrap();
        zip.write_all(&sample_save_bytes("HYPER")).unwrap();
        zip.finish().unwrap();

        let staging = dir.join("staging");
        fs::create_dir_all(&staging).unwrap();
        let error = unpack(&staging, &out_str).unwrap_err();
        assert!(error.contains("newer version"), "got {error}");
    }

    #[test]
    fn traversal_entry_names_stay_inside_staging() {
        let dir = temp_dir("zip-slip");
        let out = dir.join("evil.r2pack");
        let out_str = out.to_string_lossy().to_string();

        let file = File::create(&out).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zip.start_file("../../escaped.r2tag", options).unwrap();
        zip.write_all(&sample_save_bytes("HYPER")).unwrap();
        zip.finish().unwrap();

        let staging = dir.join("staging");
        fs::create_dir_all(&staging).unwrap();
        let result = unpack(&staging, &out_str).unwrap();

        assert_eq!(result.paths.len(), 1);
        let written = Path::new(&result.paths[0]).canonicalize().unwrap();
        assert!(
            written.starts_with(staging.canonicalize().unwrap()),
            "escaped staging: {written:?}"
        );
        assert!(!dir.parent().unwrap().join("escaped.r2tag").exists());
    }

    #[test]
    fn oversized_entry_is_rejected_despite_compressing_small() {
        let dir = temp_dir("bomb");
        let out = dir.join("bomb.r2pack");
        let out_str = out.to_string_lossy().to_string();

        let file = File::create(&out).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file("huge.r2tag", options).unwrap();
        // Highly compressible, so the archive stays tiny while the entry does not.
        zip.write_all(&vec![0u8; MAX_UNCOMPRESSED_BYTES + 1024]).unwrap();
        zip.finish().unwrap();

        let staging = dir.join("staging");
        fs::create_dir_all(&staging).unwrap();
        let error = unpack(&staging, &out_str).unwrap_err();
        assert!(error.contains("size limit"), "got {error}");
        // Nothing from the failed run should be left staged.
        assert_eq!(fs::read_dir(&staging).unwrap().count(), 0);
    }

    #[test]
    fn entry_count_cap_is_enforced() {
        let dir = temp_dir("too-many");
        let out = dir.join("many.r2pack");
        let out_str = out.to_string_lossy().to_string();

        let file = File::create(&out).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for index in 0..=MAX_PACK_ENTRIES {
            zip.start_file(format!("tag{index}.r2tag"), options).unwrap();
            zip.write_all(b"x").unwrap();
        }
        zip.finish().unwrap();

        let staging = dir.join("staging");
        fs::create_dir_all(&staging).unwrap();
        assert!(unpack(&staging, &out_str).is_err());
    }

    #[test]
    fn checksum_mismatch_skips_the_entry_without_failing_the_pack() {
        let dir = temp_dir("checksum");
        let out = dir.join("tampered.r2pack");
        let out_str = out.to_string_lossy().to_string();

        let good = sample_save_bytes("GOOD");
        let tampered = sample_save_bytes("BAD");
        let manifest = serde_json::json!({
            "formatVersion": 1,
            "entries": [
                { "file": "GOOD.r2tag", "tagName": "GOOD", "gamerTag": "GoodPlayer",
                  "sha256": sha256_hex(&good), "size": good.len() },
                // Deliberately wrong hash, as a truncated USB copy would produce.
                { "file": "BAD.r2tag", "tagName": "BAD", "gamerTag": "BadPlayer",
                  "sha256": "0".repeat(64), "size": tampered.len() }
            ]
        });

        let file = File::create(&out).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file(MANIFEST_NAME, options).unwrap();
        zip.write_all(&serde_json::to_vec(&manifest).unwrap()).unwrap();
        zip.start_file("GOOD.r2tag", options).unwrap();
        zip.write_all(&good).unwrap();
        zip.start_file("BAD.r2tag", options).unwrap();
        zip.write_all(&tampered).unwrap();
        zip.finish().unwrap();

        let staging = dir.join("staging");
        fs::create_dir_all(&staging).unwrap();
        let result = unpack(&staging, &out_str).unwrap();

        assert_eq!(result.entry_count, 1, "the good tag still imports");
        assert_eq!(result.skipped, vec!["BadPlayer".to_string()]);
    }

    #[test]
    fn unparseable_payload_is_skipped_not_fatal() {
        let dir = temp_dir("corrupt-entry");
        let out = dir.join("corrupt.r2pack");
        let out_str = out.to_string_lossy().to_string();

        let file = File::create(&out).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file("GOOD.r2tag", options).unwrap();
        zip.write_all(&sample_save_bytes("GOOD")).unwrap();
        zip.start_file("TRUNCATED.r2tag", options).unwrap();
        zip.write_all(b"not a save at all").unwrap();
        zip.finish().unwrap();

        let staging = dir.join("staging");
        fs::create_dir_all(&staging).unwrap();
        let result = unpack(&staging, &out_str).unwrap();

        assert_eq!(result.entry_count, 1, "the readable tag still imports");
        assert_eq!(result.skipped, vec!["TRUNCATED".to_string()]);
    }

    #[test]
    fn real_validator_rejects_non_save_bytes() {
        // Guards the production validator that the tests above substitute out.
        assert!(!is_valid_tag_payload(b"not a save"));
        assert!(!is_valid_tag_payload(b""));
    }

    #[test]
    fn colliding_display_names_get_distinct_entries() {
        let dir = temp_dir("collide");
        let out = dir.join("collide.r2pack");
        let out_str = out.to_string_lossy().to_string();

        let mut sources = pack_sources(&["A", "B"]);
        // Two players sharing a handle must not overwrite each other.
        sources[0].display_name = "Sam".into();
        sources[1].display_name = "Sam".into();

        let summary = write_pack(sources, &out_str, None, None).unwrap();
        assert_eq!(summary.names, vec!["Sam".to_string(), "Sam (1)".to_string()]);
        assert_eq!(summary.entry_count, 2);
    }
}
