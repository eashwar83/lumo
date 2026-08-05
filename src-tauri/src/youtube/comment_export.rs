//! Exports comments as a PDF.
//!
//! The text is routinely Telugu, Arabic, CJK and emoji, which rules out
//! the pure-Rust PDF crates: they cannot shape complex scripts and would
//! emit broken glyphs. Instead the comments become an HTML document that
//! headless Edge prints — Chromium already does the shaping, and pulls in
//! the system fonts (Nirmala UI and friends), so the output matches what
//! the drawer shows. Edge ships with Windows; if it is somehow absent the
//! HTML is saved instead, which prints from any browser.

use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Spawns without flashing a console window.
fn quiet_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x0800_0000);
    }
    command
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportComment {
    pub(crate) author: String,
    pub(crate) text: String,
    #[serde(default)]
    pub(crate) translated: Option<String>,
    #[serde(default)]
    pub(crate) published_text: Option<String>,
    #[serde(default)]
    pub(crate) like_count_text: Option<String>,
    /// Replies are indented under their parent.
    #[serde(default)]
    pub(crate) is_reply: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportPayload {
    pub(crate) title: String,
    pub(crate) video_url: String,
    pub(crate) destination: String,
    pub(crate) comments: Vec<ExportComment>,
    /// Shown in the header, e.g. "12 selected of 48".
    #[serde(default)]
    pub(crate) subtitle: Option<String>,
}

/// Writes the comments to `destination` as a PDF and returns the path
/// actually written — which is the .html fallback when Edge is missing.
#[tauri::command]
pub(crate) async fn youtube_export_comments(
    app: tauri::AppHandle,
    payload: ExportPayload,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if payload.comments.is_empty() {
            return Err("There are no comments to export".to_string());
        }
        let html = build_html(&payload);
        let destination = PathBuf::from(&payload.destination);

        let Some(edge) = find_edge() else {
            let fallback = destination.with_extension("html");
            std::fs::write(&fallback, &html)
                .map_err(|error| format!("Couldn't write the export: {error}"))?;
            return Ok(fallback.to_string_lossy().into_owned());
        };

        // Staged in our own cache rather than beside the target: the
        // destination folder may be watched or synced, and the name we
        // control cannot collide with the user's files.
        let staging = staging_dir(&app)?;
        let source = staging.join("comments.html");
        std::fs::write(&source, &html)
            .map_err(|error| format!("Couldn't stage the export: {error}"))?;

        // Deliberately not cleaned up here. Edge can return before it has
        // finished reading the page, and deleting it out from under the
        // browser is what silently produced a PDF of Chromium's
        // "File not found" page. The next export clears the directory.
        print_pdf(&edge, &staging, &source, &destination)?;

        if !destination.is_file() {
            return Err("Edge did not produce a PDF".to_string());
        }
        verify_pdf(&destination)?;
        Ok(destination.to_string_lossy().into_owned())
    })
    .await
    .map_err(|error| format!("Export worker failed: {error}"))?
}

/// Catches the failure mode where Edge printed something other than our
/// page. Chromium records the document title in the PDF, falling back to
/// the URL when the page did not load — so a `file:` title means the
/// export is an error page, however valid the PDF itself looks.
fn verify_pdf(destination: &Path) -> Result<(), String> {
    let Ok(bytes) = std::fs::read(destination) else {
        return Ok(());
    };
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    let Some(start) = head.find("/Title") else {
        return Ok(());
    };
    let title: String = head[start..].chars().take(200).collect();
    if title.contains("file:///") {
        let _ = std::fs::remove_file(destination);
        return Err(
            "The PDF came out as a browser error page, so it was discarded. \
             Please try the export again."
                .to_string(),
        );
    }
    Ok(())
}

/// Reveals an exported file in the file manager. The system may have no
/// working PDF association — showing the folder always works.
#[tauri::command]
pub(crate) fn youtube_export_reveal(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    let folder = path
        .parent()
        .ok_or_else(|| "That file has no folder".to_string())?;
    crate::commands::persistence::open_directory(folder)
}

/// A fresh directory for this export's page and Edge profile.
fn staging_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let base = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("Cache dir unavailable: {error}"))?
        .join("comment_export");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base)
        .map_err(|error| format!("Cache dir create failed: {error}"))?;
    Ok(base)
}

