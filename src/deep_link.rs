//! Provider-neutral deep-link grammar, based on
//! src/utils/deepLink/parseDeepLink.ts (2.1.88).
//! The old proprietary URI scheme is intentionally replaced with atlas-cli.
use anyhow::{bail, Context, Result};
use reqwest::Url;

pub const DEEP_LINK_PROTOCOL: &str = "atlas-cli";
const MAX_QUERY_LENGTH: usize = 5000;
const MAX_CWD_LENGTH: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeepLinkAction {
    pub query: Option<String>,
    pub cwd: Option<String>,
    pub repo: Option<String>,
}

fn contains_control_chars(input: &str) -> bool {
    input.chars().any(|c| c <= '\u{001f}' || c == '\u{007f}')
}
fn utf16_len(input: &str) -> usize {
    input.encode_utf16().count()
}
fn valid_repo_slug(input: &str) -> bool {
    let mut parts = input.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(owner), Some(repo), None) => [owner, repo].iter().all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
        }),
        _ => false,
    }
}
fn absolute_cwd(input: &str) -> bool {
    if input.starts_with('/') {
        return true;
    }
    let bytes = input.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

/// URL decoding, canonical validation and Unicode prompt security correspond
/// to parseDeepLink. All fields remain optional, and query is *never* executed.
pub fn parse_deep_link(uri: &str) -> Result<DeepLinkAction> {
    let explicit = format!("{DEEP_LINK_PROTOCOL}://");
    let colon = format!("{DEEP_LINK_PROTOCOL}:");
    let normalized = if uri.starts_with(&explicit) {
        uri.to_owned()
    } else if let Some(tail) = uri.strip_prefix(&colon) {
        format!("{explicit}{tail}")
    } else {
        bail!("Invalid deep link: expected {explicit} scheme, got \"{uri}\"")
    };
    let url =
        Url::parse(&normalized).with_context(|| format!("Invalid deep link URL: \"{uri}\""))?;
    if url.host_str() != Some("open") {
        bail!(
            "Unknown deep link action: \"{}\"",
            url.host_str().unwrap_or("")
        );
    }

    // URLSearchParams.get returns the *first* matching parameter.
    let mut cwd = None;
    let mut repo = None;
    let mut raw_query = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "cwd" if cwd.is_none() => cwd = Some(value.to_string()),
            "repo" if repo.is_none() => repo = Some(value.to_string()),
            "q" if raw_query.is_none() => raw_query = Some(value.to_string()),
            _ => (),
        }
    }
    if let Some(s) = cwd.as_deref().filter(|s| !s.is_empty()) {
        if !absolute_cwd(s) {
            bail!("Invalid cwd in deep link: must be an absolute path, got \"{s}\"");
        }
        if contains_control_chars(s) {
            bail!("Deep link cwd contains disallowed control characters");
        }
        if utf16_len(s) > MAX_CWD_LENGTH {
            bail!(
                "Deep link cwd exceeds {MAX_CWD_LENGTH} characters (got {})",
                utf16_len(s)
            );
        }
    }
    if let Some(s) = repo.as_deref().filter(|s| !s.is_empty()) {
        if !valid_repo_slug(s) {
            bail!("Invalid repo in deep link: expected \"owner/repo\", got \"{s}\"");
        }
    }
    let query = raw_query.and_then(|q| {
        let clean = q.trim();
        if clean.is_empty() {
            None
        } else {
            Some(crate::unicode_sanitization::partially_sanitize_unicode(
                clean,
            ))
        }
    });
    if let Some(ref q) = query {
        if contains_control_chars(q) {
            bail!("Deep link query contains disallowed control characters");
        }
        if utf16_len(q) > MAX_QUERY_LENGTH {
            bail!(
                "Deep link query exceeds {MAX_QUERY_LENGTH} characters (got {})",
                utf16_len(q)
            );
        }
    }
    Ok(DeepLinkAction {
        query,
        cwd: cwd.filter(|s| !s.is_empty()),
        repo: repo.filter(|s| !s.is_empty()),
    })
}

/// Matches buildDeepLink: encode query, cwd and repo in this order.
pub fn build_deep_link(action: &DeepLinkAction) -> Result<String> {
    let mut url = Url::parse(&format!("{DEEP_LINK_PROTOCOL}://open"))?;
    {
        let mut p = url.query_pairs_mut();
        if let Some(query) = action.query.as_deref().filter(|s| !s.is_empty()) {
            p.append_pair("q", query);
        }
        if let Some(cwd) = action.cwd.as_deref().filter(|s| !s.is_empty()) {
            p.append_pair("cwd", cwd);
        }
        if let Some(repo) = action.repo.as_deref().filter(|s| !s.is_empty()) {
            p.append_pair("repo", repo);
        }
    }
    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_both_uri_forms_and_decodes_first_values() {
        let a = parse_deep_link(
            "atlas-cli://open?q=hello+world&cwd=%2Ftmp%2Fproject&repo=x%2Fy&q=ignored",
        )
        .unwrap();
        assert_eq!(a.query.as_deref(), Some("hello world"));
        assert_eq!(a.cwd.as_deref(), Some("/tmp/project"));
        assert_eq!(a.repo.as_deref(), Some("x/y"));
        assert_eq!(
            parse_deep_link("atlas-cli:open").unwrap(),
            DeepLinkAction::default()
        );
    }
    #[test]
    fn normalizes_unicode_but_rejects_hidden_controls() {
        assert_eq!(
            parse_deep_link("atlas-cli://open?q=Ｈi%E2%80%8B")
                .unwrap()
                .query
                .as_deref(),
            Some("Hi")
        );
        assert!(parse_deep_link("atlas-cli://open?q=hello%0Aworld").is_err());
        assert!(parse_deep_link("atlas-cli://open?cwd=%2Ftmp%0Aevil").is_err());
        assert_eq!(
            parse_deep_link("atlas-cli://open?cwd=C%3A%5Cwork")
                .unwrap()
                .cwd
                .as_deref(),
            Some("C:\\work")
        );
    }
    #[test]
    fn validates_host_repo_and_bounds_without_truncation() {
        assert!(parse_deep_link("https://open?q=x").is_err());
        assert!(parse_deep_link("atlas-cli://delete?q=x").is_err());
        assert!(parse_deep_link("atlas-cli://open?repo=../a%2Fb").is_err());
        assert!(parse_deep_link("atlas-cli://open?repo=owner%2Frepo%2Fextra").is_err());
        assert!(parse_deep_link("atlas-cli://open?cwd=relative").is_err());
        assert!(parse_deep_link(&format!("atlas-cli://open?q={}", "a".repeat(5001))).is_err());
        assert!(parse_deep_link(&format!("atlas-cli://open?cwd=/{}", "a".repeat(4096))).is_err());
    }
    #[test]
    fn roundtrip_preserves_query_and_absolute_path() {
        let action = DeepLinkAction {
            query: Some("fix 🦀 & review".into()),
            cwd: Some("/a space/目录".into()),
            repo: Some("owner/repo".into()),
        };
        let url = build_deep_link(&action).unwrap();
        assert_eq!(parse_deep_link(&url).unwrap(), action);
    }
}
