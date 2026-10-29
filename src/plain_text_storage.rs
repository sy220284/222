//! Provider-neutral plaintext credential fallback, based on
//! src/utils/secureStorage/plainTextStorage.ts (2.1.88).
use crate::secure_fallback::{SecureStorage, WriteResult};
use async_trait::async_trait;
use serde_json::Value;
use std::{fs, io::Write, path::PathBuf};
use uuid::Uuid;

pub struct PlainTextStorage {
    pub path: PathBuf,
}

impl PlainTextStorage {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_location() -> Option<Self> {
        dirs::config_dir().map(|dir| Self::new(dir.join("atlas-agent/.credentials.json")))
    }

    fn write_secure(&self, data: &Value) -> std::io::Result<()> {
        let dir = self.path.parent().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid credentials path")
        })?;
        fs::create_dir_all(dir)?;
        let temporary = dir.join(format!(".credentials-{}.tmp", Uuid::new_v4()));
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = opts.open(&temporary)?;
        let result = (|| -> std::io::Result<()> {
            let bytes = serde_json::to_vec(data).map_err(std::io::Error::other)?;
            file.write_all(&bytes)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, &self.path)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))?;
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

#[async_trait]
impl SecureStorage for PlainTextStorage {
    fn name(&self) -> &str {
        "plaintext"
    }

    fn read(&self) -> Option<Value> {
        serde_json::from_slice(&fs::read(&self.path).ok()?).ok()
    }

    async fn read_async(&self) -> Option<Value> {
        serde_json::from_slice(&tokio::fs::read(&self.path).await.ok()?).ok()
    }

    fn update(&self, data: Value) -> WriteResult {
        match self.write_secure(&data) {
            Ok(()) => WriteResult {
                success: true,
                warning: Some("Warning: Storing credentials in plaintext.".into()),
            },
            Err(_) => WriteResult::failure(),
        }
    }

    fn delete(&self) -> bool {
        match fs::remove_file(&self.path) {
            Ok(()) => true,
            Err(error) => error.kind() == std::io::ErrorKind::NotFound,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn credentials_roundtrip_async_and_sync_with_owner_only_file() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("config/credentials.json");
        let storage = PlainTextStorage::new(file.clone());
        assert_eq!(storage.read(), None);
        assert!(storage.update(json!({"refresh":"secret"})).success);
        assert_eq!(storage.read().unwrap()["refresh"], "secret");
        assert_eq!(storage.read_async().await.unwrap()["refresh"], "secret");
        assert!(storage.update(json!({"refresh":"rotated"})).success);
        assert_eq!(storage.read().unwrap()["refresh"], "rotated");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert!(storage.delete());
        assert!(storage.delete());
        assert!(storage.read().is_none());
    }
}
