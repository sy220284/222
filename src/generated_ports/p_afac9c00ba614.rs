//! Rust exports derived from original: src/ink/components/Link.tsx
//!
//! Hyperlink branch semantics: when the terminal supports OSC-8 hyperlinks
//! the children (defaulting to the URL) are wrapped in a link; otherwise the
//! fallback node renders. supportsHyperlinks rides the ink detection port.

/// Render decision for a link node.
pub enum LinkRender {
    /// Terminal supports hyperlinks: wrap content in an OSC-8 link.
    Hyperlink { content: String, url: String },
    /// No hyperlink support: render the fallback (or content) plainly.
    Plain { content: String },
}

/// `Link` render resolution.
pub fn link_render(
    supports_hyperlinks: bool,
    children: Option<&str>,
    url: &str,
    fallback: Option<&str>,
) -> LinkRender {
    let content = children.unwrap_or(url).to_owned();
    if supports_hyperlinks {
        LinkRender::Hyperlink {
            content,
            url: url.to_owned(),
        }
    } else {
        LinkRender::Plain {
            content: fallback.unwrap_or(&content).to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branches_match_original() {
        let r = link_render(true, Some("doc"), "http://x", Some("f"));
        assert!(matches!(r, LinkRender::Hyperlink { ref content, .. } if content == "doc"));
        let r = link_render(false, None, "http://x", Some("f"));
        assert!(matches!(r, LinkRender::Plain { ref content } if content == "f"));
        let r = link_render(false, None, "http://x", None);
        assert!(matches!(r, LinkRender::Plain { ref content } if content == "http://x"));
    }
}
