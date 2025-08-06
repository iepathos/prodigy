//! Mock file system operations for testing

use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Mock file system for testing
pub struct MockFileSystem {
    files: Arc<Mutex<HashMap<PathBuf, String>>>,
    directories: Arc<Mutex<Vec<PathBuf>>>,
    read_errors: Arc<Mutex<HashMap<PathBuf, String>>>,
    write_errors: Arc<Mutex<HashMap<PathBuf, String>>>,
}

impl MockFileSystem {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
            directories: Arc::new(Mutex::new(Vec::new())),
            read_errors: Arc::new(Mutex::new(HashMap::new())),
            write_errors: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn builder() -> MockFileSystemBuilder {
        MockFileSystemBuilder::new()
    }

    pub fn add_file(&self, path: impl AsRef<Path>, content: &str) {
        let mut files = self.files.lock().unwrap();
        files.insert(path.as_ref().to_path_buf(), content.to_string());
    }

    pub fn add_directory(&self, path: impl AsRef<Path>) {
        let mut dirs = self.directories.lock().unwrap();
        dirs.push(path.as_ref().to_path_buf());
    }

    pub fn set_read_error(&self, path: impl AsRef<Path>, error: &str) {
        let mut errors = self.read_errors.lock().unwrap();
        errors.insert(path.as_ref().to_path_buf(), error.to_string());
    }

    pub fn set_write_error(&self, path: impl AsRef<Path>, error: &str) {
        let mut errors = self.write_errors.lock().unwrap();
        errors.insert(path.as_ref().to_path_buf(), error.to_string());
    }

    pub fn read_file(&self, path: impl AsRef<Path>) -> Result<String> {
        let path = path.as_ref();

        // Check for read errors
        let errors = self.read_errors.lock().unwrap();
        if let Some(error) = errors.get(path) {
            return Err(anyhow::anyhow!(error.clone()));
        }

        // Read file content
        let files = self.files.lock().unwrap();
        files
            .get(path)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("File not found: {:?}", path))
    }

    pub fn write_file(&self, path: impl AsRef<Path>, content: &str) -> Result<()> {
        let path = path.as_ref();

        // Check for write errors
        let errors = self.write_errors.lock().unwrap();
        if let Some(error) = errors.get(path) {
            return Err(anyhow::anyhow!(error.clone()));
        }

        // Write file content
        let mut files = self.files.lock().unwrap();
        files.insert(path.to_path_buf(), content.to_string());
        Ok(())
    }

    pub fn exists(&self, path: impl AsRef<Path>) -> bool {
        let path = path.as_ref();
        let files = self.files.lock().unwrap();
        let dirs = self.directories.lock().unwrap();
        files.contains_key(path) || dirs.contains(&path.to_path_buf())
    }

    pub fn is_file(&self, path: impl AsRef<Path>) -> bool {
        let files = self.files.lock().unwrap();
        files.contains_key(path.as_ref())
    }

    pub fn is_dir(&self, path: impl AsRef<Path>) -> bool {
        let dirs = self.directories.lock().unwrap();
        dirs.contains(&path.as_ref().to_path_buf())
    }

    pub fn list_dir(&self, path: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
        let path = path.as_ref();

        if !self.is_dir(path) {
            return Err(anyhow::anyhow!("Not a directory: {:?}", path));
        }

        let files = self.files.lock().unwrap();
        let dirs = self.directories.lock().unwrap();

        let mut entries = Vec::new();

        // Find all files in this directory
        for file_path in files.keys() {
            if let Some(parent) = file_path.parent() {
                if parent == path {
                    entries.push(file_path.clone());
                }
            }
        }

        // Find all subdirectories
        for dir_path in dirs.iter() {
            if let Some(parent) = dir_path.parent() {
                if parent == path {
                    entries.push(dir_path.clone());
                }
            }
        }

        Ok(entries)
    }

    pub fn get_all_files(&self) -> HashMap<PathBuf, String> {
        self.files.lock().unwrap().clone()
    }
}

/// Builder for creating configured mock file systems
pub struct MockFileSystemBuilder {
    fs: MockFileSystem,
}

