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

    /// Check if a file should be processed as a command file
    ///
    /// Returns the file name if it's a valid command file, None otherwise
    fn is_command_file(path: &std::path::Path) -> Option<String> {
        // Check if it's a markdown file
        match path.extension() {
            Some(ext) if ext == "md" => {}
            _ => return None,
        }

        // Check if it starts with "prodigy-"
        let file_name = path.file_stem()?.to_str()?;
        if file_name.starts_with("prodigy-") {
            Some(file_name.to_string())
        } else {
            None
        }
    }

    /// Try to get a command from cache if it's still valid
    fn get_cached_command(&self, file_name: &str, modified: SystemTime) -> Option<CommandFile> {
        self.cache
            .get(file_name)
            .filter(|cached| cached.modified >= modified)
            .cloned()
    }

    /// Load a command file from disk and update the cache
    async fn load_and_cache_command(
        &mut self,
        path: PathBuf,
        file_name: String,
        modified: SystemTime,
    ) -> Result<CommandFile> {
        let content = fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read command file: {}", path.display()))?;

        let command_file = CommandFile {
            path,
            name: file_name.clone(),
            content,
            modified,
        };

        self.cache.insert(file_name, command_file.clone());
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

            // Check if this is a valid command file
            let Some(file_name) = Self::is_command_file(&path) else {
                continue;
            };

            let metadata = entry.metadata().await?;
            let modified = metadata.modified()?;

            // Try to use cached version if available and up-to-date
            if let Some(cached) = self.get_cached_command(&file_name, modified) {
                commands.push(cached);
                continue;
            }

            // Load from disk and cache
            let command_file = self
                .load_and_cache_command(path, file_name, modified)
                .await?;
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

    #[test]
    fn test_is_command_file() {
        // Valid command files
        assert_eq!(
            CommandDiscovery::is_command_file(Path::new("prodigy-test.md")),
            Some("prodigy-test".to_string())
        );
        assert_eq!(
            CommandDiscovery::is_command_file(Path::new("prodigy-another-command.md")),
            Some("prodigy-another-command".to_string())
        );
        assert_eq!(
            CommandDiscovery::is_command_file(Path::new("/path/to/prodigy-cmd.md")),
            Some("prodigy-cmd".to_string())
        );

        // Invalid files - wrong extension
        assert_eq!(
            CommandDiscovery::is_command_file(Path::new("prodigy-test.txt")),
            None
        );
        assert_eq!(
            CommandDiscovery::is_command_file(Path::new("prodigy-test.rs")),
            None
        );
        assert_eq!(
            CommandDiscovery::is_command_file(Path::new("prodigy-test")),
            None
        );

        // Invalid files - wrong prefix
        assert_eq!(
            CommandDiscovery::is_command_file(Path::new("test.md")),
            None
        );
        assert_eq!(
            CommandDiscovery::is_command_file(Path::new("README.md")),
            None
        );
        assert_eq!(
            CommandDiscovery::is_command_file(Path::new("other-command.md")),
            None
        );

        // Edge cases
        assert_eq!(
            CommandDiscovery::is_command_file(Path::new("prodigy-.md")),
            Some("prodigy-".to_string())
        );
        assert_eq!(CommandDiscovery::is_command_file(Path::new(".md")), None);
        assert_eq!(CommandDiscovery::is_command_file(Path::new("")), None);
    }

    #[test]
    fn test_get_cached_command() {
        let mut cache = HashMap::new();
        let now = SystemTime::now();
        let past = now - std::time::Duration::from_secs(10);
        let future = now + std::time::Duration::from_secs(10);

        let command = CommandFile {
            path: PathBuf::from("test.md"),
            name: "test".to_string(),
            content: "content".to_string(),
            modified: now,
        };

        cache.insert("test".to_string(), command.clone());

        let discovery = CommandDiscovery {
            commands_dir: PathBuf::from("."),
            cache,
            last_scan: None,
        };

        // Cache hit - file hasn't been modified
        assert!(discovery.get_cached_command("test", past).is_some());
        assert!(discovery.get_cached_command("test", now).is_some());

        // Cache miss - file has been modified
        assert!(discovery.get_cached_command("test", future).is_none());

        // Cache miss - file not in cache
        assert!(discovery.get_cached_command("nonexistent", now).is_none());
    }

    #[tokio::test]
    async fn test_load_and_cache_command() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.md");
        fs::write(&test_file, "# Test Content").await.unwrap();

        let mut discovery = CommandDiscovery::new(temp_dir.path().to_path_buf());
        let modified = SystemTime::now();

        let command = discovery
            .load_and_cache_command(test_file.clone(), "test".to_string(), modified)
            .await
            .unwrap();

        assert_eq!(command.name, "test");
        assert_eq!(command.content, "# Test Content");
        assert_eq!(command.modified, modified);
        assert_eq!(command.path, test_file);

        // Check that it was cached
        assert!(discovery.cache.contains_key("test"));
        assert_eq!(
            discovery.cache.get("test").unwrap().content,
            "# Test Content"
        );
    }

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
}
