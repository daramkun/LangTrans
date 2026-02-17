use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub key: String,
    pub label: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked: bool,
}

impl ApiKey {
    pub fn is_expired(&self) -> bool {
        self.expires_at
            .map(|exp| Utc::now() > exp)
            .unwrap_or(false)
    }

    pub fn is_valid(&self) -> bool {
        !self.revoked && !self.is_expired()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiKeysFile {
    keys: Vec<ApiKey>,
}

pub struct ApiKeyStore {
    file_path: PathBuf,
    keys: Vec<ApiKey>,
}

impl ApiKeyStore {
    pub fn load_or_create(path: &Path) -> anyhow::Result<Self> {
        let keys = if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let file: ApiKeysFile = serde_json::from_str(&content)?;
            file.keys
        } else {
            Vec::new()
        };

        Ok(ApiKeyStore {
            file_path: path.to_path_buf(),
            keys,
        })
    }

    fn save(&self) -> anyhow::Result<()> {
        let file = ApiKeysFile {
            keys: self.keys.clone(),
        };
        let content = serde_json::to_string_pretty(&file)?;
        std::fs::write(&self.file_path, content)?;
        Ok(())
    }

    pub fn validate(&self, key: &str) -> bool {
        self.keys.iter().any(|k| k.key == key && k.is_valid())
    }

    pub fn add(
        &mut self,
        label: String,
        expires_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<ApiKey> {
        let api_key = ApiKey {
            key: uuid::Uuid::new_v4().to_string(),
            label,
            created_at: Utc::now(),
            expires_at,
            revoked: false,
        };
        self.keys.push(api_key.clone());
        self.save()?;
        Ok(api_key)
    }

    pub fn revoke(&mut self, key: &str) -> anyhow::Result<bool> {
        if let Some(api_key) = self.keys.iter_mut().find(|k| k.key == key) {
            api_key.revoked = true;
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn list(&self) -> &[ApiKey] {
        &self.keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    fn temp_path() -> PathBuf {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        drop(file);
        path
    }

    #[test]
    fn test_load_or_create_new() {
        let path = temp_path();
        let store = ApiKeyStore::load_or_create(&path).unwrap();
        assert!(store.list().is_empty());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_add_and_validate() {
        let path = temp_path();
        let mut store = ApiKeyStore::load_or_create(&path).unwrap();
        let key = store.add("test".to_string(), None).unwrap();
        assert!(store.validate(&key.key));
        assert!(!store.validate("nonexistent"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_revoke() {
        let path = temp_path();
        let mut store = ApiKeyStore::load_or_create(&path).unwrap();
        let key = store.add("test".to_string(), None).unwrap();
        assert!(store.validate(&key.key));
        store.revoke(&key.key).unwrap();
        assert!(!store.validate(&key.key));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_expired_key() {
        let path = temp_path();
        let mut store = ApiKeyStore::load_or_create(&path).unwrap();
        let past = Utc::now() - chrono::Duration::hours(1);
        let key = store.add("expired".to_string(), Some(past)).unwrap();
        assert!(!store.validate(&key.key));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_persistence() {
        let path = temp_path();
        let key_str;
        {
            let mut store = ApiKeyStore::load_or_create(&path).unwrap();
            let key = store.add("persistent".to_string(), None).unwrap();
            key_str = key.key;
        }
        let store2 = ApiKeyStore::load_or_create(&path).unwrap();
        assert!(store2.validate(&key_str));
        let _ = fs::remove_file(&path);
    }
}
