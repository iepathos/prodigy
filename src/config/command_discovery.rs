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

    /// Check if a file is a valid command file
    fn is_command_file(path: &std::path::Path) -> bool {
        match path.extension() {
            Some(ext) if ext == "md" => path
                .file_stem()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("prodigy-"))
                .unwrap_or(false),
            _ => false,
        }
    }

    /// Get or load command file from cache
    async fn get_or_load_command(
        &mut self,
        path: PathBuf,
        file_name: &str,
        modified: SystemTime,
    ) -> Result<CommandFile> {
        // Check if cached version is still valid
        if let Some(cached) = self.cache.get(file_name) {
            if cached.modified >= modified {
                return Ok(cached.clone());
            }
        }

        // Load and cache the file
        let content = fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read command file: {}", path.display()))?;

        let command_file = CommandFile {
            path: path.clone(),
            name: file_name.to_string(),
            content,
            modified,
        };

        self.cache
            .insert(file_name.to_string(), command_file.clone());

        Ok(command_file)
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
            let path = entry.path();

            // Skip non-command files
            if !Self::is_command_file(&path) {
                continue;
            }

            let file_name = path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("")
                .to_string();

            let metadata = entry.metadata().await?;
            let modified = metadata.modified()?;

            let command_file = self.get_or_load_command(path, &file_name, modified).await?;
            commands.push(command_file);
        }

        self.last_scan = Some(SystemTime::now());
        Ok(commands)
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
    use std::path::Path;
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
        // Valid command files
        assert!(CommandDiscovery::is_command_file(Path::new(
            "prodigy-test.md"
        )));
        assert!(CommandDiscovery::is_command_file(Path::new(
            "prodigy-another-command.md"
        )));
        assert!(CommandDiscovery::is_command_file(Path::new(
            "/path/to/prodigy-command.md"
        )));

        // Invalid command files
        assert!(!CommandDiscovery::is_command_file(Path::new("README.md")));
        assert!(!CommandDiscovery::is_command_file(Path::new(
            "test-prodigy.md"
        )));
        assert!(!CommandDiscovery::is_command_file(Path::new(
            "prodigy-test.txt"
        )));
        assert!(!CommandDiscovery::is_command_file(Path::new("test.md")));
        assert!(!CommandDiscovery::is_command_file(Path::new(
            "prodigy-test"
        )));
        assert!(!CommandDiscovery::is_command_file(Path::new("")));
    }

    #[tokio::test]
    async fn test_get_or_load_command_uses_cache() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();

        let test_file = commands_dir.join("prodigy-test.md");
        fs::write(&test_file, "# Original Content").await.unwrap();

        let mut discovery = CommandDiscovery::new(commands_dir.clone());
        let metadata = fs::metadata(&test_file).await.unwrap();
        let modified = metadata.modified().unwrap();

        // First load should read from disk
        let command1 = discovery
            .get_or_load_command(test_file.clone(), "prodigy-test", modified)
            .await
            .unwrap();
        assert_eq!(command1.content, "# Original Content");
        assert_eq!(discovery.cache.len(), 1);

        // Second load should use cache
        let command2 = discovery
            .get_or_load_command(test_file.clone(), "prodigy-test", modified)
            .await
            .unwrap();
        assert_eq!(command2.content, "# Original Content");
        assert_eq!(discovery.cache.len(), 1);

        // Verify both are the same cached instance
        assert_eq!(command1.content, command2.content);
    }

    #[tokio::test]
    async fn test_get_or_load_command_invalidates_stale_cache() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir(&commands_dir).await.unwrap();

        let test_file = commands_dir.join("prodigy-test.md");
        fs::write(&test_file, "# Original Content").await.unwrap();

        let mut discovery = CommandDiscovery::new(commands_dir.clone());
        let metadata = fs::metadata(&test_file).await.unwrap();
        let modified = metadata.modified().unwrap();

        // First load
        let command1 = discovery
            .get_or_load_command(test_file.clone(), "prodigy-test", modified)
            .await
            .unwrap();
        assert_eq!(command1.content, "# Original Content");

        // Update file with new content
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        fs::write(&test_file, "# Updated Content").await.unwrap();
        let new_metadata = fs::metadata(&test_file).await.unwrap();
        let new_modified = new_metadata.modified().unwrap();

        // Load with new timestamp should reload from disk
        let command2 = discovery
            .get_or_load_command(test_file.clone(), "prodigy-test", new_modified)
            .await
            .unwrap();
        assert_eq!(command2.content, "# Updated Content");
    }
}
