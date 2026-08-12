//! In-app updates.
//!
//! Tauri's own updater works by running an installer, and we ship a single .exe
//! with no installer, so we do the install ourselves. Windows won't let a
//! running program overwrite its own .exe, but it will let you rename it — so
//! the `self-replace` crate renames the running one out of the way and puts the
//! new one in its place.
//!
//! Downloads only ever come from github.com over HTTPS, and the checksum below
//! catches a file that arrived damaged. AGENTS.md explains the reasoning.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

const REPO_OWNER: &str = "alex-mireles";
const REPO_NAME: &str = "rivals-2-tag-tool";

/// The .exe is around 14 MiB, so this is generous. Checked as the download
/// arrives, because we can't trust the size the server claims up front.
const MAX_UPDATE_BYTES: u64 = 64 * 1024 * 1024;

/// The manifest is four short fields; anything this big isn't one.
const MAX_MANIFEST_BYTES: usize = 64 * 1024;

/// Progress event name. The payload is [`DownloadProgress`].
const PROGRESS_EVENT: &str = "update://download-progress";

/// On macOS the app only tells the user an update exists. Windows ships one
/// .exe we can swap; the Mac build is a folder of files, which needs more work.
const SELF_INSTALL_SUPPORTED: bool = cfg!(windows);

/// What the release CDN serves at `latest-<os>-<arch>.json`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateManifest {
    version: String,
    url: String,
    /// Lowercase hex SHA-256 of the executable at `url`.
    sha256: String,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    pub_date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
    pub pub_date: Option<String>,
    /// False when the app can only point at the download page: an unsupported
    /// platform, or an install directory this user cannot write to.
    pub can_self_install: bool,
    /// Where to send the user when `can_self_install` is false.
    pub release_page_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    downloaded: u64,
    /// `None` when the server sends no `Content-Length`.
    total: Option<u64>,
}

/// `None` on platforms we don't publish builds for.
fn platform_key() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("windows-x86_64"),
        ("macos", "aarch64") => Some("darwin-aarch64"),
        _ => None,
    }
}

/// GitHub's "latest" link only points at releases you've actually published, so
/// a release still in draft reaches nobody. Reading a file like this also avoids
/// GitHub's API request limit, which a venue full of PCs on one connection
/// could otherwise hit.
fn manifest_url(platform: &str) -> String {
    // Debug builds only. A released app must never let an environment variable
    // change where it downloads from. AGENTS.md explains how to use this.
    #[cfg(debug_assertions)]
    if let Ok(url) = std::env::var("R2_UPDATE_MANIFEST_URL") {
        return url;
    }

    format!(
        "https://github.com/{REPO_OWNER}/{REPO_NAME}/releases/latest/download/latest-{platform}.json"
    )
}

fn release_page_url() -> String {
    format!("https://github.com/{REPO_OWNER}/{REPO_NAME}/releases/latest")
}

fn client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent(format!("rivals-2-tag-tool/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())
}

/// Stops an edited manifest from sending the download somewhere else. Only this
/// first address is checked — GitHub then forwards us to its own download
/// server, which is expected.
fn check_download_url(raw: &str) -> Result<(), String> {
    let url = url::Url::parse(raw).map_err(|e| format!("Invalid update URL: {e}"))?;
    if url.scheme() != "https" {
        return Err("Update URL must use HTTPS".into());
    }
    if url.host_str() != Some("github.com") {
        return Err("Update URL must point at github.com".into());
    }
    Ok(())
}

async fn fetch_manifest(platform: &str) -> Result<UpdateManifest, String> {
    let response = client()?
        .get(manifest_url(platform))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("Update check returned {}", response.status()));
    }

    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err("Update manifest is implausibly large".into());
    }

    serde_json::from_slice(&bytes).map_err(|e| format!("Could not read update manifest: {e}"))
}

/// Only a higher version counts, so deleting a release can't downgrade anyone.
fn is_newer(candidate: &str, current: &str) -> Result<bool, String> {
    let candidate = Version::parse(candidate.trim_start_matches('v'))
        .map_err(|e| format!("Update manifest has an unreadable version: {e}"))?;
    let current =
        Version::parse(current).map_err(|e| format!("This build has an unreadable version: {e}"))?;
    Ok(candidate > current)
}

fn install_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Could not locate the running application: {e}"))?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "The running application has no parent directory".to_string())
}

/// Tries writing a real file instead of guessing from the path, because folder
/// permissions are what decide this. People run the app from USB sticks and
/// from Program Files, and neither can be written to.
fn can_write_install_dir() -> bool {
    let Ok(dir) = install_dir() else {
        return false;
    };
    let probe = dir.join(temp_name("probe"));
    match fs::File::create(&probe) {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Unique enough, without adding a random-number library just for this.
fn temp_name(kind: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!(".rivals-2-tag-tool-{kind}-{}-{nanos}.tmp", std::process::id())
}

/// Deletes the downloaded file unless the update used it. Nothing else knows
/// this file exists, so it has to clean up after itself.
struct StagedFile(Option<PathBuf>);

impl StagedFile {
    fn keep(mut self) -> PathBuf {
        self.0.take().expect("staged path taken twice")
    }
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = fs::remove_file(path);
        }
    }
}

