//! Source-grounded JSON and JSONL behaviors from 2.1.88 src/utils/json.ts.
//! All parsing and file I/O remain local, with no runtime JavaScript dependency.
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

const MAX_PARSE_CACHE_ITEMS: usize = 50;
const MAX_PARSE_CACHE_KEY_BYTES: usize = 8 * 1024;
const MAX_JSONL_READ_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Default)]
struct ParseCache {
    entries: std::collections::VecDeque<(String, Option<Value>)>,
}

fn cache() -> &'static Mutex<ParseCache> {
    static CACHE: OnceLock<Mutex<ParseCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(ParseCache::default()))
}

/// Strict parsing is used by paths that need detailed diagnostics, while the
/// original safeParseJSON conflates invalid input and JSON null as None.
pub fn parse_json<T: DeserializeOwned>(text: &str) -> serde_json::Result<T> {
    serde_json::from_str(crate::json_read::strip_bom(text))
}

pub fn safe_parse_json(text: Option<&str>) -> Option<Value> {
    let text = text.filter(|s| !s.is_empty())?;
    if text.len() > MAX_PARSE_CACHE_KEY_BYTES {
        return parse_json::<Value>(text).ok();
    }
    // Keep both successful parses and failures in the bounded LRU, matching
    // original memoization without pinning arbitrarily large config strings.
    if let Ok(mut cache) = cache().lock() {
        if let Some(index) = cache.entries.iter().position(|(key, _)| key == text) {
            let hit = cache.entries.remove(index)?;
            let result = hit.1.clone();
            cache.entries.push_back(hit);
            return result;
        }
    }
    let result = parse_json::<Value>(text).ok();
    if let Ok(mut cache) = cache().lock() {
        cache.entries.retain(|(key, _)| key != text);
        cache.entries.push_back((text.to_owned(), result.clone()));
        if cache.entries.len() > MAX_PARSE_CACHE_ITEMS {
            cache.entries.pop_front();
        }
    }
    result
}

/// Lexically strip JSONC comments, leaving newlines intact and replacing
/// comment bytes with spaces so string literals and Unicode remain untouched.
/// A second pass handles trailing commas outside quoted strings and comments.
fn strip_jsonc_comments(text: &str) -> Option<Vec<u8>> {
    let input = crate::json_read::strip_bom(text).as_bytes();
    let mut output = input.to_vec();
    let mut i = 0;
    let mut in_string = false;
    let mut escaped = false;
    while i < input.len() {
        if in_string {
            if escaped {
                escaped = false;
            } else if input[i] == b'\\' {
                escaped = true;
            } else if input[i] == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if input[i] == b'"' {
            in_string = true;
            i += 1;
            continue;
        }
        if input[i] == b'/' && i + 1 < input.len() {
            let start = i;
            if input[i + 1] == b'/' {
                i += 2;
                while i < input.len() && input[i] != b'\n' && input[i] != b'\r' {
                    i += 1;
                }
                output[start..i].fill(b' ');
                continue;
            }
            if input[i + 1] == b'*' {
                i += 2;
                let mut terminated = false;
                while i + 1 < input.len() {
                    if input[i] == b'*' && input[i + 1] == b'/' {
                        i += 2;
                        terminated = true;
                        break;
                    }
                    i += 1;
                }
                if !terminated {
                    return None;
                }
                for byte in &mut output[start..i] {
                    if *byte != b'\n' && *byte != b'\r' {
                        *byte = b' ';
                    }
                }
                continue;
            }
        }
        i += 1;
    }
    if in_string {
        return None;
    }
    Some(output)
}

fn normalize_jsonc(text: &str) -> Option<String> {
    let mut output = strip_jsonc_comments(text)?;
    let mut i = 0;
    let mut escaped = false;
    let mut in_string = false;
    // Replace trailing commas in unquoted JSON. The quote state is scanned
    // again since quote escapes can occur adjacent to comments.
    while i < output.len() {
        match (in_string, output[i]) {
            (true, b'\\') if !escaped => escaped = true,
            (true, b'"') if !escaped => in_string = false,
            (false, b'"') => in_string = true,
            (false, b',') => {
                let mut look = i + 1;
                while look < output.len() && output[look].is_ascii_whitespace() {
                    look += 1;
                }
                if look < output.len() && matches!(output[look], b']' | b'}') {
                    output[i] = b' ';
                }
            }
            _ => escaped = false,
        }
        if in_string && output[i] != b'\\' {
            escaped = false;
        }
        i += 1;
    }
    String::from_utf8(output).ok()
}

pub fn safe_parse_jsonc(text: Option<&str>) -> Option<Value> {
    let text = text.filter(|s| !s.is_empty())?;
    serde_json::from_str(&normalize_jsonc(text)?).ok()
}

/// Original parseJSONL skips each malformed line and accepts a string or a
/// Buffer. Decode with replacement characters as Node Buffer.toString does.
pub fn parse_jsonl(data: &[u8]) -> Vec<Value> {
    let data = if data.starts_with(&[0xef, 0xbb, 0xbf]) {
        &data[3..]
    } else {
        data
    };
    data.split(|b| *b == b'\n')
        .filter_map(|line| serde_json::from_str::<Value>(String::from_utf8_lossy(line).trim()).ok())
        .collect()
}

pub fn parse_jsonl_string(data: &str) -> Vec<Value> {
    parse_jsonl(data.as_bytes())
}

fn read_jsonl_file_limited(path: &Path, limit: u64) -> std::io::Result<Vec<Value>> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let offset = size.saturating_sub(limit);
    file.seek(SeekFrom::Start(offset))?;
    let mut data = Vec::new();
    file.take(limit).read_to_end(&mut data)?;
    if offset > 0 {
        // The original drops the first possibly partial record when tailing
        // long files. No newline means the first fragment is its entire tail.
        if let Some(end) = data.iter().position(|b| *b == b'\n') {
            return Ok(parse_jsonl(&data[end + 1..]));
        }
    }
    Ok(parse_jsonl(&data))
}

