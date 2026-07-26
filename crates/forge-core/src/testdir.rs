//! Scratch directories for tests.
//!
//! Working storage is project-relative `.tmp/`, never `/tmp` (I-8). Each
//! directory is removed before it is created, so a run that died mid-test
//! cannot leave remnants that make the next run pass or fail for the wrong
//! reason, and it is removed again on drop.

use std::path::{Path, PathBuf};

/// A uniquely-named scratch directory under the repository's `.tmp/`.
pub(crate) struct TestDir {
    path: PathBuf,
}

impl TestDir {
    /// Create (or re-create) a scratch directory named for the test using it.
    pub(crate) fn new(name: &str) -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(".tmp")
            .join("forge-core-tests");
        let path = root.join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create scratch directory");
        TestDir { path }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
