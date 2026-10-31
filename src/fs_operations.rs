//! Filesystem path identity and bounded reads shared by the permission engine.
use std::{
    collections::HashSet,
    fs, io,
    io::BufRead,
    path::{Path, PathBuf},
};

pub struct ResolvedPath {
    pub path: PathBuf,
    pub is_symlink: bool,
    pub is_canonical: bool,
}
fn network_path(path: &Path) -> bool {
    let text = path.as_os_str().to_string_lossy();
    text.starts_with("//") || text.starts_with(r"\\")
}
fn special(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        let t = metadata.file_type();
        t.is_fifo() || t.is_socket() || t.is_char_device() || t.is_block_device()
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        false
    }
}
pub fn safe_resolve_path(path: &Path) -> ResolvedPath {
    let fallback = || ResolvedPath {
        path: path.to_path_buf(),
        is_symlink: false,
        is_canonical: false,
    };
    if network_path(path) {
        return fallback();
    }
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return fallback();
    };
    if special(&metadata) {
        return fallback();
    }
    match path.canonicalize() {
        Ok(resolved) => ResolvedPath {
            is_symlink: resolved != path,
            path: resolved,
            is_canonical: true,
        },
        Err(_) => fallback(),
    }
}
pub fn is_duplicate_path(path: &Path, loaded: &mut HashSet<PathBuf>) -> bool {
    !loaded.insert(safe_resolve_path(path).path)
}
fn lexical(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                result.pop();
            }
            _ => result.push(part.as_os_str()),
        }
    }
    result
}
pub fn resolve_deepest_existing_ancestor(path: &Path) -> io::Result<Option<PathBuf>> {
    if network_path(path) {
        return Ok(None);
    }
    let mut current = path;
    let mut segments = Vec::new();
    while let Some(parent) = current.parent() {
        match fs::symlink_metadata(current) {
            Err(_) => {
                if let Some(name) = current.file_name() {
                    segments.push(name.to_os_string());
                }
                current = parent;
                continue;
            }
            Ok(metadata) => {
                let resolved = if metadata.file_type().is_symlink() {
                    match current.canonicalize() {
                        Ok(path) => Some(path),
                        Err(_) => {
                            let target = fs::read_link(current)?;
                            Some(lexical(&if target.is_absolute() {
                                target
                            } else {
                                parent.join(target)
                            }))
                        }
                    }
                } else {
                    current
                        .canonicalize()
                        .ok()
                        .filter(|resolved| resolved != current)
                };
                return Ok(resolved.map(|mut path| {
                    for segment in segments.iter().rev() {
                        path.push(segment);
                    }
                    path
                }));
            }
        }
    }
    Ok(None)
}
/// Preserve every spelling relevant to deny/ask rules, including intermediate
/// symlinks and the actual destination of a file that has not been created yet.
pub fn permission_paths(path: &Path) -> Vec<PathBuf> {
    let mut result = vec![path.to_path_buf()];
    if network_path(path) {
        return result;
    }
    let mut current = path.to_path_buf();
    let mut seen = HashSet::new();
    for _ in 0..40 {
        if !seen.insert(current.clone()) {
            break;
        }
        if !current.exists() {
            if current == path {
                if let Ok(Some(resolved)) = resolve_deepest_existing_ancestor(path) {
                    if !result.contains(&resolved) {
                        result.push(resolved);
                    }
                }
            }
            break;
        }
        let Ok(metadata) = fs::symlink_metadata(&current) else {
            break;
        };
        if special(&metadata) || !metadata.file_type().is_symlink() {
            break;
        }
        let Ok(target) = fs::read_link(&current) else {
            break;
        };
        current = lexical(&if target.is_absolute() {
            target
        } else {
            current.parent().unwrap_or(Path::new("")).join(target)
        });
        if !result.contains(&current) {
            result.push(current.clone());
        }
    }
    let resolved = safe_resolve_path(path);
    if resolved.is_symlink && !result.contains(&resolved.path) {
        result.push(resolved.path);
    }
    result
}

/// Source utils/readFileInRange.ts: stream the full file to count lines,
/// but keep only the requested line window. Byte limits constrain the
/// selected *content*, not the formatted line-number prefixes.
#[derive(Debug, PartialEq, Eq)]
pub struct TextLineWindow {
    pub formatted: String,
    pub selected_lines: usize,
    pub total_lines: usize,
    pub total_bytes: u64,
    pub read_bytes: usize,
    pub truncated_by_bytes: bool,
}

