//! Test helper functions and custom assertions

pub mod assertions;

use anyhow::Result;
use std::path::Path;
use tempfile::TempDir;

/// Create a temporary directory with a git repository initialized
pub async fn create_test_repo() -> Result<TempDir> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()?;

    // Configure git user
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(temp_dir.path())
        .output()?;

    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(temp_dir.path())
        .output()?;

    Ok(temp_dir)
}

/// Create a test file with content in a directory
pub fn create_file(dir: &Path, relative_path: &str, content: &str) -> Result<()> {
    let file_path = dir.join(relative_path);
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(file_path, content)?;
    Ok(())
}

/// Create a standard Rust project structure
pub fn create_rust_project(dir: &Path) -> Result<()> {
    // Create directories
    std::fs::create_dir_all(dir.join("src"))?;
    std::fs::create_dir_all(dir.join("tests"))?;

    // Create Cargo.toml
    create_file(
        dir,
        "Cargo.toml",
        r#"[package]
name = "test_project"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1.0"
"#,
    )?;

    // Create main.rs
    create_file(
        dir,
        "src/main.rs",
        "fn main() {\n    println!(\"Hello, world!\");\n}",
    )?;

    // Create lib.rs
    create_file(
        dir,
        "src/lib.rs",
        "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}",
    )?;

    Ok(())
}

/// Helper to run a command and capture output
pub async fn run_command(command: &str, args: &[&str], dir: &Path) -> Result<String> {
    let output = tokio::process::Command::new(command)
        .args(args)
        .current_dir(dir)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Command failed: {}", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Check if a file contains specific content
pub fn file_contains(path: &Path, content: &str) -> Result<bool> {
    let file_content = std::fs::read_to_string(path)?;
    Ok(file_content.contains(content))
}

/// Get the number of files in a directory (recursively)
pub fn count_files(dir: &Path) -> Result<usize> {
    let mut count = 0;
    for entry in walkdir::WalkDir::new(dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            count += 1;
        }
    }
    Ok(count)
}

