use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;
use tokio::fs;

/// Command discovery system for dynamically loading commands from filesystem
///
/// This struct handles scanning the .claude/commands directory for markdown files,
/// caching their contents, and providing them to the command registry for parsing
/// and validation.
pub struct CommandDiscovery {
    commands_dir: PathBuf,
    cache: HashMap<String, CommandFile>,
    last_scan: Option<SystemTime>,
}

impl CommandDiscovery {
    pub fn new(commands_dir: PathBuf) -> Self {
        Self {
            commands_dir,
            cache: HashMap::new(),
            last_scan: None,
        }
    }

    /// Check if a file is a valid command file (markdown file starting with "prodigy-")
    fn is_command_file(path: &std::path::Path) -> bool {
        let is_md = path.extension().is_some_and(|ext| ext == "md");
        let has_prefix = path
            .file_stem()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("prodigy-"));
        is_md && has_prefix
    }

    /// Check if a cached command file is still valid
    fn is_cache_valid(cached: &CommandFile, modified: SystemTime) -> bool {
        cached.modified >= modified
    }

    /// Create a CommandFile from a path and content
    async fn create_command_file(path: PathBuf, modified: SystemTime) -> Result<CommandFile> {
        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();
        let content = fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read command file: {}", path.display()))?;

        Ok(CommandFile {
            path,
            name,
            content,
            modified,
        })
    }

    /// Scan the commands directory for .md files and return `CommandFile` objects
    ///
    /// This method:
    /// - Reads all .md files from the commands directory
    /// - Filters for files starting with "prodigy-"
    /// - Caches file contents based on modification time
    /// - Returns a list of discovered command files
    pub async fn scan_commands(&mut self) -> Result<Vec<CommandFile>> {
        if !self.commands_dir.exists() {
            return Ok(vec![]);
        }

        let mut commands = Vec::new();
        let mut entries = fs::read_dir(&self.commands_dir).await.with_context(|| {
            format!(
                "Failed to read commands directory: {}",
                self.commands_dir.display()
            )
        })?;

        while let Some(entry) = entries.next_entry().await? {
            if let Some(command_file) = self.process_entry(entry).await? {
                commands.push(command_file);
            }
        }

        self.last_scan = Some(SystemTime::now());
        Ok(commands)
    }

    /// Process a single directory entry
    async fn process_entry(&mut self, entry: fs::DirEntry) -> Result<Option<CommandFile>> {
        let path = entry.path();

        if !Self::is_command_file(&path) {
            return Ok(None);
        }

        let metadata = entry.metadata().await?;
        let modified = metadata.modified()?;

        let file_name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("");

        // Check cache and use if valid
        if let Some(cached) = self.cache.get(file_name) {
            if Self::is_cache_valid(cached, modified) {
                return Ok(Some(cached.clone()));
            }
        }

        // Create and cache new command file
        let command_file = Self::create_command_file(path, modified).await?;
        self.cache
            .insert(command_file.name.clone(), command_file.clone());
        Ok(Some(command_file))
    }

    /// Check if the discovery cache needs refreshing
    pub fn needs_refresh(&self) -> bool {
        self.last_scan.is_none()
    }

    /// Clear the cache to force a full rescan on next `scan_commands` call
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.last_scan = None;
    }
}

/// Represents a discovered command file with its metadata
#[derive(Clone, Debug)]
pub struct CommandFile {
    pub path: PathBuf,
    pub name: String,
    pub content: String,
    pub modified: SystemTime,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_scan_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();

        let mut discovery = CommandDiscovery::new(commands_dir);
        let commands = discovery.scan_commands().await.unwrap();