fn append_selected_line(
    number: usize,
    bytes: &[u8],
    content: &mut String,
    count: &mut usize,
    read_bytes: &mut usize,
    max_bytes: Option<usize>,
) -> bool {
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    let bytes = if number == 1 {
        bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes)
    } else {
        bytes
    };
    let text = String::from_utf8_lossy(bytes);
    let separator = usize::from(*count > 0);
    let next_bytes = read_bytes
        .saturating_add(separator)
        .saturating_add(text.len());
    if max_bytes.is_some_and(|limit| next_bytes > limit) {
        return false;
    }
    if separator != 0 {
        content.push('\n');
    }
    content.push_str(&number.to_string());
    content.push_str(": ");
    content.push_str(&text);
    *read_bytes = next_bytes;
    *count += 1;
    true
}

pub fn read_text_line_window(
    path: &Path,
    start: usize,
    count: usize,
) -> io::Result<TextLineWindow> {
    read_text_line_window_with_limit(path, start, count, None, false)
}

/// Source semantics: non-truncating maxBytes rejects oversized source files;
/// truncation retains complete selected lines up to the UTF-8 byte budget,
/// continues counting all source lines, and marks truncated_by_bytes.
pub fn read_text_line_window_with_limit(
    path: &Path,
    start: usize,
    count: usize,
    max_bytes: Option<usize>,
    truncate_on_byte_limit: bool,
) -> io::Result<TextLineWindow> {
    let file = fs::File::open(path)?;
    let file_meta = file.metadata()?;
    if !truncate_on_byte_limit {
        if let Some(limit) = max_bytes {
            if file_meta.is_file() && file_meta.len() > limit as u64 {
                return Err(io::Error::new(
                    io::ErrorKind::FileTooLarge,
                    format!(
                        "File content ({}) exceeds maximum allowed size ({limit})",
                        file_meta.len()
                    ),
                ));
            }
        }
    }
    let mut reader = io::BufReader::new(file);
    let has_bom = reader.fill_buf()?.starts_with(&[0xef, 0xbb, 0xbf]);
    let end = start.saturating_add(count);
    let mut formatted = String::new();
    let mut selected_bytes = Vec::new();
    let mut selected_lines = 0;
    let mut read_bytes = 0;
    let mut total_bytes = 0_u64;
    let mut line_number = 1_usize;
    let mut truncated_by_bytes = false;
    let output_limit = if truncate_on_byte_limit {
        max_bytes
    } else {
        None
    };
    loop {
        let chunk = reader.fill_buf()?;
        if chunk.is_empty() {
            break;
        }
        let chunk_len = chunk.len();
        let mut segment_start = 0;
        for (index, byte) in chunk.iter().enumerate() {
            if *byte != b'\n' {
                continue;
            }
            if !truncated_by_bytes && line_number >= start && line_number < end {
                selected_bytes.extend_from_slice(&chunk[segment_start..index]);
                if !append_selected_line(
                    line_number,
                    &selected_bytes,
                    &mut formatted,
                    &mut selected_lines,
                    &mut read_bytes,
                    output_limit,
                ) {
                    truncated_by_bytes = true;
                }
                selected_bytes.clear();
            }
            line_number = line_number.saturating_add(1);
            segment_start = index + 1;
        }
        if !truncated_by_bytes && line_number >= start && line_number < end {
            selected_bytes.extend_from_slice(&chunk[segment_start..]);
            if let Some(limit) = output_limit {
                // Allow BOM and CRLF overhead until the complete line is
                // known. This bounds a selected line whose size exceeds its
                // remaining byte allowance while preserving exact fitting.
                let allowance = limit.saturating_sub(read_bytes);
                if selected_bytes.len() > allowance.saturating_add(4) {
                    truncated_by_bytes = true;
                    selected_bytes.clear();
                }
            }
        } else {
            selected_bytes.clear();
        }
        total_bytes += chunk_len as u64;
        if !truncate_on_byte_limit {
            if let Some(limit) = max_bytes {
                if total_bytes.saturating_sub(if has_bom { 3 } else { 0 }) > limit as u64 {
                    return Err(io::Error::new(
                        io::ErrorKind::FileTooLarge,
                        "File exceeds maximum allowed size",
                    ));
                }
            }
        }
        reader.consume(chunk_len);
    }
    // A trailing LF adds an empty final line in the original JS fast and
    // streaming paths, even when the preceding line was the last nonempty.
    if !truncated_by_bytes && line_number >= start && line_number < end {
        if !append_selected_line(
            line_number,
            &selected_bytes,
            &mut formatted,
            &mut selected_lines,
            &mut read_bytes,
            output_limit,
        ) {
            truncated_by_bytes = true;
        }
    }
    Ok(TextLineWindow {
        formatted,
        selected_lines,
        total_lines: line_number,
        total_bytes: total_bytes.saturating_sub(if has_bom { 3 } else { 0 }),
        read_bytes,
        truncated_by_bytes,
    })
}

