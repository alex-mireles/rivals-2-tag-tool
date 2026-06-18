//! start.gg lookups for linking a tag to a player's start.gg account.
//!
//! These call start.gg's unauthenticated website GraphQL endpoint directly
//! (the same one the site itself uses — no API token). Because the requests
//! go out from Rust rather than the webview, they aren't subject to browser
//! CORS, so no proxy is needed.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const STARTGG_API: &str = "https://www.start.gg/api/-/gql";
const USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 6.0; Nexus 5 Build/MRA58N) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Mobile Safari/537.36";

/// A start.gg account as shown in the search dropdown.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartggPlayer {
    pub gamer_tag: String,
    pub prefix: String,
    /// The user slug, e.g. `user/6192f6f1`.
    pub slug: String,
    /// Profile image URL, or empty if the account has none.
    pub image: String,
}

#[derive(Serialize)]
struct GqlRequest<'a> {
    query: &'a str,
    variables: serde_json::Value,
}

/// True for a well-formed user slug like `user/6192f6f1`.
fn is_user_slug(s: &str) -> bool {
    match s.strip_prefix("user/") {
        Some(id) => !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric()),
        None => false,
    }
}

async fn gql(query: &str, variables: serde_json::Value) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let res = client
        .post(STARTGG_API)
        .header("Content-Type", "application/json")
        .header("client-version", "20")
        .header("User-Agent", USER_AGENT)
        .json(&GqlRequest { query, variables })
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(format!("start.gg returned {}", res.status()));
    }

    let body: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    let has_errors = body
        .get("errors")
        .and_then(|e| e.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    if has_errors {
        return Err("start.gg GraphQL error".into());
    }

    Ok(body.get("data").cloned().unwrap_or(serde_json::Value::Null))
}

fn str_at<'a>(v: &'a serde_json::Value, ptr: &str) -> &'a str {
    v.pointer(ptr).and_then(|x| x.as_str()).unwrap_or("")
}

/// Search start.gg players by gamer tag for the link dropdown. Returns only
/// accounts with a linked start.gg user (those are the ones that can be matched
/// against brackets on the website), de-duplicated by slug.
#[tauri::command]
pub async fn startgg_search(query: String) -> Result<Vec<StartggPlayer>, String> {
    let q = query.trim();
    if q.chars().count() < 2 {
        return Ok(Vec::new());
    }

    let data = gql(
        "query($q:String!){ players(query:{ perPage:30, filter:{ gamerTag:$q } }){ \
           nodes{ gamerTag prefix user{ slug images(type:\"profile\"){ url } } } } }",
        serde_json::json!({ "q": q }),
    )
    .await?;

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    if let Some(nodes) = data.pointer("/players/nodes").and_then(|v| v.as_array()) {
        for n in nodes {
            let slug = str_at(n, "/user/slug");
            if slug.is_empty() || !seen.insert(slug.to_string()) {
                continue;
            }
            out.push(StartggPlayer {
                gamer_tag: str_at(n, "/gamerTag").to_string(),
                prefix: str_at(n, "/prefix").to_string(),
                slug: slug.to_string(),
                image: str_at(n, "/user/images/0/url").to_string(),
            });
        }
    }
    Ok(out)
}

/// Resolve a single start.gg user by slug (for when someone pastes their
/// profile URL instead of searching).
#[tauri::command]
pub async fn startgg_user(slug: String) -> Result<StartggPlayer, String> {
    let slug = slug.trim();
    if !is_user_slug(slug) {
        return Err("Not a valid start.gg user slug (expected user/<id>).".into());
    }

    let data = gql(
        "query($slug:String!){ user(slug:$slug){ slug player{ gamerTag prefix } \
           images(type:\"profile\"){ url } } }",
        serde_json::json!({ "slug": slug }),
    )
    .await?;

    let user = data
        .get("user")
        .filter(|v| !v.is_null())
        .ok_or_else(|| "start.gg user not found.".to_string())?;

    Ok(StartggPlayer {
        gamer_tag: str_at(user, "/player/gamerTag").to_string(),
        prefix: str_at(user, "/player/prefix").to_string(),
        slug: user.get("slug").and_then(|v| v.as_str()).unwrap_or(slug).to_string(),
        image: str_at(user, "/images/0/url").to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::is_user_slug;

    #[test]
    fn accepts_valid_slugs() {
        assert!(is_user_slug("user/6192f6f1"));
        assert!(is_user_slug("user/e85def91"));
    }

    #[test]
    fn rejects_bad_slugs() {
        assert!(!is_user_slug("user/"));
        assert!(!is_user_slug("6192f6f1"));
        assert!(!is_user_slug("user/../etc"));
        assert!(!is_user_slug(""));
        assert!(!is_user_slug("tournament/x"));
    }
}
