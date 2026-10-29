//! Primary/secondary credential store semantics ported from
//! src/utils/secureStorage/fallbackStorage.ts (2.1.88).
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteResult {
    pub success: bool,
    pub warning: Option<String>,
}

impl WriteResult {
    pub fn success() -> Self {
        Self { success: true, warning: None }
    }
    pub fn failure() -> Self {
        Self { success: false, warning: None }
    }
}

#[async_trait]
pub trait SecureStorage: Send + Sync {
    fn name(&self) -> &str;
    fn read(&self) -> Option<Value>;
    async fn read_async(&self) -> Option<Value>;
    fn update(&self, data: Value) -> WriteResult;
    fn delete(&self) -> bool;
}

pub struct FallbackStorage {
    pub name: String,
    primary: Arc<dyn SecureStorage>,
    secondary: Arc<dyn SecureStorage>,
}

impl FallbackStorage {
    pub fn new(
        primary: Arc<dyn SecureStorage>,
        secondary: Arc<dyn SecureStorage>,
    ) -> Self {
        let name = format!("{}-with-{}-fallback", primary.name(), secondary.name());
        Self { name, primary, secondary }
    }
}

#[async_trait]
impl SecureStorage for FallbackStorage {
    fn name(&self) -> &str {
        &self.name
    }

    fn read(&self) -> Option<Value> {
        self.primary.read().or_else(|| self.secondary.read())
            .or_else(|| Some(serde_json::json!({})))
    }

    async fn read_async(&self) -> Option<Value> {
        if let Some(value) = self.primary.read_async().await {
            return Some(value);
        }
        self.secondary.read_async().await.or_else(|| Some(serde_json::json!({})))
    }

    fn update(&self, data: Value) -> WriteResult {
        let primary_before = self.primary.read();
        let result = self.primary.update(data.clone());
        if result.success {
            // A newly working secure keychain supersedes previous fallback data.
            if primary_before.is_none() {
                self.secondary.delete();
            }
            return result;
        }
        let fallback = self.secondary.update(data);
        if fallback.success {
            // A stale, but readable, primary value must not shadow the
            // rotated refresh token that just landed in the secondary store.
            if primary_before.is_some() {
                self.primary.delete();
            }
            return WriteResult {
                success: true,
                warning: fallback.warning,
            };
        }
        WriteResult::failure()
    }

    fn delete(&self) -> bool {
        let primary_deleted = self.primary.delete();
        let secondary_deleted = self.secondary.delete();
        primary_deleted || secondary_deleted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{Mutex, atomic::{AtomicBool, Ordering}};

    struct MemoryStore {
        label: &'static str,
        data: Mutex<Option<Value>>,
        writable: AtomicBool,
    }

    impl MemoryStore {
        fn new(label: &'static str, value: Option<Value>, writable: bool) -> Self {
            Self {
                label,
                data: Mutex::new(value),
                writable: AtomicBool::new(writable),
            }
        }
    }

    #[async_trait]
    impl SecureStorage for MemoryStore {
        fn name(&self) -> &str { self.label }
        fn read(&self) -> Option<Value> { self.data.lock().unwrap().clone() }
        async fn read_async(&self) -> Option<Value> { self.read() }
        fn update(&self, data: Value) -> WriteResult {
            if !self.writable.load(Ordering::SeqCst) {
                return WriteResult::failure();
            }
            *self.data.lock().unwrap() = Some(data);
            WriteResult::success()
        }
        fn delete(&self) -> bool {
            self.data.lock().unwrap().take().is_some()
        }
    }

    #[tokio::test]
    async fn secondary_data_used_only_when_primary_is_absent() {
        let first = Arc::new(MemoryStore::new("keychain", None, false));
        let second = Arc::new(MemoryStore::new("fallback", Some(json!({"access":"secondary"})), true));
        let store = FallbackStorage::new(first.clone(), second.clone());
        assert_eq!(store.name(), "keychain-with-fallback-fallback");
        assert_eq!(store.read().unwrap()["access"], "secondary");
        assert_eq!(store.read_async().await.unwrap()["access"], "secondary");
        assert!(store.update(json!({"access":"rotated"})).success);
        assert_eq!(store.read().unwrap()["access"], "rotated");
        first.writable.store(true, Ordering::SeqCst);
        assert!(store.update(json!({"access":"primary"})).success);
        assert_eq!(store.read().unwrap()["access"], "primary");
        assert!(second.read().is_none());
    }

    #[test]
    fn fallback_write_removes_stale_primary_which_would_shadow_new_token() {
        let first = Arc::new(MemoryStore::new("keychain", Some(json!({"refresh":"old"})), false));
        let second = Arc::new(MemoryStore::new("file", None, true));
        let store = FallbackStorage::new(first.clone(), second.clone());
        assert!(store.update(json!({"refresh":"new"})).success);
        assert!(first.read().is_none());
        assert_eq!(store.read().unwrap()["refresh"], "new");
        assert!(store.delete());
        assert_eq!(store.read(), Some(json!({})));
    }
}
