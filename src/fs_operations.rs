//! Filesystem path identity and bounded reads shared by the permission engine.
use std::{
    collections::HashSet,
    fs, io,
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
