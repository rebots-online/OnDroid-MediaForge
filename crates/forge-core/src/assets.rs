//! Content-addressed intermediate storage.
//!
//! Every segment output is written here under the SHA-256 of its own bytes, so
//! a resumed job that recomputes a segment lands on the same key and costs one
//! hash rather than one rewrite. `put` is idempotent by construction.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;

use crate::CoreError;

/// The SHA-256 of a payload, lower-case hex. Also the payload's address in the
/// store.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AssetKey(pub String);

impl AssetKey {
    /// The key a payload will have, without touching the filesystem.
    pub fn of(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let mut hex = String::with_capacity(64);
        for byte in digest {
            use std::fmt::Write as _;
            let _ = write!(hex, "{byte:02x}");
        }
        AssetKey(hex)
    }

    /// The key as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AssetKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Content-addressed intermediate storage rooted at a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetStore {
    pub root: PathBuf,
}

impl AssetStore {
    /// A store rooted at `root`. The directory is created on first `put`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        AssetStore { root: root.into() }
    }

    /// Where a key lives. Fanned out by the first two hex characters so a long
    /// job does not build one directory with tens of thousands of entries.
    pub fn path_for(&self, key: &AssetKey) -> PathBuf {
        self.root.join(&key.0[..2]).join(&key.0[2..])
    }

    /// Store `bytes` and return their key.
    ///
    /// Idempotent: content already present is not rewritten, and the existing
    /// key is returned. The write itself goes to a temporary file in the same
    /// directory and is renamed into place, so a process death mid-write
    /// cannot leave a truncated payload under a valid key.
    pub fn put(&self, bytes: &[u8]) -> Result<AssetKey, CoreError> {
        let key = AssetKey::of(bytes);
        let path = self.path_for(&key);
        if path.exists() {
            return Ok(key);
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let staging = path.with_extension("partial");
        std::fs::write(&staging, bytes)?;
        std::fs::rename(&staging, &path)?;
        Ok(key)
    }

    /// Read a payload back.
    pub fn get(&self, key: &AssetKey) -> Result<Vec<u8>, CoreError> {
        Ok(std::fs::read(self.path_for(key))?)
    }

    /// Whether a payload is already present.
    pub fn contains(&self, key: &AssetKey) -> bool {
        self.path_for(key).exists()
    }
}

/// Every regular file under `root`, recursively.
#[cfg(test)]
pub(crate) fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(files_under(&path));
        } else {
            out.push(path);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdir::TestDir;

    #[test]
    fn put_twice_yields_one_file_and_equal_keys() {
        let dir = TestDir::new("assets-idempotent");
        let store = AssetStore::new(dir.path());
        let payload = b"a segment of rendered audio".to_vec();

        let first = store.put(&payload).expect("first put");
        let second = store.put(&payload).expect("second put");

        assert_eq!(first, second);
        assert_eq!(
            files_under(dir.path()).len(),
            1,
            "storing identical content twice must leave one file"
        );
    }

    #[test]
    fn a_payload_reads_back_byte_for_byte() {
        let dir = TestDir::new("assets-roundtrip");
        let store = AssetStore::new(dir.path());
        let payload: Vec<u8> = (0..=255u8).cycle().take(4096).collect();

        let key = store.put(&payload).expect("put");
        assert_eq!(store.get(&key).expect("get"), payload);
        assert!(store.contains(&key));
    }

    #[test]
    fn different_content_takes_a_different_key() {
        let dir = TestDir::new("assets-distinct");
        let store = AssetStore::new(dir.path());

        let a = store.put(b"one").expect("put one");
        let b = store.put(b"two").expect("put two");

        assert_ne!(a, b);
        assert_eq!(files_under(dir.path()).len(), 2);
    }

    #[test]
    fn a_key_is_the_sha256_of_the_payload() {
        // The published SHA-256 of the empty input.
        assert_eq!(
            AssetKey::of(b"").as_str(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn reading_an_absent_key_is_an_error() {
        let dir = TestDir::new("assets-absent");
        let store = AssetStore::new(dir.path());
        let key = AssetKey::of(b"never stored");
        assert!(!store.contains(&key));
        assert!(store.get(&key).is_err());
    }
}
