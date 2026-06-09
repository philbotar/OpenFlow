use std::path::Path;

pub trait PatchFileSystem: Send + Sync {
    fn read(&self, path: &Path) -> std::io::Result<String>;
    fn write(&self, path: &Path, content: &str) -> std::io::Result<()>;
    fn exists(&self, path: &Path) -> bool;
}

pub trait SnapshotStore: Send + Sync {
    fn get(&self, path: &Path) -> Option<String>;
    fn set(&self, path: &Path, content: String);
    fn clear(&self, path: &Path);
}

pub trait HashlineFilesystem: Send + Sync {
    fn read(&self, path: &Path) -> std::io::Result<String>;
    fn write(&self, path: &Path, content: &str) -> std::io::Result<()>;
    fn exists(&self, path: &Path) -> bool;
}