pub struct ReadFileRangeResult {
    pub content: String,
    pub bytes_read: usize,
    pub bytes_total: u64,
}
pub fn read_file_range(
    path: &Path,
    offset: u64,
    max_bytes: u64,
) -> io::Result<Option<ReadFileRangeResult>> {
    let mut file = fs::File::open(path)?;
    let size = file.metadata()?.len();
    if size <= offset {
        return Ok(None);
    }
    read_range_from(&mut file, offset, max_bytes).map(Some)
}
pub fn read_range_from(
    file: &mut fs::File,
    offset: u64,
    max_bytes: u64,
) -> io::Result<ReadFileRangeResult> {
    use std::io::{Read, Seek, SeekFrom};
    let size = file.metadata()?.len();
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = Vec::new();
    file.take(max_bytes.min(size.saturating_sub(offset)))
        .read_to_end(&mut bytes)?;
    Ok(ReadFileRangeResult {
        content: String::from_utf8_lossy(&bytes).into_owned(),
        bytes_read: bytes.len(),
        bytes_total: size,
    })
}
pub fn tail_file(path: &Path, max_bytes: u64) -> io::Result<ReadFileRangeResult> {
    let mut file = fs::File::open(path)?;
    let offset = file.metadata()?.len().saturating_sub(max_bytes);
    read_range_from(&mut file, offset, max_bytes)
}
/// Own the file so an early consumer exit closes it immediately. Carry bytes
/// across chunks before decoding, including code points split at 4096 bytes.
pub struct ReverseLines {
    file: fs::File,
    position: u64,
    remainder: Vec<u8>,
    ready: Vec<String>,
    done: bool,
}
impl ReverseLines {
    pub fn from_file(file: fs::File) -> io::Result<Self> {
        let position = file.metadata()?.len();
        Ok(Self {
            file,
            position,
            remainder: Vec::new(),
            ready: Vec::new(),
            done: false,
        })
    }
    pub fn open(path: &Path) -> io::Result<Self> {
        Self::from_file(fs::File::open(path)?)
    }
}
impl Iterator for ReverseLines {
    type Item = io::Result<String>;
    fn next(&mut self) -> Option<Self::Item> {
        use std::io::{Read, Seek, SeekFrom};
        loop {
            if let Some(line) = self.ready.pop() {
                return Some(Ok(line));
            }
            if self.done {
                return None;
            }
            if self.position == 0 {
                self.done = true;
                return if self.remainder.is_empty() {
                    None
                } else {
                    Some(Ok(String::from_utf8_lossy(&std::mem::take(
                        &mut self.remainder,
                    ))
                    .into_owned()))
                };
            }
            let size = self.position.min(4096) as usize;
            self.position -= size as u64;
            let mut bytes = vec![0; size];
            if let Err(error) = self
                .file
                .seek(SeekFrom::Start(self.position))
                .and_then(|_| self.file.read_exact(&mut bytes))
            {
                self.done = true;
                return Some(Err(error));
            }
            bytes.append(&mut self.remainder);
            if let Some(first) = bytes.iter().position(|b| *b == b'\n') {
                self.remainder = bytes[..first].to_vec();
                self.ready = bytes[first + 1..]
                    .split(|b| *b == b'\n')
                    .filter(|line| !line.is_empty())
                    .map(|line| String::from_utf8_lossy(line).into_owned())
                    .collect();
            } else {
                self.remainder = bytes;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_range_byte_limit_does_not_split_utf8_or_lines() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("unicode.txt");
        fs::write(&path, "zero\n你好\nlast\n").unwrap();

        let limited = read_text_line_window_with_limit(&path, 2, 3, Some(6), true).unwrap();
        assert_eq!(limited.formatted, "2: 你好");
        assert_eq!(limited.selected_lines, 1);
        assert_eq!(limited.read_bytes, 6);
        assert_eq!(limited.total_lines, 4);
        assert!(limited.truncated_by_bytes);

        let fits_separator = read_text_line_window_with_limit(&path, 1, 2, Some(11), true).unwrap();
        assert_eq!(fits_separator.formatted, "1: zero\n2: 你好");
        assert_eq!(fits_separator.read_bytes, 11);
        assert!(!fits_separator.truncated_by_bytes);

        let denied = read_text_line_window_with_limit(&path, 2, 1, Some(11), false).unwrap_err();
        assert_eq!(denied.kind(), io::ErrorKind::FileTooLarge);
    }

    #[test]
    fn source_range_truncation_bounds_huge_selected_lines_and_counts_rest() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("long.txt");
        let mut data = vec![b'x'; 2_000_000];
        data.extend_from_slice(b"\nend\n");
        fs::write(&path, data).unwrap();

        let limited = read_text_line_window_with_limit(&path, 1, 3, Some(3), true).unwrap();
        assert!(limited.truncated_by_bytes);
        assert_eq!(limited.formatted, "");
        assert_eq!(limited.selected_lines, 0);
        assert_eq!(limited.total_lines, 3);
        assert_eq!(limited.total_bytes, fs::metadata(&path).unwrap().len());

        let out_of_range = read_text_line_window_with_limit(&path, 9, 1, Some(0), true).unwrap();
        assert!(!out_of_range.truncated_by_bytes);
        assert_eq!(out_of_range.total_lines, 3);
    }

    #[test]
    fn line_window_matches_source_crlf_bom_unicode_and_terminal_empty_line() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("source.txt");
        fs::write(&path, b"\xef\xbb\xbfheader\r\n\xe4\xbd\xa0\xe5\xa5\xbd\r\n").unwrap();
        let first = read_text_line_window(&path, 1, 1).unwrap();
        assert_eq!(first.formatted, "1: header");
        assert_eq!(first.total_lines, 3);
        let end = read_text_line_window(&path, 2, 2).unwrap();
        assert_eq!(end.formatted, "2: 你好\n3: ");
        assert_eq!(end.selected_lines, 2);
        assert_eq!(end.read_bytes, "你好\n".len());
        assert_eq!(end.total_bytes, fs::metadata(&path).unwrap().len() - 3);
        assert_eq!(
            read_text_line_window(&path, 5, 2).unwrap().selected_lines,
            0
        );
    }

