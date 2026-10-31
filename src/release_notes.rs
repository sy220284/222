//! Release notes cache, parsing, version filtering and user-triggered refresh.
//! The remote URL is explicit user configuration, never a vendor endpoint.
use anyhow::{Context, Result};
use std::{collections::BTreeMap, fs, path::Path, time::Duration};

const MAX_RECENT_NOTES: usize = 5;

fn numeric_version(s: &str) -> Option<(u32, u32, u32)> {
    let version = s
        .trim()
        .trim_start_matches('v')
        .split([' ', '-', '+'])
        .next()?;
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    Some((major, minor, patch))
}

pub fn parse_changelog(content: &str) -> Vec<(String, Vec<String>)> {
    let mut releases = BTreeMap::<(u32, u32, u32), (String, Vec<String>)>::new();
    let mut current: Option<(u32, u32, u32)> = None;
    for line in content.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            let name = heading.trim().split(" - ").next().unwrap_or("").trim();
            current = numeric_version(name);
            if let Some(key) = current {
                releases
                    .entry(key)
                    .or_insert_with(|| (name.to_owned(), Vec::new()));
            }
        } else if let Some(note) = line.trim().strip_prefix("- ") {
            if let Some(key) = current {
                if !note.trim().is_empty() {
                    releases
                        .entry(key)
                        .or_default()
                        .1
                        .push(note.trim().to_owned());
                }
            }
        }
    }
    releases
        .into_values()
        .filter(|(_, notes)| !notes.is_empty())
        .collect()
}

pub fn all_notes(content: &str) -> String {
    parse_changelog(content)
        .into_iter()
        .map(|(version, notes)| {
            format!(
                "Version {version}:\n{}",
                notes
                    .iter()
                    .map(|s| format!("· {s}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn recent_notes(content: &str, current: &str, previous: Option<&str>) -> Vec<String> {
    let Some(current) = numeric_version(current) else {
        return Vec::new();
    };
    let prev = previous.and_then(numeric_version);
    if prev.is_some_and(|p| current <= p) {
        return Vec::new();
    }
    parse_changelog(content)
        .into_iter()
        .rev()
        .filter(|(v, _)| {
            numeric_version(v)
                .is_some_and(|version| version <= current && prev.is_none_or(|p| version > p))
        })
        .flat_map(|(_, notes)| notes)
        .take(MAX_RECENT_NOTES)
        .collect()
}

pub fn cache_path(home: &Path) -> std::path::PathBuf {
    home.join("cache/changelog.md")
}

pub fn cached(home: &Path) -> Result<String> {
    let path = cache_path(home);
    match fs::read_to_string(&path) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => {
            Err(error).with_context(|| format!("Cannot read changelog at {}", path.display()))
        }
    }
}

pub async fn refresh(home: &Path, url: &str) -> Result<bool> {
    let url = reqwest::Url::parse(url).context("Invalid configured changelog URL")?;
    if !matches!(url.scheme(), "http" | "https") {
        anyhow::bail!("Changelog URL must use HTTP(S)");
    }
    let client = crate::tls_ca::client_builder()?
        .timeout(Duration::from_millis(500))
        .build()?;
    let response = client.get(url).send().await?.error_for_status()?;
    let content = response.text().await?;
    if cached(home)? == content {
        return Ok(false);
    }
    let dest = cache_path(home);
    fs::create_dir_all(dest.parent().context("Invalid changelog cache directory")?)?;
    // Atomic replacement prevents readers from observing partial changelogs.
    let temp = dest.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        fs::write(&temp, &content)?;
        fs::rename(&temp, &dest)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result?;
    Ok(true)
}

pub async fn show(home: &Path, remote_url: Option<&str>) -> Result<String> {
    if let Some(url) = remote_url.filter(|u| !u.trim().is_empty()) {
        // A failed or slow optional changelog fetch never discards the cache.
        // Remote configuration itself is explicit opt-in.
        let _ = refresh(home, url).await;
    }
    let content = cached(home)?;
    if content.is_empty() {
        return Ok("No cached release notes. Set ATLAS_RELEASE_NOTES_URL to a trusted changelog URL to fetch them.".into());
    }
    let rendered = all_notes(&content);
    if rendered.is_empty() {
        Ok("No versioned release notes found in cached changelog.".into())
    } else {
        Ok(rendered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const CHANGELOG:&str="# Changelog\n## 1.3.0 - 2026-10-29\n- Alpha\n- Beta\n## 1.2.0\n- Previous\n## nonsense\n- Ignore\n## 1.1.0\n- Old\n";
    #[test]
    fn parse_and_recent_filter_follow_source_version_and_item_order() {
        let data = parse_changelog(CHANGELOG);
        assert_eq!(data.len(), 3);
        assert_eq!(data[0].0, "1.1.0");
        assert_eq!(data[2].1, vec!["Alpha", "Beta"]);
        assert_eq!(
            recent_notes(CHANGELOG, "1.3.0", Some("1.2.0")),
            vec!["Alpha", "Beta"]
        );
        assert!(recent_notes(CHANGELOG, "1.2.0", Some("1.3.0")).is_empty());
        assert_eq!(recent_notes(CHANGELOG, "1.1.0", None), vec!["Old"]);
        assert!(all_notes(CHANGELOG).contains("Version 1.3.0:"));
    }
    #[tokio::test]
    async fn localhost_http_refresh_persists_and_prefers_new_content() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let root = tempfile::tempdir().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let service = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request).await.unwrap();
            let body = CHANGELOG;
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });
        assert!(refresh(root.path(), &format!("http://{address}/notes.md"))
            .await
            .unwrap());
        service.await.unwrap();
        assert!(cached(root.path()).unwrap().contains("Alpha"));
        assert!(show(root.path(), None).await.unwrap().contains("Previous"));
    }
}
