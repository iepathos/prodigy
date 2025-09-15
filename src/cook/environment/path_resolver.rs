//! Cross-platform path resolution
//!
//! Handles path resolution, variable expansion, and separator normalization
//! across different operating systems.

use regex::Regex;
use std::path::PathBuf;

/// Platform type for path resolution
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Platform {
    Windows,
    Unix,
}

impl Platform {
    /// Detect current platform
    pub fn current() -> Self {
        if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Unix
        }
    }
}

/// Path resolver for cross-platform path handling
pub struct PathResolver {
    platform: Platform,
}

impl PathResolver {
    /// Create a new path resolver for the current platform
    pub fn new() -> Self {
        Self {
            platform: Platform::current(),
        }
    }

    /// Create a path resolver for a specific platform
    pub fn with_platform(platform: Platform) -> Self {
        Self { platform }
    }

    /// Resolve a path with variable expansion and normalization
    pub fn resolve(&self, path: &str) -> PathBuf {
        let expanded = self.expand_variables(path);
        let normalized = self.normalize_separators(&expanded);
        PathBuf::from(normalized)
    }

    /// Expand variables in path (pure function)
    fn expand_variables(&self, path: &str) -> String {
        let mut result = path.to_string();

        // Expand ~ to home directory
        result = self.expand_home_dir(result);

        // Expand environment variables
        result = self.expand_env_vars(result);

        result
    }

    /// Expand home directory notation (pure function)
    fn expand_home_dir(&self, path: String) -> String {
        if path.starts_with("~/") || path == "~" {
            if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
                if path == "~" {
                    return home;
                }
                return path.replacen("~", &home, 1);
            }
        }
        path
    }

    /// Expand environment variables in path (pure function)
    fn expand_env_vars(&self, path: String) -> String {
        // Match ${VAR} or $VAR patterns
        let env_var_re = Regex::new(r"\$\{([^}]+)\}|\$([A-Za-z_][A-Za-z0-9_]*)").unwrap();

        let mut result = path.clone();
        for cap in env_var_re.captures_iter(&path) {
            let var_name = cap.get(1).or_else(|| cap.get(2)).unwrap().as_str();
            if let Ok(value) = std::env::var(var_name) {
                result = result.replace(cap.get(0).unwrap().as_str(), &value);
            }
        }
        result
    }

    /// Normalize path separators for platform (pure function)
    fn normalize_separators(&self, path: &str) -> String {
        match self.platform {
            Platform::Windows => {
                // Convert forward slashes to backslashes on Windows
                // But preserve UNC paths and single forward slashes in arguments
                if path.starts_with("\\\\") || path.starts_with("//") {
                    // UNC path - normalize to backslashes
                    path.replace('/', "\\")
                } else {
                    path.replace('/', "\\")
                }
            }
            Platform::Unix => {
                // Convert backslashes to forward slashes on Unix
                path.replace('\\', "/")
            }
        }
    }

    /// Check if a path is absolute
    pub fn is_absolute(&self, path: &str) -> bool {
        match self.platform {
            Platform::Windows => {
                // Windows absolute paths:
                // - Start with drive letter (C:\)
                // - Start with UNC (\\server\share)
                path.chars().nth(1) == Some(':')
                    || path.starts_with("\\\\")
                    || path.starts_with("//")
            }
            Platform::Unix => {
                // Unix absolute paths start with /
                path.starts_with('/')
            }
        }
    }

    /// Join two paths
    pub fn join(&self, base: &str, path: &str) -> String {
        if self.is_absolute(path) {
            return path.to_string();
        }

        let separator = match self.platform {
            Platform::Windows => "\\",
            Platform::Unix => "/",
        };

        let base = base.trim_end_matches(&['/', '\\'][..]);
        format!("{}{}{}", base, separator, path)
    }
}

impl Default for PathResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_home_dir() {
        let resolver = PathResolver::new();

        // Set HOME for test
        std::env::set_var("HOME", "/home/user");

        let expanded = resolver.expand_home_dir("~/Documents".to_string());
        assert_eq!(expanded, "/home/user/Documents");

        let expanded = resolver.expand_home_dir("~".to_string());
        assert_eq!(expanded, "/home/user");

        let expanded = resolver.expand_home_dir("/absolute/path".to_string());
        assert_eq!(expanded, "/absolute/path");
    }

    #[test]
    fn test_expand_env_vars() {
        let resolver = PathResolver::new();

        std::env::set_var("TEST_VAR", "value");
        std::env::set_var("TEST_PATH", "/test/path");

        let expanded = resolver.expand_env_vars("${TEST_VAR}/file".to_string());
        assert_eq!(expanded, "value/file");

        let expanded = resolver.expand_env_vars("$TEST_PATH/file".to_string());
        assert_eq!(expanded, "/test/path/file");

        let expanded = resolver.expand_env_vars("${TEST_VAR}/${TEST_PATH}".to_string());
        assert_eq!(expanded, "value//test/path");
    }

    #[test]
    fn test_normalize_separators_unix() {
        let resolver = PathResolver::with_platform(Platform::Unix);

        assert_eq!(
            resolver.normalize_separators("path\\to\\file"),
            "path/to/file"
        );
        assert_eq!(
            resolver.normalize_separators("path/to/file"),
            "path/to/file"
        );
    }

    #[test]
    fn test_normalize_separators_windows() {
        let resolver = PathResolver::with_platform(Platform::Windows);

        assert_eq!(
            resolver.normalize_separators("path/to/file"),
            "path\\to\\file"
        );
        assert_eq!(
            resolver.normalize_separators("path\\to\\file"),
            "path\\to\\file"
        );
        assert_eq!(
            resolver.normalize_separators("\\\\server\\share"),
            "\\\\server\\share"
        );
    }

    #[test]
    fn test_is_absolute() {
        let unix_resolver = PathResolver::with_platform(Platform::Unix);
        assert!(unix_resolver.is_absolute("/path/to/file"));
        assert!(!unix_resolver.is_absolute("relative/path"));

        let win_resolver = PathResolver::with_platform(Platform::Windows);
        assert!(win_resolver.is_absolute("C:\\path\\to\\file"));
        assert!(win_resolver.is_absolute("\\\\server\\share"));
        assert!(!win_resolver.is_absolute("relative\\path"));
    }

    #[test]
    fn test_join() {
        let unix_resolver = PathResolver::with_platform(Platform::Unix);
        assert_eq!(
            unix_resolver.join("/base/path", "file.txt"),
            "/base/path/file.txt"
        );
        assert_eq!(
            unix_resolver.join("/base/path/", "file.txt"),
            "/base/path/file.txt"
        );
        assert_eq!(unix_resolver.join("/base", "/absolute"), "/absolute");

        let win_resolver = PathResolver::with_platform(Platform::Windows);
        assert_eq!(
            win_resolver.join("C:\\base", "file.txt"),
            "C:\\base\\file.txt"
        );
    }

    #[test]
    fn test_resolve_full_path() {
        let resolver = PathResolver::new();

        std::env::set_var("HOME", "/home/user");
        std::env::set_var("PROJECT", "myproject");

        let resolved = resolver.resolve("~/${PROJECT}/src");
        let expected = if cfg!(windows) {
            PathBuf::from("/home/user/myproject/src".replace('/', "\\"))
        } else {
            PathBuf::from("/home/user/myproject/src")
        };

        assert_eq!(resolved, expected);
    }
}
