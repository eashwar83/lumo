//! Signs Innertube requests with the user's YouTube session.
//!
//! Only used where anonymous requests come back empty: YouTube hides an
//! age-restricted video's comments from signed-out clients. Cookies alone
//! are not enough — the request must also carry a `SAPISIDHASH`, which is
//! a SHA-1 over the current time, the SAPISID cookie, and the origin.
//!
//! Scoped deliberately: search and browsing stay anonymous so ordinary use
//! of Lumo is never attributed to the user's Google account.

use sha1::{Digest, Sha1};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

const ORIGIN: &str = "https://www.youtube.com";

/// The headers that turn an Innertube call into an authenticated one.
pub(crate) struct SessionAuth {
    pub(crate) cookie: String,
    pub(crate) authorization: String,
}

/// Reads the configured cookies.txt and derives the signing headers.
/// Returns None whenever the user has not configured one, or it lacks a
/// usable session — callers then carry on anonymously.
pub(crate) fn session_auth(app: &AppHandle) -> Option<SessionAuth> {
    let path = crate::mpv::resolve_ytdlp_settings(app).cookies.file?;
    let jar = read_netscape_cookies(&path)?;

    // Google accepts either; 1P is the first-party session, 3P the one
    // used in embedded contexts.
    let sapisid = jar
        .iter()
        .find(|(name, _)| name == "SAPISID")
        .or_else(|| jar.iter().find(|(name, _)| name == "__Secure-3PAPISID"))
        .map(|(_, value)| value.clone())?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();
    let mut hasher = Sha1::new();
    hasher.update(format!("{now} {sapisid} {ORIGIN}").as_bytes());
    let digest = hasher.finalize();
    let digest = digest.iter().fold(String::new(), |mut out, byte| {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
        out
    });

    let cookie = jar
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ");

    Some(SessionAuth {
        cookie,
        authorization: format!("SAPISIDHASH {now}_{digest}"),
    })
}

/// Parses a Netscape-format cookies.txt into (name, value) pairs.
fn read_netscape_cookies(path: &str) -> Option<Vec<(String, String)>> {
    let text = std::fs::read_to_string(path).ok()?;
    let jar: Vec<(String, String)> = text
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .filter_map(|line| {
            // domain, include_subdomains, path, secure, expiry, name, value
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 7 {
                return None;
            }
            Some((fields[5].to_string(), fields[6].to_string()))
        })
        .collect();
    (!jar.is_empty()).then_some(jar)
}

/// Adds the signing headers to a request when a session is available.
pub(crate) fn apply(
    request: reqwest::blocking::RequestBuilder,
    auth: Option<&SessionAuth>,
) -> reqwest::blocking::RequestBuilder {
    let Some(auth) = auth else { return request };
    request
        .header("Cookie", &auth.cookie)
        .header("Authorization", &auth.authorization)
        .header("X-Goog-AuthUser", "0")
        .header("X-Origin", ORIGIN)
        .header("Origin", ORIGIN)
}
