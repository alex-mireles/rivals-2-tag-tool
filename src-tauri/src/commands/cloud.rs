use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::Manager;
use url::Url;

use super::tags::single_tag_bytes;

const MAX_COMPRESSED_BYTES: usize = 2 * 1024 * 1024;
const MAX_UNCOMPRESSED_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CloudTagMetadata {
    pub startgg_user_id: String,
    pub startgg_slug: String,
    pub gamer_tag: String,
    pub tag_name: String,
    pub save_version: Option<i32>,
    pub uncompressed_sha256: String,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthRequest {
    pub request_id: String,
    pub poll_token: String,
    pub authorization_url: String,
    pub expires_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TournamentTagPage {
    pub tournament_name: String,
    pub tournament_slug: String,
    pub event_names: Vec<String>,
    pub page: u32,
    pub total_pages: u32,
    pub total_entrants: u32,
    pub matches: Vec<CloudTagMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudDownloadRequest {
    pub startgg_user_id: String,
    pub uncompressed_sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CloudTagUpload {
    tag_name: String,
    save_version: Option<i32>,
    compression: &'static str,
    uncompressed_sha256: String,
    compressed_base64: String,
}

fn client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(format!("rivals-2-tag-tool/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())
}

fn api_url(base: &str, path: &str) -> Result<Url, String> {
    let value = format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    let url = Url::parse(&value).map_err(|e| format!("Invalid cloud API URL: {e}"))?;
    if url.scheme() != "https"
        && url.host_str() != Some("localhost")
        && url.host_str() != Some("127.0.0.1")
    {
        return Err("Cloud API must use HTTPS".into());
    }
    Ok(url)
}

async fn checked(response: Response) -> Result<Response, String> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let message = response.text().await.unwrap_or_default();
    let message = message.chars().take(500).collect::<String>();
    Err(format!("Cloud API returned {status}: {message}"))
}

fn bearer(request: reqwest::RequestBuilder, token: &str) -> reqwest::RequestBuilder {
    request.bearer_auth(token)
}

#[tauri::command]
pub async fn cloud_begin_auth(api_base_url: String) -> Result<AuthRequest, String> {
    checked(
        client()?
            .post(api_url(&api_base_url, "v1/auth/requests")?)
            .send()
            .await
            .map_err(|e| e.to_string())?,
    )
    .await?
    .json()
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cloud_poll_auth(
    api_base_url: String,
    request_id: String,
    poll_token: String,
) -> Result<serde_json::Value, String> {
    let response = client()?
        .post(api_url(
            &api_base_url,
            &format!("v1/auth/requests/{request_id}/poll"),
        )?)
        .json(&serde_json::json!({ "pollToken": poll_token }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    checked(response)
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cloud_end_session(api_base_url: String, session_token: String) -> Result<(), String> {
    let response = bearer(
        client()?.delete(api_url(&api_base_url, "v1/session")?),
        &session_token,
    )
    .send()
    .await
    .map_err(|e| e.to_string())?;
    checked(response).await?;
    Ok(())
}

#[tauri::command]
pub async fn cloud_search_tags(
    api_base_url: String,
    query: String,
) -> Result<Vec<CloudTagMetadata>, String> {
    let mut url = api_url(&api_base_url, "v1/tags")?;
    url.query_pairs_mut().append_pair("query", &query);
    checked(client()?.get(url).send().await.map_err(|e| e.to_string())?)
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cloud_tournament_tags(
    api_base_url: String,
    slug: String,
    page: u32,
) -> Result<TournamentTagPage, String> {
    let mut url = api_url(&api_base_url, "v1/tournaments/tags")?;
    url.query_pairs_mut()
        .append_pair("slug", &slug)
        .append_pair("page", &page.max(1).to_string());
    checked(client()?.get(url).send().await.map_err(|e| e.to_string())?)
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cloud_upload_tag(
    api_base_url: String,
    session_token: String,
    save_path: String,
    tag_name: String,
) -> Result<CloudTagMetadata, String> {
    let (bytes, save_version) = tauri::async_runtime::spawn_blocking({
        let save_path = save_path.clone();
        let tag_name = tag_name.clone();
        move || single_tag_bytes(&save_path, &tag_name)
    })
    .await
    .map_err(|e| e.to_string())??;

    if bytes.is_empty() || bytes.len() > MAX_UNCOMPRESSED_BYTES {
        return Err("Tag file exceeds the cloud upload size limit".into());
    }
    let uncompressed_sha256 = format!("{:x}", Sha256::digest(&bytes));
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&bytes).map_err(|e| e.to_string())?;
    let compressed = encoder.finish().map_err(|e| e.to_string())?;
    if compressed.len() > MAX_COMPRESSED_BYTES {
        return Err("Compressed tag exceeds the cloud upload size limit".into());
    }

    let body = CloudTagUpload {
        tag_name,
        save_version,
        compression: "gzip",
        uncompressed_sha256,
        compressed_base64: BASE64.encode(compressed),
    };
    let response = bearer(
        client()?.put(api_url(&api_base_url, "v1/me/tag")?),
        &session_token,
    )
    .json(&body)
    .send()
    .await
    .map_err(|e| e.to_string())?;
    checked(response)
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cloud_delete_tag(api_base_url: String, session_token: String) -> Result<(), String> {
    let response = bearer(
        client()?.delete(api_url(&api_base_url, "v1/me/tag")?),
        &session_token,
    )
    .send()
    .await
    .map_err(|e| e.to_string())?;
    checked(response).await?;
    Ok(())
}

fn safe_file_component(value: &str) -> String {
    let value: String = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    if value.is_empty() {
        "tag".into()
    } else {
        value
    }
}

fn decode_cloud_payload_with_limits(
    compressed: &[u8],
    expected_hash: &str,
    max_compressed: usize,
    max_uncompressed: usize,
) -> Result<Vec<u8>, String> {
    if compressed.is_empty() || compressed.len() > max_compressed {
        return Err("Downloaded tag exceeds the compressed size limit".into());
    }
    let mut decoder = GzDecoder::new(compressed).take((max_uncompressed + 1) as u64);
    let mut bytes = Vec::new();
    decoder
        .read_to_end(&mut bytes)
        .map_err(|_| "Downloaded tag is not valid gzip data".to_string())?;
    if bytes.is_empty() || bytes.len() > max_uncompressed {
        return Err("Downloaded tag exceeds the uncompressed size limit".into());
    }
    let actual_hash = format!("{:x}", Sha256::digest(&bytes));
    if actual_hash != expected_hash.to_ascii_lowercase() {
        return Err("Downloaded tag failed its integrity check".into());
    }
    Ok(bytes)
}

#[tauri::command]
pub async fn cloud_download_tags(
    app: tauri::AppHandle,
    api_base_url: String,
    tags: Vec<CloudDownloadRequest>,
) -> Result<Vec<String>, String> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("cloud-tags");
    fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    let http = client()?;
    let mut paths = Vec::with_capacity(tags.len());

    for tag in tags {
        let response = checked(
            http.get(api_url(
                &api_base_url,
                &format!("v1/tags/{}/download", tag.startgg_user_id),
            )?)
            .send()
            .await
            .map_err(|e| e.to_string())?,
        )
        .await?;
        let compressed = response.bytes().await.map_err(|e| e.to_string())?;
        let bytes = decode_cloud_payload_with_limits(
            &compressed,
            &tag.uncompressed_sha256,
            MAX_COMPRESSED_BYTES,
            MAX_UNCOMPRESSED_BYTES,
        )?;
        let actual_hash = format!("{:x}", Sha256::digest(&bytes));

        let file_name = format!(
            "{}-{}.r2tag",
            safe_file_component(&tag.startgg_user_id),
            &actual_hash[..12]
        );
        let path = cache_dir.join(file_name);
        fs::write(&path, bytes).map_err(|e| e.to_string())?;
        paths.push(path.to_string_lossy().to_string());
    }
    Ok(paths)
}

fn remove_if_in_cache(cache_dir: &Path, path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let root = cache_dir.canonicalize().map_err(|e| e.to_string())?;
    let target = path.canonicalize().map_err(|e| e.to_string())?;
    if !target.starts_with(&root) {
        return Err("Refusing to remove a file outside the cloud cache".into());
    }
    fs::remove_file(target).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cleanup_cloud_files(app: tauri::AppHandle, paths: Vec<String>) -> Result<(), String> {
    let cache_dir: PathBuf = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("cloud-tags");
    if !cache_dir.exists() {
        return Ok(());
    }
    for path in paths {
        remove_if_in_cache(&cache_dir, Path::new(&path))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn cleanup_stale_cloud_files(app: tauri::AppHandle) -> Result<(), String> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("cloud-tags");
    if !cache_dir.exists() {
        return Ok(());
    }
    let cutoff = Duration::from_secs(24 * 60 * 60);
    for entry in fs::read_dir(&cache_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        if metadata.is_file()
            && SystemTime::now()
                .duration_since(metadata.modified().map_err(|e| e.to_string())?)
                .unwrap_or_default()
                > cutoff
        {
            remove_if_in_cache(&cache_dir, &entry.path())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gzip_round_trip_preserves_bytes_and_hash() {
        let source = b"single tag payload";
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(source).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut decoded = Vec::new();
        GzDecoder::new(compressed.as_slice())
            .read_to_end(&mut decoded)
            .unwrap();
        assert_eq!(decoded, source);
        assert_eq!(
            format!("{:x}", Sha256::digest(&decoded)),
            format!("{:x}", Sha256::digest(source))
        );
    }

    #[test]
    fn file_components_drop_path_characters() {
        assert_eq!(safe_file_component("../../user:42"), "user42");
        assert_eq!(safe_file_component(""), "tag");
    }

    #[test]
    fn download_validation_rejects_corruption_hash_mismatch_and_expansion() {
        assert!(decode_cloud_payload_with_limits(b"not gzip", &"0".repeat(64), 128, 128).is_err());

        let source = b"payload";
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(source).unwrap();
        let compressed = encoder.finish().unwrap();
        assert!(decode_cloud_payload_with_limits(&compressed, &"0".repeat(64), 128, 128).is_err());
        assert!(decode_cloud_payload_with_limits(
            &compressed,
            &format!("{:x}", Sha256::digest(source)),
            128,
            3,
        )
        .is_err());
    }
}
