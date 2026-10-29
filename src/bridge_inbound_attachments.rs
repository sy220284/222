//! Session-owned inbound attachments for a configurable remote bridge.
//! Source semantics: 2.1.88 `src/bridge/inboundAttachments.ts`.
//! No provider-specific endpoint or implicit network access is retained.
use futures::{future::join_all, StreamExt};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{fs, io::AsyncWriteExt};

const MAX_FILES: usize = 8;
const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub file_uuid: String,
    pub file_name: String,
}

/// Matches the original all-or-nothing schema validation.
pub fn extract(event: &Value) -> Vec<Attachment> {
    let Some(raw) = event.get("file_attachments") else {
        return Vec::new();
    };
    let Some(array) = raw.as_array() else {
        return Vec::new();
    };
    if array.len() > MAX_FILES {
        return Vec::new();
    }
    let mut parsed = Vec::with_capacity(array.len());
    for value in array {
        let (Some(uuid), Some(name)) = (
            value.get("file_uuid").and_then(Value::as_str),
            value.get("file_name").and_then(Value::as_str),
        ) else {
            return Vec::new();
        };
        if uuid.is_empty() || uuid.len() > 256 || name.len() > 1024 {
            return Vec::new();
        }
        parsed.push(Attachment {
            file_uuid: uuid.to_owned(),
            file_name: name.to_owned(),
        });
    }
    parsed
}

fn safe_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let clean: String = base
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || "._-".contains(ch) {
                ch
            } else {
                '_'
            }
        })
        .take(160)
        .collect();
    if clean.is_empty() || clean == "." || clean == ".." {
        "attachment".to_owned()
    } else {
        clean
    }
}

fn safe_session_id(value: &str) -> String {
    let clean: String = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .take(128)
        .collect();
    if clean.is_empty() {
        "session".to_owned()
    } else {
        clean
    }
}

async fn download_one(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    attachment: Attachment,
    root: &Path,
    session: &str,
) -> Option<PathBuf> {
    let mut url = reqwest::Url::parse(base_url).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.username() != "" || url.password().is_some()
    {
        return None;
    }
    {
        let mut segments = url.path_segments_mut().ok()?;
        segments
            .pop_if_empty()
            .push("files")
            .push(&attachment.file_uuid)
            .push("content");
    }
    let response = client
        .get(url)
        .bearer_auth(token)
        .timeout(TIMEOUT)
        .send()
        .await
        .ok()?;
    if response.status() != reqwest::StatusCode::OK
        || response
            .content_length()
            .is_some_and(|size| size > MAX_FILE_BYTES as u64)
    {
        return None;
    }
    let mut data = Vec::new();
    let mut chunks = response.bytes_stream();
    while let Some(piece) = chunks.next().await {
        let chunk = piece.ok()?;
        if data.len().saturating_add(chunk.len()) > MAX_FILE_BYTES {
            return None;
        }
        data.extend_from_slice(&chunk);
    }
    let folder = root
        .join(".atlas")
        .join("uploads")
        .join(safe_session_id(session));
    fs::create_dir_all(&folder).await.ok()?;
    let prefix: String = attachment
        .file_uuid
        .chars()
        .take(8)
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let path = folder.join(format!(
        "{}-{}-{}",
        prefix,
        &uuid::Uuid::new_v4().simple().to_string()[..8],
        safe_filename(&attachment.file_name)
    ));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .await
        .ok()?;
    if file.write_all(&data).await.is_err() || file.flush().await.is_err() {
        let _ = fs::remove_file(&path).await;
        return None;
    }
    Some(path)
}