pub fn read_jsonl_file(path: &Path) -> std::io::Result<Vec<Value>> {
    read_jsonl_file_limited(path, MAX_JSONL_READ_BYTES)
}

fn pretty_array(new_item: &Value) -> String {
    serde_json::to_string_pretty(&vec![new_item]).unwrap_or_else(|_| "[]".into())
}

/// Add one element to a JSONC array while retaining existing inline comments,
/// original entries and indentation. Invalid JSONC or non-array input is
/// replaced with a one-element pretty JSON array, as in the source.
pub fn add_item_to_jsonc_array(text: &str, new_item: &Value) -> String {
    if text.trim().is_empty() {
        return pretty_array(new_item);
    }
    let original = crate::json_read::strip_bom(text);
    let Some(normalized) = normalize_jsonc(original) else {
        return pretty_array(new_item);
    };
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&normalized) else {
        return pretty_array(new_item);
    };
    // Both are byte-for-byte aligned: the normalization replaces comments and
    // commas with ASCII spaces and preserves all other bytes/newlines.
    let bytes = normalized.as_bytes();
    let Some(close) = bytes.iter().rposition(|b| !b.is_ascii_whitespace()) else {
        return pretty_array(new_item);
    };
    if bytes[close] != b']' {
        return pretty_array(new_item);
    }
    let Some(open) = bytes.iter().position(|b| !b.is_ascii_whitespace()) else {
        return pretty_array(new_item);
    };
    if bytes[open] != b'[' {
        return pretty_array(new_item);
    }
    let newline = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let line_start = original[..close].rfind('\n').map_or(0, |p| p + 1);
    let indent = original[line_start..close]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect::<String>();
    let item_indent = format!("{indent}    ");
    let serialized = serde_json::to_string_pretty(new_item).unwrap_or_else(|_| "null".into());
    let item = serialized.replace('\n', &format!("{newline}{item_indent}"));
    let mut edited = original.to_owned();
    let mut inserted_comma = false;
    if !items.is_empty() {
        // Find the last material byte before ] ignoring JSONC comments. Add
        // the delimiter directly after the final item, before any comments.
        if let Some(last) = (open + 1..close)
            .rev()
            .find(|i| !bytes[*i].is_ascii_whitespace())
        {
            // `normalize_jsonc` blanks trailing comma. If there was one,
            // retain it instead of injecting a second comma.
            if original.as_bytes()[last] != b',' {
                // A trailing comma (if any) was normalized away. Find the
                // original comma after last material byte before the close.
                let trailing_comma = strip_jsonc_comments(original)
                    .is_some_and(|masked| masked[last + 1..close].contains(&b','));
                if !trailing_comma {
                    edited.insert(last + 1, ',');
                    inserted_comma = true;
                }
            }
        }
    }
    let final_close = close + usize::from(inserted_comma);
    edited.insert_str(
        final_close,
        &format!("{newline}{item_indent}{item}{newline}{indent}"),
    );
    // Preserve the original comment text exactly, including trailing comments.
    if safe_parse_jsonc(Some(&edited)).is_none() {
        let mut copy = items;
        copy.push(new_item.clone());
        return serde_json::to_string_pretty(&copy).unwrap_or_else(|_| pretty_array(new_item));
    }
    edited
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    #[test]
    fn strict_json_bom_and_failure_cache_behavior() {
        assert_eq!(
            parse_json::<Value>("\u{feff}{\"ok\":true}").unwrap(),
            json!({"ok":true})
        );
        assert_eq!(safe_parse_json(Some("null")), Some(Value::Null));
        assert_eq!(safe_parse_json(Some("{")), None);
        assert_eq!(safe_parse_json(Some("{")), None);
        assert_eq!(safe_parse_json(None), None);
        assert_eq!(safe_parse_json(Some("")), None);
        assert_eq!(
            safe_parse_json(Some(&" ".repeat(MAX_PARSE_CACHE_KEY_BYTES + 1))),
            None
        );
    }

    #[test]
    fn jsonc_preserves_slashes_in_strings_and_strips_comments_commas() {
        let text =
            "\u{feff}{\"url\":\"https://host/a/*keep*/\",/* block */\"values\":[1,2,],//tail\n}";
        assert_eq!(
            safe_parse_jsonc(Some(text)),
            Some(json!({"url":"https://host/a/*keep*/","values":[1,2]}))
        );
        assert_eq!(safe_parse_jsonc(Some("[1,/*unterminated")), None);
        assert_eq!(safe_parse_jsonc(Some("[1,,2]")), None);
    }

    #[test]
    fn jsonl_ignores_invalid_and_strips_only_initial_bom() {
        let data = b"\xef\xbb\xbf{\"a\":1}\r\ninvalid\nnull\n[1,2]\n";
        assert_eq!(
            parse_jsonl(data),
            vec![json!({"a":1}), Value::Null, json!([1, 2])]
        );
        assert_eq!(
            parse_jsonl_string("\u{feff}true\nnot-json\nfalse"),
            vec![json!(true), json!(false)]
        );
    }

    #[test]
    fn tail_read_discards_partial_first_line_and_keeps_valid_rest() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("history.jsonl");
        std::fs::write(&file, b"{\"first\":1}\n{\"second\":2}\n{\"last\":3}\n").unwrap();
        assert_eq!(
            read_jsonl_file_limited(&file, 27).unwrap(),
            vec![json!({"second":2}), json!({"last":3})]
        );
        assert_eq!(read_jsonl_file(&file).unwrap().len(), 3);
        assert!(read_jsonl_file(&dir.path().join("missing")).is_err());
        let mut out = File::create(dir.path().join("empty")).unwrap();
        out.write_all(b"").unwrap();
        assert!(read_jsonl_file(&dir.path().join("empty"))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn jsonc_append_retains_existing_comments_and_trailing_comma() {
        let text = "[\n    {\"command\":\"a\"}, // existing\n]";
        let updated = add_item_to_jsonc_array(text, &json!({"command":"b"}));
        assert!(updated.contains("// existing"));
        assert_eq!(
            safe_parse_jsonc(Some(&updated)),
            Some(json!([{"command":"a"},{"command":"b"}]))
        );
        let another = add_item_to_jsonc_array("[1, /*comment*/]", &json!(2));
        assert!(another.contains("/*comment*/"));
        assert_eq!(safe_parse_jsonc(Some(&another)), Some(json!([1, 2])));
        assert_eq!(
            safe_parse_jsonc(Some(&add_item_to_jsonc_array("", &json!(1)))),
            Some(json!([1]))
        );
        assert_eq!(
            safe_parse_jsonc(Some(&add_item_to_jsonc_array("{\"a\":1}", &json!(2)))),
            Some(json!([2]))
        );
    }
}
