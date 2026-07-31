//! Talking to the tag-sharing website (jugeeya.github.io):
//!   - share_tags_to_site: zip selected tags and POST them to the broker, which
//!     opens a PR that auto-merges them onto the site.
//!   - fetch_shared_tags: read the site's published tag manifest.
//!   - download_tags: download published tag zips and unpack the .r2tag inside,
//!     ready to feed into the existing import flow.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

use super::tags::{sanitize_file_stem, single_tag_save_bytes, StartggLink};

const BROKER_URL: &str = "https://r2tag-broker.jdsambasivam.workers.dev";
const SITE_INDEX_URL: &str = "https://jugeeya.github.io/tags/data/index.json";
const SITE_DATA_BASE: &str = "https://jugeeya.github.io/tags/data";
const USER_AGENT: &str = "rivals-2-tag-tool";

fn str_at(v: &serde_json::Value, ptr: &str) -> String {
    v.pointer(ptr).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

// ---- Share to site --------------------------------------------------------

/// One tag to share, with its own start.gg link.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareItem {
    pub tag_name: String,
    pub startgg: StartggLink,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareResult {
    /// URL of the opened pull request.
    pub pr: String,
    pub number: u64,
}

/// Build an in-memory zip containing a single `<stem>.r2tag`.
fn zip_single_tag(stem: &str, r2tag_bytes: &[u8]) -> Result<Vec<u8>, String> {
    use zip::write::SimpleFileOptions;
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut zw = zip::ZipWriter::new(&mut cursor);
        zw.start_file(format!("{stem}.r2tag"), SimpleFileOptions::default())
            .map_err(|e| e.to_string())?;
        zw.write_all(r2tag_bytes).map_err(|e| e.to_string())?;
        zw.finish().map_err(|e| e.to_string())?;
    }
    Ok(cursor.into_inner())
}

/// Zip each selected tag and upload them to the broker in one submission, each
/// carrying its own start.gg link. No files are written locally.
#[tauri::command]
pub async fn share_tags_to_site(
    save_path: String,
    items: Vec<ShareItem>,
) -> Result<ShareResult, String> {
    if items.is_empty() {
        return Err("No tags selected to share.".into());
    }

    // Reading + parsing the save and zipping is blocking work; do it off the
    // async runtime, then upload.
    let built = tauri::async_runtime::spawn_blocking(
        move || -> Result<Vec<(String, Vec<u8>, StartggLink)>, String> {
            let mut out = Vec::new();
            for item in items {
                let bytes = single_tag_save_bytes(&save_path, &item.tag_name)?;
                let stem = sanitize_file_stem(&item.tag_name);
                let zip = zip_single_tag(&stem, &bytes)?;
                out.push((format!("{stem}.r2tag.zip"), zip, item.startgg));
            }
            Ok(out)
        },
    )
    .await
    .map_err(|e| e.to_string())??;

    let mut form = reqwest::multipart::Form::new().text("author", "");
    for (file_name, zip, link) in built {
        let part = reqwest::multipart::Part::bytes(zip)
            .file_name(file_name)
            .mime_str("application/zip")
            .map_err(|e| e.to_string())?;
        form = form
            .part("tags", part)
            .text("startgg_slug", link.slug)
            .text("startgg_tag", link.tag);
    }

    let client = reqwest::Client::new();
    let res = client
        .post(BROKER_URL)
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = res.status();
    let body: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        let msg = body
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("submission failed");
        return Err(msg.to_string());
    }
    Ok(ShareResult {
        pr: str_at(&body, "/pr"),
        number: body.get("number").and_then(|v| v.as_u64()).unwrap_or(0),
    })
}

// ---- Browse + download ----------------------------------------------------

/// A published tag from the site manifest.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedTag {
    pub name: String,
    pub author: String,
    /// Manifest file name, e.g. `kim-e85def91.r2tag.zip`.
    pub file: String,
    pub startgg_slug: String,
    pub startgg_tag: String,
}

/// Read the site's published tag manifest (index.json).
#[tauri::command]
pub async fn fetch_shared_tags() -> Result<Vec<SharedTag>, String> {
    let client = reqwest::Client::new();
    let res = client
        .get(SITE_INDEX_URL)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("Could not load shared tags ({}).", res.status()));
    }
    let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    if let Some(tags) = data.get("tags").and_then(|v| v.as_array()) {
        for t in tags {
            out.push(SharedTag {
                name: str_at(t, "/name"),
                author: str_at(t, "/author"),
                file: str_at(t, "/file"),
                startgg_slug: str_at(t, "/startgg/slug"),
                startgg_tag: str_at(t, "/startgg/tag"),
            });
        }
    }
    Ok(out)
}

fn extract_single_r2tag(zip_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_file() && entry.name().to_lowercase().ends_with(".r2tag") {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            return Ok(buf);
        }
    }
    Err("downloaded zip did not contain a .r2tag".into())
}

/// Download the named published tag zips, unpack the `.r2tag` inside each, and
/// return the extracted `.r2tag` paths. `files` are manifest file names like
/// `kim-e85def91.r2tag.zip`. With `dest_dir` set, the files are written there
/// (download-to-disk); otherwise they go to a temp dir (to feed the import flow).
#[tauri::command]
pub async fn download_tags(
    files: Vec<String>,
    dest_dir: Option<String>,
) -> Result<Vec<String>, String> {
    if files.is_empty() {
        return Err("No tags selected to download.".into());
    }

    let dir = match dest_dir {
        Some(d) => std::path::PathBuf::from(d),
        None => std::env::temp_dir().join("rivals-2-tag-tool"),
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let client = reqwest::Client::new();
    let mut written = Vec::new();
    for file in files {
        // Only fetch plain manifest file names — no path traversal.
        if file.contains('/')
            || file.contains('\\')
            || file.contains("..")
            || !file.ends_with(".r2tag.zip")
        {
            return Err(format!("Unexpected tag file name: {file}"));
        }

        let url = format!("{SITE_DATA_BASE}/{file}");
        let res = client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            return Err(format!("Download failed for {file} ({}).", res.status()));
        }
        let bytes = res.bytes().await.map_err(|e| e.to_string())?;
        let r2tag = extract_single_r2tag(&bytes)?;

        let out_name = file.trim_end_matches(".zip"); // <stem>.r2tag
        let out_path = dir.join(out_name);
        std::fs::write(&out_path, &r2tag).map_err(|e| e.to_string())?;
        written.push(out_path.to_string_lossy().to_string());
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::{extract_single_r2tag, zip_single_tag};

    #[test]
    fn zip_then_extract_round_trips() {
        let payload = b"GVAS fake save bytes for the test".to_vec();
        let zip = zip_single_tag("my-tag", &payload).unwrap();
        // Real zip archives start with "PK".
        assert_eq!(&zip[0..2], b"PK");
        let back = extract_single_r2tag(&zip).unwrap();
        assert_eq!(back, payload);
    }

    #[test]
    fn extract_rejects_zip_without_r2tag() {
        use std::io::Write;
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut zw = zip::ZipWriter::new(&mut cursor);
            zw.start_file("readme.txt", zip::write::SimpleFileOptions::default()).unwrap();
            zw.write_all(b"nope").unwrap();
            zw.finish().unwrap();
        }
        assert!(extract_single_r2tag(&cursor.into_inner()).is_err());
    }
}