impl MockFileSystemBuilder {
    pub fn new() -> Self {
        Self {
            fs: MockFileSystem::new(),
        }
    }

    pub fn with_file(self, path: impl AsRef<Path>, content: &str) -> Self {
        self.fs.add_file(path, content);
        self
    }

    pub fn with_directory(self, path: impl AsRef<Path>) -> Self {
        self.fs.add_directory(path);
        self
    }

    pub fn with_project_structure(self) -> Self {
        // Add common project structure
        self.fs.add_directory("src");
        self.fs.add_directory("tests");
        self.fs.add_directory("benches");
        self.fs.add_file(
            "Cargo.toml",
            r#"[package]
name = "test_project"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1.0"
"#,
        );
        self.fs.add_file(
            "src/main.rs",
            "fn main() {\n    println!(\"Hello, world!\");\n}",
        );
        self.fs
            .add_file("src/lib.rs", "pub fn lib_function() -> i32 {\n    42\n}");
        self
    }

    pub fn with_read_error(self, path: impl AsRef<Path>, error: &str) -> Self {
        self.fs.set_read_error(path, error);
        self
    }

    pub fn with_write_error(self, path: impl AsRef<Path>, error: &str) -> Self {
        self.fs.set_write_error(path, error);
        self
    }

    pub fn build(self) -> MockFileSystem {
        self.fs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_fs_basic_operations() {
        let fs = MockFileSystem::new();

        // Test file operations
        fs.add_file("test.txt", "Hello, world!");
        assert!(fs.exists("test.txt"));
        assert!(fs.is_file("test.txt"));
        assert!(!fs.is_dir("test.txt"));

        let content = fs.read_file("test.txt").unwrap();
        assert_eq!(content, "Hello, world!");

        // Test directory operations
        fs.add_directory("src");
        assert!(fs.exists("src"));
        assert!(fs.is_dir("src"));
        assert!(!fs.is_file("src"));
    }

    #[test]
    fn test_mock_fs_builder() {
        let fs = MockFileSystemBuilder::new()
            .with_file("README.md", "# Test Project")
            .with_directory("docs")
            .with_file("docs/guide.md", "# Guide")
            .build();

        assert!(fs.exists("README.md"));
        assert!(fs.exists("docs"));
        assert!(fs.exists("docs/guide.md"));

        let readme = fs.read_file("README.md").unwrap();
        assert_eq!(readme, "# Test Project");
    }

    #[test]
    fn test_mock_fs_errors() {
        let fs = MockFileSystemBuilder::new()
            .with_file("readonly.txt", "content")
            .with_read_error("forbidden.txt", "Permission denied")
            .with_write_error("readonly.txt", "Read-only file system")
            .build();

        // Test read error
        let error = fs.read_file("forbidden.txt").unwrap_err();
        assert!(error.to_string().contains("Permission denied"));

        // Test write error
        let error = fs.write_file("readonly.txt", "new content").unwrap_err();
        assert!(error.to_string().contains("Read-only file system"));

        // Can still read the file
        let content = fs.read_file("readonly.txt").unwrap();
        assert_eq!(content, "content");
    }

    #[test]
    fn test_mock_fs_project_structure() {
        let fs = MockFileSystemBuilder::new()
            .with_project_structure()
            .build();

        assert!(fs.exists("src"));
        assert!(fs.exists("tests"));
        assert!(fs.exists("Cargo.toml"));
        assert!(fs.exists("src/main.rs"));
        assert!(fs.exists("src/lib.rs"));

        let cargo = fs.read_file("Cargo.toml").unwrap();
        assert!(cargo.contains("test_project"));
    }

    #[test]
    fn test_mock_fs_list_dir() {
        let fs = MockFileSystemBuilder::new()
            .with_directory("src")
            .with_file("src/main.rs", "")
            .with_file("src/lib.rs", "")
            .with_directory("src/modules")
            .build();

        let entries = fs.list_dir("src").unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries.contains(&PathBuf::from("src/main.rs")));
        assert!(entries.contains(&PathBuf::from("src/lib.rs")));
        assert!(entries.contains(&PathBuf::from("src/modules")));
    }
}