#[tauri::command]
pub async fn check_for_update() -> Result<Option<UpdateInfo>, String> {
    let Some(platform) = platform_key() else {
        return Ok(None);
    };

    let manifest = fetch_manifest(platform).await?;
    if !is_newer(&manifest.version, env!("CARGO_PKG_VERSION"))? {
        return Ok(None);
    }
    check_download_url(&manifest.url)?;

    Ok(Some(UpdateInfo {
        version: manifest.version,
        notes: manifest.notes,
        pub_date: manifest.pub_date,
        can_self_install: SELF_INSTALL_SUPPORTED && can_write_install_dir(),
        release_page_url: release_page_url(),
    }))
}

/// Download, check, install, and restart. Does not return on success.
///
/// Looks the manifest up again rather than accepting one, so the frontend can
/// never choose what gets downloaded.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    if !SELF_INSTALL_SUPPORTED {
        return Err("This platform does not support installing updates in place".into());
    }

    let platform = platform_key().ok_or("This platform does not receive updates")?;
    let manifest = fetch_manifest(platform).await?;
    if !is_newer(&manifest.version, env!("CARGO_PKG_VERSION"))? {
        return Err("This is already the latest version".into());
    }
    check_download_url(&manifest.url)?;

    let (staged, digest) = download(&app, &manifest.url).await?;
    if !digest.eq_ignore_ascii_case(manifest.sha256.trim()) {
        // Returning here deletes the bad download.
        return Err(
            "The downloaded update did not match the expected checksum, so it was discarded."
                .into(),
        );
    }

    let path = staged.keep();
    let result = self_replace::self_replace(&path)
        .map_err(|e| format!("Could not replace the application: {e}"));
    // self_replace copies the file rather than moving it, so we delete ours.
    let _ = fs::remove_file(&path);
    result?;

    // The .exe path hasn't changed, it just holds the new build now, so this
    // starts the version we installed.
    app.restart();
}

