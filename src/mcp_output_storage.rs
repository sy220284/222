//! Lossless MCP/WebFetch output persistence from the original output utility.
use anyhow::{bail, Context, Result};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy)]
pub enum ResultType {
    ToolResult,
    StructuredContent,
    ContentArray,
}
pub fn get_format_description(kind: ResultType, schema: Option<&str>) -> String {
    match kind {
        ResultType::ToolResult => "Plain text".into(),
        ResultType::StructuredContent => schema
            .filter(|s| !s.is_empty())
            .map(|s| format!("JSON with schema: {s}"))
            .unwrap_or_else(|| "JSON".into()),
        ResultType::ContentArray => schema
            .filter(|s| !s.is_empty())
            .map(|s| format!("JSON array with schema: {s}"))
            .unwrap_or_else(|| "JSON array".into()),
    }
}
fn grouped(value: usize) -> String {
    let digits = value.to_string();
    let mut result = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result
}
pub fn get_large_output_instructions(
    path: &str,
    length: usize,
    format: &str,
    max_read_length: Option<usize>,
) -> String {
    let base=format!("Error: result ({} characters) exceeds maximum allowed tokens. Output has been saved to {path}.\nFormat: {format}\nUse offset and limit parameters to read specific portions of the file, search within it for specific content, and jq to make structured queries.\nREQUIREMENTS FOR SUMMARIZATION/ANALYSIS/REVIEW:\n- You MUST read the content from the file at {path} in sequential chunks until 100% of the content has been read.\n",grouped(length));
    let warning=match max_read_length.filter(|n|*n>0) {
        Some(max)=>format!("- If you receive truncation warnings when reading the file (\"[N lines truncated]\"), reduce the chunk size until you have read 100% of the content without truncation ***DO NOT PROCEED UNTIL YOU HAVE DONE THIS***. Bash output is limited to {} chars.\n",grouped(max)),
        None=>"- If you receive truncation warnings when reading the file, reduce the chunk size until you have read 100% of the content without truncation.\n".into(),
    };
    base+&warning+"- Before producing ANY summary or analysis, you MUST explicitly describe what portion of the content you have read. ***If you did not read the entire content, you MUST explicitly state this.***\n"
}
pub fn extension_for_mime_type(mime: Option<&str>) -> &'static str {
    match mime
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "application/pdf" => "pdf",
        "application/json" => "json",
        "text/csv" => "csv",
        "text/plain" => "txt",
        "text/html" => "html",
        "text/markdown" => "md",
        "application/zip" => "zip",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => "pptx",
        "application/msword" => "doc",
        "application/vnd.ms-excel" => "xls",
        "audio/mpeg" => "mp3",
        "audio/wav" => "wav",
        "audio/ogg" => "ogg",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        _ => "bin",
    }
}
pub fn is_binary_content_type(content_type: &str) -> bool {
    if content_type.is_empty() {
        return false;
    }
    let mime = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    !(mime.starts_with("text/")
        || mime.ends_with("+json")
        || mime == "application/json"
        || mime.ends_with("+xml")
        || mime == "application/xml"
        || mime.starts_with("application/javascript")
        || mime == "application/x-www-form-urlencoded")
}
pub struct PersistedBinary {
    pub filepath: PathBuf,
    pub size: usize,
    pub ext: &'static str,
}
pub fn tool_results_directory(root: &Path) -> Result<PathBuf> {
    let root = root
        .canonicalize()
        .context("Cannot resolve tool workspace")?;
    let mut directory = root.clone();
    for component in [".atlas", "tool-results"] {
        directory = directory.join(component);
        fs::create_dir_all(&directory).context("Cannot create tool-results directory")?;
        directory = directory.canonicalize()?;
        if !directory.starts_with(&root) {
            bail!("Tool results directory escapes workspace");
        }
    }
    Ok(directory)
}
pub fn persist_binary_content(
    root: &Path,
    bytes: &[u8],
    mime: Option<&str>,
    persist_id: &str,
) -> Result<PersistedBinary> {
    if persist_id.is_empty()
        || !persist_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        bail!("Tool result ID must be a single safe filename component");
    }
    let directory = tool_results_directory(root)?;
    let ext = extension_for_mime_type(mime);
    let filepath = directory.join(format!("{persist_id}.{ext}"));
    let temporary = directory.join(format!(".output-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &filepath)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result?;
    Ok(PersistedBinary {
        filepath,
        size: bytes.len(),
        ext,
    })
}
pub fn get_binary_blob_saved_message(
    path: &str,
    mime: Option<&str>,
    size: usize,
    source: &str,
) -> String {
    format!(
        "{source}Binary content ({}, {}) saved to {path}",
        mime.filter(|s| !s.is_empty()).unwrap_or("unknown type"),
        crate::display_format::format_file_size(size as f64)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn binary_detection_is_case_insensitive_and_does_not_confuse_office_with_xml() {
        assert!(!is_binary_content_type(
            "Application/Problem+JSON; charset=utf-8"
        ));
        assert!(!is_binary_content_type("TEXT/CSV"));
        assert!(is_binary_content_type(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        ));
        assert_eq!(
            extension_for_mime_type(Some("APPLICATION/PDF; charset=binary")),
            "pdf"
        );
        assert_eq!(extension_for_mime_type(Some("application/json")), "json");
        assert_eq!(extension_for_mime_type(None), "bin");
    }
    #[test]
    fn binary_persistence_preserves_bytes_and_rejects_path_escape() {
        let root = tempfile::tempdir().unwrap();
        let saved =
            persist_binary_content(root.path(), b"\0\xffPDF", Some("application/pdf"), "file-1")
                .unwrap();
        assert_eq!(std::fs::read(&saved.filepath).unwrap(), b"\0\xffPDF");
        assert_eq!(saved.ext, "pdf");
        assert!(persist_binary_content(root.path(), b"bad", None, "../outside").is_err());
    }
    #[cfg(unix)]
    #[test]
    fn escaped_results_directory_does_not_create_or_write_outside_workspace() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), root.path().join(".atlas")).unwrap();
        assert!(persist_binary_content(root.path(), b"private", None, "result").is_err());
        assert!(!outside.path().join("tool-results").exists());
    }
    #[test]
    fn output_instructions_require_complete_chunked_inspection() {
        let text = get_large_output_instructions("result.txt", 12345, "JSON", Some(2000));
        assert!(text.contains("12,345 characters"));
        assert!(text.contains("100%"));
        assert!(text.contains("2,000 chars"));
        assert_eq!(
            get_format_description(ResultType::ContentArray, None),
            "JSON array"
        );
        assert_eq!(
            get_binary_blob_saved_message("x.pdf", Some("application/pdf"), 1536, "WebFetch: "),
            "WebFetch: Binary content (application/pdf, 1.5KB) saved to x.pdf"
        );
    }
}
