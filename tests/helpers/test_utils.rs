use std::ops::Deref;
use std::path::PathBuf;

/// Wrapper around a path that will get destroyed on drop
pub(crate) struct TempDir {
    path: PathBuf,
}

/// Delete underlying directory on drop
impl Drop for TempDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).unwrap();
    }
}

/// Deref to wrapped path for more ergonomic usage
impl Deref for TempDir {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

/// Create a new temporary directory with a random name for testing purposes
pub(crate) fn temp_dir() -> TempDir {
    let unique_id: u32 = rand::random();
    let temp_dir = std::env::temp_dir().join(format!("oxidux_temp_{}", unique_id));
    std::fs::create_dir(&temp_dir).unwrap();

    TempDir { path: temp_dir }
}