        assert_eq!(commands.len(), 0);
    }

    #[tokio::test]
    async fn test_scan_commands_filters_correctly() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();

        // Create various test files
        fs::write(commands_dir.join("prodigy-test.md"), "# Test Command")
            .await
            .unwrap();
        fs::write(commands_dir.join("README.md"), "# Readme")
            .await
            .unwrap();
        fs::write(commands_dir.join("test.txt"), "Not a command")
            .await
            .unwrap();
        fs::write(commands_dir.join("other-command.md"), "# Other")
            .await
            .unwrap();

        let mut discovery = CommandDiscovery::new(commands_dir);
        let commands = discovery.scan_commands().await.unwrap();

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "prodigy-test");
    }

    #[tokio::test]
    async fn test_cache_behavior() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();

        let test_file = commands_dir.join("prodigy-test.md");
        fs::write(&test_file, "# Original Content").await.unwrap();

        let mut discovery = CommandDiscovery::new(commands_dir.clone());

        // First scan
        let commands1 = discovery.scan_commands().await.unwrap();
        assert_eq!(commands1.len(), 1);
        assert_eq!(commands1[0].content, "# Original Content");

        // Second scan should use cache
        let commands2 = discovery.scan_commands().await.unwrap();
        assert_eq!(commands2.len(), 1);
        assert_eq!(commands2[0].content, "# Original Content");

        // Modify file
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        fs::write(&test_file, "# Updated Content").await.unwrap();

        // Third scan should detect change
        let commands3 = discovery.scan_commands().await.unwrap();
        assert_eq!(commands3.len(), 1);
        assert_eq!(commands3[0].content, "# Updated Content");
    }

    #[tokio::test]
    async fn test_needs_refresh() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();

        let mut discovery = CommandDiscovery::new(commands_dir);

        assert!(discovery.needs_refresh());

        discovery.scan_commands().await.unwrap();
        assert!(!discovery.needs_refresh());

        discovery.clear_cache();
        assert!(discovery.needs_refresh());
    }

    #[test]
    fn test_is_command_file() {
        use std::path::Path;

        // Valid command files
        assert!(CommandDiscovery::is_command_file(Path::new(
            "prodigy-test.md"
        )));
        assert!(CommandDiscovery::is_command_file(Path::new(
            "prodigy-another-command.md"
        )));
        assert!(CommandDiscovery::is_command_file(Path::new(
            "/path/to/prodigy-cmd.md"
        )));

        // Invalid: wrong extension
        assert!(!CommandDiscovery::is_command_file(Path::new(
            "prodigy-test.txt"
        )));
        assert!(!CommandDiscovery::is_command_file(Path::new(
            "prodigy-test"
        )));

        // Invalid: wrong prefix
        assert!(!CommandDiscovery::is_command_file(Path::new("test.md")));
        assert!(!CommandDiscovery::is_command_file(Path::new("README.md")));
        assert!(!CommandDiscovery::is_command_file(Path::new(
            "other-command.md"
        )));

        // Edge cases
        assert!(!CommandDiscovery::is_command_file(Path::new("")));
        assert!(!CommandDiscovery::is_command_file(Path::new(".")));
    }

    #[test]
    fn test_is_cache_valid() {
        use std::time::Duration;

        let now = SystemTime::now();
        let earlier = now - Duration::from_secs(10);
        let later = now + Duration::from_secs(10);

        let cached = CommandFile {
            path: PathBuf::from("test.md"),
            name: "test".to_string(),
            content: "content".to_string(),
            modified: now,
        };

        // Cache is valid when cached time >= file modified time
        assert!(CommandDiscovery::is_cache_valid(&cached, now));
        assert!(CommandDiscovery::is_cache_valid(&cached, earlier));

        // Cache is invalid when cached time < file modified time
        assert!(!CommandDiscovery::is_cache_valid(&cached, later));
    }

    #[tokio::test]
    async fn test_create_command_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("prodigy-test.md");
        fs::write(&test_file, "# Test Content").await.unwrap();

        let metadata = fs::metadata(&test_file).await.unwrap();
        let modified = metadata.modified().unwrap();

        let command_file = CommandDiscovery::create_command_file(test_file.clone(), modified)
            .await
            .unwrap();

        assert_eq!(command_file.path, test_file);
        assert_eq!(command_file.name, "prodigy-test");
        assert_eq!(command_file.content, "# Test Content");
        assert_eq!(command_file.modified, modified);
    }

    #[tokio::test]
    async fn test_create_command_file_nonexistent() {
        let path = PathBuf::from("/nonexistent/prodigy-test.md");
        let modified = SystemTime::now();

        let result = CommandDiscovery::create_command_file(path, modified).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to read command file"));
    }

    #[tokio::test]
    async fn test_cache_invalidation_on_file_update() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();

        let test_file = commands_dir.join("prodigy-test.md");
        fs::write(&test_file, "# Original").await.unwrap();

        let mut discovery = CommandDiscovery::new(commands_dir.clone());

        // First scan
        let commands1 = discovery.scan_commands().await.unwrap();
        assert_eq!(commands1[0].content, "# Original");

        // Update file with a small delay to ensure modified time changes
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        fs::write(&test_file, "# Updated").await.unwrap();

        // Scan again - should detect the change
        let commands2 = discovery.scan_commands().await.unwrap();
        assert_eq!(commands2[0].content, "# Updated");
    }

    #[tokio::test]
    async fn test_scan_nonexistent_directory() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("nonexistent");

        let mut discovery = CommandDiscovery::new(commands_dir);
        let commands = discovery.scan_commands().await.unwrap();

        assert_eq!(commands.len(), 0);
    }

    #[tokio::test]
    async fn test_multiple_valid_command_files() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();

        // Create multiple valid command files
        for i in 1..=5 {
            let filename = format!("prodigy-cmd{}.md", i);
            let content = format!("# Command {}", i);
            fs::write(commands_dir.join(&filename), &content)
                .await
                .unwrap();
        }

        let mut discovery = CommandDiscovery::new(commands_dir);
        let commands = discovery.scan_commands().await.unwrap();

        assert_eq!(commands.len(), 5);

        // Verify all commands were found
        let names: Vec<String> = commands.iter().map(|c| c.name.clone()).collect();
        for i in 1..=5 {
            assert!(names.contains(&format!("prodigy-cmd{}", i)));
        }
    }

    #[tokio::test]
    async fn test_process_entry_with_cache() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();

        let test_file = commands_dir.join("prodigy-test.md");
        fs::write(&test_file, "# Test Content").await.unwrap();

        let mut discovery = CommandDiscovery::new(commands_dir.clone());

        // First scan - should read from disk
        let commands1 = discovery.scan_commands().await.unwrap();
        assert_eq!(commands1.len(), 1);
        assert_eq!(commands1[0].content, "# Test Content");

        // Second scan - should use cache (file not modified)
        let commands2 = discovery.scan_commands().await.unwrap();
        assert_eq!(commands2.len(), 1);
        assert_eq!(commands2[0].content, "# Test Content");

        // Clear cache and scan again
        discovery.clear_cache();
        let commands3 = discovery.scan_commands().await.unwrap();
        assert_eq!(commands3.len(), 1);
        assert_eq!(commands3[0].content, "# Test Content");
    }
}