/// Downloads the update next to the current .exe and returns its checksum.
///
/// Same folder means the same drive, which lets the swap be a rename instead of
/// a copy. The checksum is built up as the data arrives so we don't have to read
/// the whole file again afterwards.
async fn download(app: &AppHandle, url: &str) -> Result<(StagedFile, String), String> {
    let target = install_dir()?.join(temp_name("update"));
    // Keep this declared before `file`. Rust cleans up variables in reverse
    // order, so the file gets closed before this tries to delete it, and
    // Windows won't delete a file that's still open.
    let staged = StagedFile(Some(target.clone()));

    let mut response = client()?
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Could not download the update: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Update download returned {}", response.status()));
    }

    let total = response.content_length();
    if total.is_some_and(|len| len > MAX_UPDATE_BYTES) {
        return Err("The update is larger than expected".into());
    }

    let mut file = fs::File::create(&target)
        .map_err(|e| format!("Could not write next to the application: {e}"))?;
    let mut hasher = Sha256::new();
    let mut downloaded: u64 = 0;

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("The update download was interrupted: {e}"))?
    {
        downloaded += chunk.len() as u64;
        if downloaded > MAX_UPDATE_BYTES {
            return Err("The update is larger than expected".into());
        }
        hasher.update(&chunk);
        file.write_all(&chunk)
            .map_err(|e| format!("Could not write the update: {e}"))?;

        let _ = app.emit(PROGRESS_EVENT, DownloadProgress { downloaded, total });
    }

    file.sync_all()
        .map_err(|e| format!("Could not finish writing the update: {e}"))?;

    Ok((staged, hex(&hasher.finalize())))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_higher_version_counts_as_newer() {
        assert!(is_newer("2.1.2", "2.1.1").unwrap());
        assert!(is_newer("2.2.0", "2.1.9").unwrap());
        assert!(is_newer("3.0.0", "2.9.9").unwrap());
        assert!(!is_newer("2.1.1", "2.1.1").unwrap());
        assert!(!is_newer("2.1.0", "2.1.1").unwrap());
        // String comparison would get this one wrong.
        assert!(!is_newer("2.1.9", "2.10.0").unwrap());
    }

    #[test]
    fn a_leading_v_in_the_manifest_is_tolerated() {
        assert!(is_newer("v2.2.0", "2.1.1").unwrap());
    }

    #[test]
    fn prereleases_do_not_supersede_the_release_they_precede() {
        // A beta must never replace the finished release of the same version.
        assert!(!is_newer("2.2.0-beta.1", "2.2.0").unwrap());
        assert!(is_newer("2.2.0-beta.1", "2.1.0").unwrap());
    }

    #[test]
    fn an_unreadable_version_is_an_error_not_an_update() {
        assert!(is_newer("not-a-version", "2.1.1").is_err());
        assert!(is_newer("", "2.1.1").is_err());
    }

    #[test]
    fn this_builds_own_version_parses() {
        // Catches a Cargo.toml version the updater could never compare against.
        Version::parse(env!("CARGO_PKG_VERSION")).expect("crate version must be semver");
    }

    #[test]
    fn download_urls_must_be_github_over_https() {
        assert!(check_download_url(
            "https://github.com/alex-mireles/rivals-2-tag-tool/releases/download/v2.1.2/app.exe"
        )
        .is_ok());
        assert!(check_download_url("http://github.com/a/b/releases/download/v1/app.exe").is_err());
        assert!(check_download_url("https://example.com/app.exe").is_err());
        // Lookalike domains that a "ends with github.com" check would allow.
        assert!(check_download_url("https://evilgithub.com/app.exe").is_err());
        assert!(check_download_url("https://github.com.evil.net/app.exe").is_err());
        assert!(check_download_url("file:///C:/windows/system32/cmd.exe").is_err());
    }

    #[test]
    fn manifest_parses_with_only_the_required_fields() {
        let manifest: UpdateManifest = serde_json::from_str(
            r#"{"version":"2.1.2","url":"https://github.com/a/b","sha256":"ab12"}"#,
        )
        .expect("notes and pubDate are optional");
        assert_eq!(manifest.version, "2.1.2");
        assert!(manifest.notes.is_none());

        // Unknown fields must not break an older build reading a newer manifest.
        let forward: UpdateManifest = serde_json::from_str(
            r#"{"version":"2.1.2","url":"https://github.com/a/b","sha256":"ab12","futureField":true}"#,
        )
        .expect("unknown fields are tolerated");
        assert_eq!(forward.version, "2.1.2");
    }

    #[test]
    fn the_manifest_the_release_workflow_writes_is_readable() {
        // Copied exactly from what the release workflow writes. If the two ever
        // disagree on field names, updates quietly stop working.
        let emitted = r#"{
  "version": "2.1.1",
  "url": "https://github.com/alex-mireles/rivals-2-tag-tool/releases/download/v2.1.1/Rivals-II-Tag-Tool_2.1.1_windows_x64.exe",
  "sha256": "c373df4b9c93db4b7c2cffda52abc6968e6adc5dc1620a686272af327f92bacc",
  "pubDate": "2026-08-11T23:46:03Z"
}"#;

        let manifest: UpdateManifest =
            serde_json::from_str(emitted).expect("the workflow's manifest must deserialize");
        assert_eq!(manifest.version, "2.1.1");
        assert_eq!(manifest.sha256.len(), 64, "SHA-256 renders as 64 hex chars");
        assert!(manifest.pub_date.is_some(), "pubDate must map to pub_date");
        check_download_url(&manifest.url).expect("the workflow's URL must pass validation");
    }

    #[test]
    fn a_manifest_without_a_checksum_is_rejected() {
        // `sha256` has no default on purpose: defaulting it would mean
        // installing a file we never checked.
        serde_json::from_str::<UpdateManifest>(
            r#"{"version":"2.1.2","url":"https://github.com/a/b"}"#,
        )
        .expect_err("a manifest with no checksum must not parse");
    }

    #[test]
    fn digests_render_as_lowercase_hex_with_leading_zeroes() {
        assert_eq!(hex(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
        // The known SHA-256 of empty input.
        assert_eq!(
            hex(&Sha256::digest(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn checksum_comparison_ignores_case_and_surrounding_space() {
        // Windows tools write checksums in uppercase, Unix ones in lowercase.
        let digest = hex(&Sha256::digest(b"payload"));
        assert!(digest.eq_ignore_ascii_case(digest.to_uppercase().trim()));
        assert!(digest.eq_ignore_ascii_case(format!("  {}\n", digest.to_uppercase()).trim()));
        assert!(!digest.eq_ignore_ascii_case(hex(&Sha256::digest(b"other")).trim()));
    }

    #[test]
    fn staged_files_are_removed_unless_kept() {
        let dir = std::env::temp_dir();

        let doomed = dir.join(temp_name("test-drop"));
        fs::write(&doomed, b"x").unwrap();
        drop(StagedFile(Some(doomed.clone())));
        assert!(!doomed.exists(), "a dropped staged file must be cleaned up");

        let survivor = dir.join(temp_name("test-keep"));
        fs::write(&survivor, b"x").unwrap();
        let path = StagedFile(Some(survivor.clone())).keep();
        assert!(survivor.exists(), "a kept staged file must survive");
        assert_eq!(path, survivor);
        fs::remove_file(&survivor).unwrap();
    }
}