/// Compare two directories for equality
pub fn dirs_equal(dir1: &Path, dir2: &Path) -> Result<bool> {
    use std::collections::HashSet;

    let files1: HashSet<_> = walkdir::WalkDir::new(dir1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().strip_prefix(dir1).unwrap().to_path_buf())
        .collect();

    let files2: HashSet<_> = walkdir::WalkDir::new(dir2)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().strip_prefix(dir2).unwrap().to_path_buf())
        .collect();

    if files1 != files2 {
        return Ok(false);
    }

    // Compare file contents
    for file in files1 {
        let content1 = std::fs::read_to_string(dir1.join(&file))?;
        let content2 = std::fs::read_to_string(dir2.join(&file))?;
        if content1 != content2 {
            return Ok(false);
        }
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_test_repo() {
        let temp_dir = create_test_repo().await.unwrap();
        assert!(temp_dir.path().join(".git").exists());
    }

    #[test]
    fn test_create_rust_project() {
        let temp_dir = TempDir::new().unwrap();
        create_rust_project(temp_dir.path()).unwrap();

        assert!(temp_dir.path().join("Cargo.toml").exists());
        assert!(temp_dir.path().join("src/main.rs").exists());
        assert!(temp_dir.path().join("src/lib.rs").exists());
    }

    #[test]
    fn test_file_contains() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "Hello, world!").unwrap();

        assert!(file_contains(&file_path, "Hello").unwrap());
        assert!(!file_contains(&file_path, "Goodbye").unwrap());
    }

    #[test]
    fn test_dirs_equal_identical_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let dir1 = temp_dir.path().join("dir1");
        let dir2 = temp_dir.path().join("dir2");

        std::fs::create_dir(&dir1).unwrap();
        std::fs::create_dir(&dir2).unwrap();

        // Create identical files in both directories
        std::fs::write(dir1.join("file1.txt"), "content1").unwrap();
        std::fs::write(dir2.join("file1.txt"), "content1").unwrap();
        std::fs::write(dir1.join("file2.txt"), "content2").unwrap();
        std::fs::write(dir2.join("file2.txt"), "content2").unwrap();

        assert!(dirs_equal(&dir1, &dir2).unwrap());
    }

    #[test]
    fn test_dirs_equal_different_files() {
        let temp_dir = TempDir::new().unwrap();
        let dir1 = temp_dir.path().join("dir1");
        let dir2 = temp_dir.path().join("dir2");

        std::fs::create_dir(&dir1).unwrap();
        std::fs::create_dir(&dir2).unwrap();

        // Different file sets
        std::fs::write(dir1.join("file1.txt"), "content1").unwrap();
        std::fs::write(dir2.join("file2.txt"), "content2").unwrap();

        assert!(!dirs_equal(&dir1, &dir2).unwrap());
    }

    #[test]
    fn test_dirs_equal_different_content() {
        let temp_dir = TempDir::new().unwrap();
        let dir1 = temp_dir.path().join("dir1");
        let dir2 = temp_dir.path().join("dir2");

        std::fs::create_dir(&dir1).unwrap();
        std::fs::create_dir(&dir2).unwrap();

        // Same files but different content
        std::fs::write(dir1.join("file.txt"), "content1").unwrap();
        std::fs::write(dir2.join("file.txt"), "content2").unwrap();

        assert!(!dirs_equal(&dir1, &dir2).unwrap());
    }

    #[test]
    fn test_dirs_equal_empty_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let dir1 = temp_dir.path().join("dir1");
        let dir2 = temp_dir.path().join("dir2");

        std::fs::create_dir(&dir1).unwrap();
        std::fs::create_dir(&dir2).unwrap();

        // Both directories are empty
        assert!(dirs_equal(&dir1, &dir2).unwrap());
    }

    #[test]
    fn test_dirs_equal_nested_structure() {
        let temp_dir = TempDir::new().unwrap();
        let dir1 = temp_dir.path().join("dir1");
        let dir2 = temp_dir.path().join("dir2");

        std::fs::create_dir(&dir1).unwrap();
        std::fs::create_dir(&dir2).unwrap();

        // Create nested directory structure
        std::fs::create_dir(dir1.join("subdir")).unwrap();
        std::fs::create_dir(dir2.join("subdir")).unwrap();
        std::fs::write(dir1.join("subdir").join("nested.txt"), "nested content").unwrap();
        std::fs::write(dir2.join("subdir").join("nested.txt"), "nested content").unwrap();

        assert!(dirs_equal(&dir1, &dir2).unwrap());
    }

    #[test]
    fn test_dirs_equal_missing_nested_file() {
        let temp_dir = TempDir::new().unwrap();
        let dir1 = temp_dir.path().join("dir1");
        let dir2 = temp_dir.path().join("dir2");

        std::fs::create_dir(&dir1).unwrap();
        std::fs::create_dir(&dir2).unwrap();

        // Create nested structure with missing file in dir2
        std::fs::create_dir(dir1.join("subdir")).unwrap();
        std::fs::create_dir(dir2.join("subdir")).unwrap();
        std::fs::write(dir1.join("subdir").join("nested.txt"), "content").unwrap();
        // Missing file in dir2/subdir

        assert!(!dirs_equal(&dir1, &dir2).unwrap());
    }

    #[test]
    fn test_dirs_equal_nonexistent_dir() {
        let temp_dir = TempDir::new().unwrap();
        let dir1 = temp_dir.path().join("dir1");
        let dir2 = temp_dir.path().join("nonexistent");

        std::fs::create_dir(&dir1).unwrap();
        std::fs::write(dir1.join("file.txt"), "content").unwrap();

        // dir2 doesn't exist, should return false (dirs not equal)
        assert!(!dirs_equal(&dir1, &dir2).unwrap());
    }

    #[test]
    fn test_dirs_equal_with_subdirs_only() {
        let temp_dir = TempDir::new().unwrap();
        let dir1 = temp_dir.path().join("dir1");
        let dir2 = temp_dir.path().join("dir2");

        std::fs::create_dir(&dir1).unwrap();
        std::fs::create_dir(&dir2).unwrap();

        // Create only subdirectories, no files
        std::fs::create_dir(dir1.join("empty_subdir")).unwrap();
        std::fs::create_dir(dir2.join("empty_subdir")).unwrap();

        // Should be equal since both have same structure (empty subdirs)
        assert!(dirs_equal(&dir1, &dir2).unwrap());
    }

    #[test]
    fn test_count_files() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        // Create files and subdirectories
        std::fs::write(dir.join("file1.txt"), "content1").unwrap();
        std::fs::write(dir.join("file2.txt"), "content2").unwrap();
        std::fs::create_dir(dir.join("subdir")).unwrap();
        std::fs::write(dir.join("subdir").join("file3.txt"), "content3").unwrap();

        assert_eq!(count_files(dir).unwrap(), 3);
    }
}