/// Percent-encodes the parts of a path that a file URL cannot carry raw.
fn file_url(path: &Path) -> String {
    let mut url = String::from("file:///");
    for character in path.to_string_lossy().chars() {
        match character {
            '\\' | '/' => url.push('/'),
            ':' | '-' | '_' | '.' | '~' => url.push(character),
            c if c.is_ascii_alphanumeric() => url.push(c),
            c => {
                let mut buffer = [0u8; 4];
                for byte in c.encode_utf8(&mut buffer).as_bytes() {
                    url.push_str(&format!("%{byte:02X}"));
                }
            }
        }
    }
    url
}

fn print_pdf(
    edge: &Path,
    staging: &Path,
    source: &Path,
    destination: &Path,
) -> Result<(), String> {
    let mut command = quiet_command(edge);
    let output = command
        .arg("--headless")
        .arg("--disable-gpu")
        // Without a private profile Edge's headless mode honours the
        // browser singleton: when Edge is already running our process
        // hands the command line over and exits at once, so we tear the
        // page down while the real browser is still loading it and the
        // PDF ends up containing "ERR_FILE_NOT_FOUND".
        .arg(format!(
            "--user-data-dir={}",
            staging.join("profile").to_string_lossy()
        ))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--no-pdf-header-footer")
        .arg(format!("--print-to-pdf={}", destination.to_string_lossy()))
        .arg(file_url(source))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("Couldn't run Edge to make the PDF: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Edge failed to write the PDF: {}",
            stderr.trim().chars().take(200).collect::<String>()
        ));
    }
    Ok(())
}

fn find_edge() -> Option<PathBuf> {
    let candidates = [
        std::env::var("ProgramFiles(x86)").ok(),
        std::env::var("ProgramFiles").ok(),
        std::env::var("LOCALAPPDATA").ok(),
    ];
    for root in candidates.into_iter().flatten() {
        let path = Path::new(&root)
            .join("Microsoft")
            .join("Edge")
            .join("Application")
            .join("msedge.exe");
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn build_html(payload: &ExportPayload) -> String {
    let mut body = String::new();
    for comment in &payload.comments {
        let class = if comment.is_reply { "c reply" } else { "c" };
        body.push_str(&format!("<div class=\"{class}\">"));
        body.push_str(&format!(
            "<div class=\"meta\"><span class=\"who\">{}</span>{}{}</div>",
            escape(&comment.author),
            comment
                .published_text
                .as_deref()
                .map(|when| format!("<span>{}</span>", escape(when)))
                .unwrap_or_default(),
            comment
                .like_count_text
                .as_deref()
                .map(|likes| format!("<span>&#9829; {}</span>", escape(likes)))
                .unwrap_or_default(),
        ));
        body.push_str(&format!(
            "<div class=\"text\">{}</div>",
            escape(&comment.text)
        ));
        if let Some(translated) = comment
            .translated
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != comment.text)
        {
            body.push_str(&format!(
                "<div class=\"tr\">{}</div>",
                escape(translated)
            ));
        }
        body.push_str("</div>");
    }

    format!(
        r#"<!doctype html>
<html><head><meta charset="utf-8"><title>{title}</title>
<style>
  @page {{ margin: 14mm 12mm; }}
  body {{ font-family: "Segoe UI", "Nirmala UI", "Segoe UI Emoji", sans-serif;
         font-size: 10.5pt; color: #111; line-height: 1.45; }}
  h1 {{ font-size: 15pt; margin: 0 0 2px; }}
  .src {{ font-size: 9pt; color: #555; word-break: break-all; }}
  .sub {{ font-size: 9pt; color: #555; margin-bottom: 14px; }}
  .c {{ padding: 7px 0; border-bottom: 1px solid #e6e6e6;
        break-inside: avoid; }}
  .reply {{ margin-left: 22px; border-bottom: none; padding: 4px 0 4px 10px;
            border-left: 2px solid #ddd; }}
  .meta {{ font-size: 8.5pt; color: #666; display: flex; gap: 10px;
           margin-bottom: 2px; }}
  .who {{ font-weight: 700; color: #222; }}
  .text {{ white-space: pre-wrap; word-wrap: break-word; }}
  .tr {{ white-space: pre-wrap; word-wrap: break-word; margin-top: 3px;
         padding-left: 9px; border-left: 2px solid #8b7cf7; color: #333; }}
</style></head>
<body>
<h1>{title}</h1>
<div class="src">{url}</div>
<div class="sub">{subtitle}</div>
{body}
</body></html>"#,
        title = escape(&payload.title),
        url = escape(&payload.video_url),
        subtitle = escape(payload.subtitle.as_deref().unwrap_or("")),
        body = body,
    )
}
