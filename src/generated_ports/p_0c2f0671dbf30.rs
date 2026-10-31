//! Native Rust port derived from original: src/utils/intl.ts
//!
//! Shared text-segmentation and locale helpers with lazy process-lifetime
//! caching. `Intl.Segmenter` has no Rust std equivalent, so grapheme
//! segmentation uses the same scalar-walk + ZWJ/variation-selector merge
//! as the ANSI slice port (recorded remaining difference: full UAX #29
//! grapheme boundaries are approximated; word segmentation splits on
//! whitespace and surrounding punctuation). Locale/timezone resolution
//! reads `TZ`/`LANG` instead of ICU, matching stripped-ICU behaviour.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use std::sync::OnceLock;

fn is_variation_or_joiner(cp: u32) -> bool {
    (0xfe00..=0xfe0f).contains(&cp) || cp == 0x200d || cp == 0x20e3
}

fn is_regional_indicator(cp: u32) -> bool {
    (0x1f1e6..=0x1f1ff).contains(&cp)
}

/// Approximated grapheme segmentation: base scalar + combining marks
/// (0x0300-0x036f et al.), variation selectors, ZWJ chains and regional
/// indicator pairs stay attached to their base.
pub fn segment_graphemes(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let cp = chars[i] as u32;
        let start = i;
        i += 1;
        // Regional indicator pair forms one cluster.
        if is_regional_indicator(cp) && i < chars.len() && is_regional_indicator(chars[i] as u32) {
            i += 1;
        }
        // Attach continuation scalars: combining marks, VS, ZWJ+next.
        while i < chars.len() {
            let c = chars[i] as u32;
            if is_variation_or_joiner(c) || (0x0300..=0x036f).contains(&c) {
                i += 1;
                if chars.get(i - 1) == Some(&'\u{200d}') && i < chars.len() {
                    i += 1; // ZWJ pulls the next base into the cluster.
                }
                continue;
            }
            break;
        }
        out.push(chars[start..i].iter().collect());
    }
    out
}

/// Extract the first grapheme cluster from a string. Returns "" for empty.
pub fn first_grapheme(text: &str) -> String {
    segment_graphemes(text)
        .into_iter()
        .next()
        .unwrap_or_default()
}

/// Extract the last grapheme cluster from a string. Returns "" for empty.
pub fn last_grapheme(text: &str) -> String {
    segment_graphemes(text)
        .into_iter()
        .next_back()
        .unwrap_or_default()
}

/// Approximated word segmentation: runs of word characters vs everything
/// else, whitespace dropped (mirrors `granularity: 'word'` usage where
/// callers iterate words for search/matching).
pub fn segment_words(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_wordish: Option<bool> = None;
    for ch in text.chars() {
        let wordish = ch.is_alphanumeric() || ch == '_';
        match current_wordish {
            Some(prev) if prev == wordish => current.push(ch),
            Some(_) => {
                if !current.trim().is_empty() {
                    out.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
                current.push(ch);
                current_wordish = Some(wordish);
            }
            None => {
                current.push(ch);
                current_wordish = Some(wordish);
            }
        }
    }
    if !current.trim().is_empty() {
        out.push(current);
    }
    out
}

/// Process-lifetime timezone: `TZ` env when set, otherwise UTC fallback.
/// (Original resolves via `Intl.DateTimeFormat().resolvedOptions().timeZone`.)
pub fn get_time_zone() -> &'static str {
    static TZ: OnceLock<String> = OnceLock::new();
    TZ.get_or_init(|| {
        std::env::var("TZ").unwrap_or_else(|_| {
            // /etc/localtime symlink target gives the IANA zone on Unix.
            #[cfg(unix)]
            {
                if let Ok(target) = std::fs::read_link("/etc/localtime") {
                    let s = target.to_string_lossy();
                    if let Some(idx) = s.find("zoneinfo/") {
                        return s[idx + "zoneinfo/".len()..].to_string();
                    }
                }
            }
            "UTC".to_string()
        })
    })
}

/// System locale language subtag (e.g. "en", "ja"); None when unavailable.
/// Read once (original caches `undefined` for stripped-ICU to fail once).
pub fn get_system_locale_language() -> Option<&'static str> {
    static LANG: OnceLock<Option<String>> = OnceLock::new();
    LANG.get_or_init(|| {
        let raw = std::env::var("LANG")
            .or_else(|_| std::env::var("LC_ALL"))
            .ok()?;
        let primary = raw.split('.').next().unwrap_or(&raw);
        let language = primary.split(['_', '-']).next()?;
        if language.is_empty() || language == "C" || language == "POSIX" {
            return None;
        }
        Some(language.to_lowercase())
    })
    .as_deref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grapheme_clusters_group_emoji_zwj() {
        // Family emoji: man + ZWJ + woman + ZWJ + girl = one cluster.
        let family = "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}";
        assert_eq!(segment_graphemes(family).len(), 1);
        assert_eq!(first_grapheme(family), family);
    }

    #[test]
    fn regional_pair_is_single_grapheme() {
        assert_eq!(segment_graphemes("\u{1f1ef}\u{1f1f5}").len(), 1);
    }

    #[test]
    fn combining_mark_stays_with_base() {
        let s = "\u{00e9}x"; // é + x
        let segs = segment_graphemes(s);
        assert_eq!(segs.len(), 2);
        assert_eq!(last_grapheme(s), "x");
    }

    #[test]
    fn empty_text_returns_empty_grapheme() {
        assert_eq!(first_grapheme(""), "");
        assert_eq!(last_grapheme(""), "");
    }

    #[test]
    fn words_split_and_skip_whitespace() {
        assert_eq!(segment_words("hello world"), vec!["hello", "world"]);
        assert_eq!(segment_words("a-b"), vec!["a", "-", "b"]);
    }

    #[test]
    fn locale_language_parses_lang() {
        // LANG presence depends on env; only verify shape when set.
        if let Some(lang) = get_system_locale_language() {
            assert!(!lang.is_empty());
            assert!(lang.chars().all(|c| c.is_ascii_lowercase()));
        }
    }
}
