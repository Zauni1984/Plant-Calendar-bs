//! Where photo bytes live.
//!
//! Files are kept on disk under `PHOTO_STORAGE_DIR` (default `./data/photos`),
//! one directory per user: `<root>/<user_id>/<photo_id>.webp`. Nothing here is
//! web-served; every read goes through an authenticated handler.
//!
//! Per-user directories make account erasure (GDPR Art. 17) a single
//! directory removal instead of a lookup of every row.
//!
//! On a multi-server or container deployment this should become an object
//! store; keeping the interface this small is what makes that a local change.

use std::path::{Path, PathBuf};

use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct PhotoStore {
    root: PathBuf,
}

impl PhotoStore {
    pub fn from_env() -> Self {
        let root = std::env::var("PHOTO_STORAGE_DIR").unwrap_or_else(|_| "./data/photos".to_string());
        Self::new(root)
    }

    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn user_dir(&self, user_id: Uuid) -> PathBuf {
        // Uuid's Display is a fixed hex/hyphen format, so this can never
        // contain a path separator or "..".
        self.root.join(user_id.to_string())
    }

    pub fn path(&self, user_id: Uuid, photo_id: Uuid) -> PathBuf {
        self.user_dir(user_id).join(format!("{photo_id}.webp"))
    }

    /// Write atomically: a crash mid-write must never leave a truncated file
    /// that a later read would serve as a broken image.
    pub async fn put(&self, user_id: Uuid, photo_id: Uuid, bytes: &[u8]) -> std::io::Result<()> {
        let dir = self.user_dir(user_id);
        tokio::fs::create_dir_all(&dir).await?;
        let final_path = self.path(user_id, photo_id);
        let tmp_path = dir.join(format!("{photo_id}.webp.tmp"));
        tokio::fs::write(&tmp_path, bytes).await?;
        tokio::fs::rename(&tmp_path, &final_path).await
    }

    pub async fn get(&self, user_id: Uuid, photo_id: Uuid) -> std::io::Result<Vec<u8>> {
        tokio::fs::read(self.path(user_id, photo_id)).await
    }

    /// Deleting something already gone counts as success.
    pub async fn delete(&self, user_id: Uuid, photo_id: Uuid) -> std::io::Result<()> {
        ignore_not_found(tokio::fs::remove_file(self.path(user_id, photo_id)).await)
    }

    /// Remove every photo of a user.
    pub async fn delete_user(&self, user_id: Uuid) -> std::io::Result<()> {
        ignore_not_found(tokio::fs::remove_dir_all(self.user_dir(user_id)).await)
    }
}

fn ignore_not_found(r: std::io::Result<()>) -> std::io::Result<()> {
    match r {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn put_get_delete_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = PhotoStore::new(dir.path());
        let (user, photo) = (Uuid::new_v4(), Uuid::new_v4());

        store.put(user, photo, b"RIFFxxxxWEBP").await.unwrap();
        assert_eq!(store.get(user, photo).await.unwrap(), b"RIFFxxxxWEBP");
        // no temp file left behind
        let leftovers: Vec<_> = std::fs::read_dir(dir.path().join(user.to_string())).unwrap().collect();
        assert_eq!(leftovers.len(), 1);

        store.delete(user, photo).await.unwrap();
        assert!(store.get(user, photo).await.is_err());
        store.delete(user, photo).await.unwrap(); // idempotent
    }

    #[tokio::test]
    async fn delete_user_removes_everything_and_only_that_user() {
        let dir = tempfile::tempdir().unwrap();
        let store = PhotoStore::new(dir.path());
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        let (pa, pb) = (Uuid::new_v4(), Uuid::new_v4());
        store.put(a, pa, b"a").await.unwrap();
        store.put(b, pb, b"b").await.unwrap();

        store.delete_user(a).await.unwrap();
        assert!(store.get(a, pa).await.is_err());
        assert_eq!(store.get(b, pb).await.unwrap(), b"b");
        store.delete_user(a).await.unwrap(); // idempotent
    }
}
