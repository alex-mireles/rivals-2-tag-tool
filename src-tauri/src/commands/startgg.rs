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

/// An entrant in a start.gg event, with their linked start.gg user slug.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventEntrant {
    /// The entrant name as it appears in the bracket (may include a prefix/team).
    pub entrant: String,
    pub gamer_tag: String,
    pub slug: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventResult {
    pub event: String,
    pub entrants: Vec<EventEntrant>,
}

/// Pull `tournament/<t>/event/<e>` out of a start.gg event URL (or accept a
/// bare slug).
fn parse_event_slug(text: &str) -> Option<String> {
    let t = text.trim();
    // Find "tournament/<seg>/event/<seg>" anywhere in the string.
    let bytes = t.to_ascii_lowercase();
    let idx = bytes.find("tournament/")?;
    let rest = &t[idx..];
    let parts: Vec<&str> = rest.split('/').collect();
    // parts: ["tournament", <t>, "event", <e>, ...]
    if parts.len() >= 4 && parts[0].eq_ignore_ascii_case("tournament") && parts[2].eq_ignore_ascii_case("event") {
        let tour = parts[1];
        // Trim any trailing query/fragment on the event segment.
        let ev = parts[3].split(|c| c == '?' || c == '#').next().unwrap_or("");
        if !tour.is_empty() && !ev.is_empty() {
            return Some(format!("tournament/{tour}/event/{ev}"));
        }
    }
    None
}

/// Resolve a start.gg event URL to its entrants and their linked user slugs,
/// for matching against shared tags (download-by-bracket).
#[tauri::command]
pub async fn startgg_event(url: String) -> Result<EventResult, String> {
    let slug = parse_event_slug(&url)
        .ok_or_else(|| "Expected a start.gg event URL (…/tournament/<t>/event/<e>).".to_string())?;

    let mut entrants = Vec::new();
    let mut event_name = String::new();
    let mut page = 1;
    let mut total_pages = 1;

    while page <= total_pages && page <= 30 {
        let data = gql(
            "query($slug:String!,$page:Int!){ event(slug:$slug){ name \
               entrants(query:{ page:$page, perPage:64 }){ \
                 pageInfo{ totalPages } \
                 nodes{ name participants{ gamerTag user{ slug } } } } } }",
            serde_json::json!({ "slug": slug, "page": page }),
        )
        .await?;

        let ev = data
            .get("event")
            .filter(|v| !v.is_null())
            .ok_or_else(|| "Event not found.".to_string())?;

        if page == 1 {
            event_name = str_at(ev, "/name").to_string();
        }
        total_pages = ev
            .pointer("/entrants/pageInfo/totalPages")
            .and_then(|v| v.as_i64())
            .unwrap_or(1);

        if let Some(nodes) = ev.pointer("/entrants/nodes").and_then(|v| v.as_array()) {
            for n in nodes {
                let entrant = str_at(n, "/name").to_string();
                if let Some(parts) = n.pointer("/participants").and_then(|v| v.as_array()) {
                    for p in parts {
                        let slug = str_at(p, "/user/slug");
                        if !slug.is_empty() {
                            entrants.push(EventEntrant {
                                entrant: entrant.clone(),
                                gamer_tag: str_at(p, "/gamerTag").to_string(),
                                slug: slug.to_string(),
                            });
                        }
                    }
                }
            }
        }
        page += 1;
    }

    Ok(EventResult { event: event_name, entrants })
}

#[cfg(test)]
mod tests {
    use super::{is_user_slug, parse_event_slug};

    #[test]
    fn parses_event_slugs() {
        assert_eq!(
            parse_event_slug("https://www.start.gg/tournament/port-priority-dx/event/rivals-of-aether-ii-singles"),
            Some("tournament/port-priority-dx/event/rivals-of-aether-ii-singles".into())
        );
        assert_eq!(
            parse_event_slug("start.gg/tournament/x/event/y/brackets/123/456"),
            Some("tournament/x/event/y".into())
        );
        assert_eq!(
            parse_event_slug("tournament/a/event/b?foo=1"),
            Some("tournament/a/event/b".into())
        );
        assert_eq!(parse_event_slug("https://www.start.gg/user/6192f6f1"), None);
        assert_eq!(parse_event_slug("nonsense"), None);
    }


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