    #[test]
    fn line_window_skips_huge_unselected_line_without_materializing_it() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("large.txt");
        let mut data = vec![b'x'; 2_000_000];
        data.extend_from_slice(b"\nend\n");
        fs::write(&path, data).unwrap();
        let result = read_text_line_window(&path, 2, 2).unwrap();
        assert_eq!(result.formatted, "2: end\n3: ");
        assert_eq!(result.total_lines, 3);
    }

    #[cfg(unix)]
    #[test]
    fn permission_paths_include_each_link_and_uncreated_destination() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        std::fs::write(root.join("final"), "x").unwrap();
        symlink("final", root.join("middle")).unwrap();
        symlink("middle", root.join("alias")).unwrap();
        assert_eq!(
            permission_paths(&root.join("alias")),
            vec![root.join("alias"), root.join("middle"), root.join("final")]
        );
        symlink("missing", root.join("dangling")).unwrap();
        assert_eq!(
            resolve_deepest_existing_ancestor(&root.join("dangling/child")).unwrap(),
            Some(root.join("missing/child"))
        );
        assert!(
            permission_paths(&root.join("dangling/child")).contains(&root.join("missing/child"))
        );
        let mut loaded = std::collections::HashSet::new();
        assert!(!is_duplicate_path(&root.join("alias"), &mut loaded));
        assert!(is_duplicate_path(&root.join("final"), &mut loaded));
    }
    #[test]
    fn bounded_ranges_and_reverse_lines_preserve_utf8_across_chunks() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("history");
        let long = format!("{}🦀suffix", "x".repeat(4093));
        let contents = format!("first\n\n{long}\nlast\n");
        std::fs::write(&path, &contents).unwrap();
        let lines = ReverseLines::open(&path)
            .unwrap()
            .collect::<io::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(lines, vec!["last".to_owned(), long, "first".to_owned()]);
        let range = read_file_range(&path, 0, 5).unwrap().unwrap();
        assert_eq!(range.content, "first");
        assert_eq!(range.bytes_read, 5);
        assert_eq!(range.bytes_total, contents.len() as u64);
        assert!(read_file_range(&path, contents.len() as u64, 5)
            .unwrap()
            .is_none());
        assert_eq!(tail_file(&path, 5).unwrap().content, "last\n");
    }
    #[test]
    fn nonexistent_and_network_paths_are_not_canonical() {
        let root = tempfile::tempdir().unwrap();
        let p = root.path().join("missing");
        let result = safe_resolve_path(&p);
        assert_eq!(result.path, p);
        assert!(!result.is_canonical);
        assert!(!safe_resolve_path(std::path::Path::new("//invalid/share")).is_canonical);
    }
}