pub fn prepend_path_refs(content: &Value, prefix: &str) -> Value {
    if prefix.is_empty() {
        return content.clone();
    }
    if let Some(text) = content.as_str() {
        return json!(format!("{prefix}{text}"));
    }
    if let Some(blocks) = content.as_array() {
        let mut out = blocks.clone();
        if let Some(index) = out
            .iter()
            .rposition(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        {
            if let Some(text) = out[index].get("text").and_then(Value::as_str) {
                out[index]["text"] = json!(format!("{prefix}{text}"));
                return Value::Array(out);
            }
        }
        out.push(json!({"type":"text","text":prefix.trim_end()}));
        return Value::Array(out);
    }
    content.clone()
}

/// Best-effort: unknown token/config/network errors leave the user message intact.
/// The application supplies a provider-neutral endpoint and authentication;
/// no attachment URL is accepted from the inbound untrusted message itself.
pub async fn resolve_and_prepend(
    event: &Value,
    content: &Value,
    root: &Path,
    session: &str,
    credentials: Option<(&str, &str)>,
) -> Value {
    let attachments = extract(event);
    let Some((url, token)) = credentials.filter(|(u, t)| !u.is_empty() && !t.is_empty()) else {
        return content.clone();
    };
    if attachments.is_empty() {
        return content.clone();
    }
    let client = match reqwest::Client::builder().timeout(TIMEOUT).build() {
        Ok(client) => client,
        Err(_) => return content.clone(),
    };
    let results = join_all(
        attachments
            .into_iter()
            .map(|a| download_one(&client, url, token, a, root, session)),
    )
    .await;
    let paths: Vec<_> = results
        .into_iter()
        .flatten()
        .map(|p| {
            // Quoted @ references survive whitespace in the user's home directory.
            format!("@\"{}\"", p.display().to_string().replace('"', "\\\""))
        })
        .collect();
    if paths.is_empty() {
        return content.clone();
    }
    prepend_path_refs(content, &format!("{} ", paths.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    #[test]
    fn rejects_invalid_attachments_without_partial_results() {
        assert!(extract(&json!({"file_attachments":[{"file_uuid":"x","file_name":"ok"},{"file_uuid":12,"file_name":"bad"}]})).is_empty());
        assert!(extract(&json!({"file_attachments":"invalid"})).is_empty());
        assert!(
            extract(&json!({"file_attachments":[{"file_uuid":"x","file_name":"n"}]})).len() == 1
        );
        assert_eq!(safe_filename("../../some path/secret?.png"), "secret_.png");
        assert_eq!(safe_filename("..\\sub\\name.jpg"), "name.jpg");
        assert_eq!(safe_filename(".."), "attachment");
    }

    #[test]
    fn inserts_references_in_last_text_block_and_adds_when_absent() {
        let prefix = "@\"/tmp/a b\" ";
        assert_eq!(
            prepend_path_refs(&json!("query"), prefix),
            json!(format!("{prefix}query"))
        );
        let blocks = json!([{"type":"text","text":"first"},{"type":"image","source":{"type":"base64","data":"AQID"}},{"type":"text","text":"last"}]);
        let value = prepend_path_refs(&blocks, prefix);
        assert_eq!(value[0]["text"], "first");
        assert_eq!(value[2]["text"], format!("{prefix}last"));
        let only_image = json!([{"type":"image","source":{"type":"base64","data":"AQID"}}]);
        assert_eq!(
            prepend_path_refs(&only_image, prefix)[1]["text"],
            "@\"/tmp/a b\""
        );
        assert_eq!(prepend_path_refs(&blocks, ""), blocks);
    }

    #[tokio::test]
    async fn downloads_only_from_configured_service_and_stores_in_session() {
        let temp = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let len = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..len]);
            assert!(
                request.starts_with("GET /files/abc123/content HTTP/1.1"),
                "{request}"
            );
            assert!(request
                .to_ascii_lowercase()
                .contains("authorization: bearer secret"));
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
                )
                .unwrap();
        });
        let event = json!({"file_attachments":[{"file_uuid":"abc123","file_name":"../../hello world.txt"}]});
        let result = resolve_and_prepend(
            &event,
            &json!("question"),
            temp.path(),
            "one",
            Some((&base, "secret")),
        )
        .await;
        server.join().unwrap();
        let text = result.as_str().unwrap();
        assert!(
            text.contains("hello_world.txt") && text.ends_with("question"),
            "{text}"
        );
        let files = std::fs::read_dir(temp.path().join(".atlas/uploads/one"))
            .unwrap()
            .collect::<Vec<_>>();
        assert_eq!(files.len(), 1);
        assert_eq!(
            std::fs::read(files[0].as_ref().unwrap().path()).unwrap(),
            b"hello"
        );
        assert_eq!(
            resolve_and_prepend(&event, &json!("question"), temp.path(), "two", None).await,
            json!("question")
        );
        assert!(!temp.path().join(".atlas/uploads/two").exists());
    }
}
