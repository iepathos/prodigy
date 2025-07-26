//! Project structure analysis

use anyhow::Result;
use std::collections::HashSet;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tokio::fs;

/// Types of configuration files
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigFileType {
    Build,
    Config,
    CI,
    Other,
}

/// Configuration file information
#[derive(Debug, Clone)]
pub struct ConfigFile {
    pub path: PathBuf,
    pub file_type: ConfigFileType,
}

/// Project structure information
#[derive(Debug, Clone)]
pub struct ProjectStructure {
    pub root: PathBuf,
    pub src_dirs: Vec<PathBuf>,
    pub test_dirs: Vec<PathBuf>,
    pub config_files: Vec<ConfigFile>,
    pub entry_points: Vec<PathBuf>,
    pub important_files: Vec<PathBuf>,
    pub ignored_patterns: Vec<String>,
}

/// Structure analyzer
pub struct StructureAnalyzer {
    src_dir_names: HashSet<&'static str>,
    test_dir_names: HashSet<&'static str>,
    config_file_patterns: Vec<(&'static str, ConfigFileType)>,
    entry_point_patterns: Vec<&'static str>,
    important_file_patterns: Vec<&'static str>,
}

impl StructureAnalyzer {
    pub fn new() -> Self {
        let mut src_dir_names = HashSet::new();
        src_dir_names.insert("src");
        src_dir_names.insert("lib");
        src_dir_names.insert("app");
        src_dir_names.insert("source");
        src_dir_names.insert("Sources");

        let mut test_dir_names = HashSet::new();
        test_dir_names.insert("tests");
        test_dir_names.insert("test");
        test_dir_names.insert("spec");
        test_dir_names.insert("specs");
        test_dir_names.insert("__tests__");
        test_dir_names.insert("Tests");

        let config_file_patterns = vec![
            // Build files
            ("Cargo.toml", ConfigFileType::Build),
            ("package.json", ConfigFileType::Build),
            ("requirements.txt", ConfigFileType::Build),
            ("setup.py", ConfigFileType::Build),
            ("pyproject.toml", ConfigFileType::Build),
            ("go.mod", ConfigFileType::Build),
            ("pom.xml", ConfigFileType::Build),
            ("build.gradle", ConfigFileType::Build),
            ("build.gradle.kts", ConfigFileType::Build),
            ("Gemfile", ConfigFileType::Build),
            ("Package.swift", ConfigFileType::Build),
            // Config files
            ("tsconfig.json", ConfigFileType::Config),
            (".eslintrc", ConfigFileType::Config),
            (".prettierrc", ConfigFileType::Config),
            ("jest.config.js", ConfigFileType::Config),
            ("webpack.config.js", ConfigFileType::Config),
            ("vite.config.js", ConfigFileType::Config),
            // CI files
            (".travis.yml", ConfigFileType::CI),
            (".gitlab-ci.yml", ConfigFileType::CI),
            ("Jenkinsfile", ConfigFileType::CI),
            ("azure-pipelines.yml", ConfigFileType::CI),
        ];

        let entry_point_patterns = vec![
            "main.rs",
            "lib.rs",
            "index.js",
            "index.ts",
            "main.py",
            "__main__.py",
            "app.py",
            "main.go",
            "Main.java",
            "Program.cs",
            "main.rb",
            "app.rb",
            "main.swift",
            "Main.kt",
        ];

        let important_file_patterns = vec![
            "README.md",
            "README.rst",
            "README",
            "LICENSE",
            "LICENSE.md",
            "LICENSE.txt",
            "CONTRIBUTING.md",
            "CHANGELOG.md",
            ".gitignore",
            ".dockerignore",
            "Dockerfile",
            "docker-compose.yml",
            "Makefile",
            ".env.example",
        ];

        Self {
            src_dir_names,
            test_dir_names,
            config_file_patterns,
            entry_point_patterns,
            important_file_patterns,
        }
    }

    pub async fn analyze(&self, path: &Path) -> Result<ProjectStructure> {
        let root = path.canonicalize()?;

        // Find source directories
        let src_dirs = self.find_directories(&root, &self.src_dir_names).await?;

        // Find test directories
        let test_dirs = self.find_directories(&root, &self.test_dir_names).await?;

        // Find config files
        let config_files = self.find_config_files(&root).await?;

        // Find entry points
        let entry_points = self.find_entry_points(&root, &src_dirs).await?;

        // Find important files
        let important_files = self.find_important_files(&root).await?;

        // Load .gitignore patterns
        let ignored_patterns = self.load_gitignore_patterns(&root).await?;

        Ok(ProjectStructure {
            root,
            src_dirs,
            test_dirs,
            config_files,
            entry_points,
            important_files,
            ignored_patterns,
        })
    }

    async fn find_directories(&self, root: &Path, names: &HashSet<&str>) -> Result<Vec<PathBuf>> {
        let mut dirs = Vec::new();
        self.scan_for_directories(root, root, names, &mut dirs, 0)
            .await?;
        dirs.sort();
        dirs.dedup();
        Ok(dirs)
    }

    fn scan_for_directories<'a>(
        &'a self,
        root: &'a Path,
        current: &'a Path,
        names: &'a HashSet<&str>,
        dirs: &'a mut Vec<PathBuf>,
        depth: usize,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            // Limit recursion depth
            if depth > 5 {
                return Ok(());
            }

            let mut entries = fs::read_dir(current).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();

                // Skip hidden directories (except .github)
                if let Some(name) = path.file_name() {
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with('.') && name_str != ".github" {
                        continue;
                    }

                    // Skip node_modules, target, dist, build directories
                    if matches!(
                        name_str.as_ref(),
                        "node_modules" | "target" | "dist" | "build" | "out"
                    ) {
                        continue;
                    }
                }

                if path.is_dir() {
                    if let Some(dir_name) = path.file_name() {
                        if names.contains(dir_name.to_str().unwrap_or_default()) {
                            dirs.push(path.clone());
                        }
                    }

                    // Recurse
                    self.scan_for_directories(root, &path, names, dirs, depth + 1)
                        .await?;
                }
            }

            Ok(())
        })
    }

    async fn find_config_files(&self, root: &Path) -> Result<Vec<ConfigFile>> {
        let mut config_files = Vec::new();
        self.scan_for_config_files(root, root, &mut config_files, 0)
            .await?;
        Ok(config_files)
    }

    fn scan_for_config_files<'a>(
        &'a self,
        root: &'a Path,
        current: &'a Path,
        files: &'a mut Vec<ConfigFile>,
        depth: usize,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            // Limit recursion depth for config files
            if depth > 3 {
                return Ok(());
            }

            let mut entries = fs::read_dir(current).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();

                if path.is_file() {
                    if let Some(file_name) = path.file_name() {
                        let name_str = file_name.to_string_lossy();

                        // Check against patterns
                        for (pattern, file_type) in &self.config_file_patterns {
                            if name_str == *pattern {
                                files.push(ConfigFile {
                                    path: path.clone(),
                                    file_type: file_type.clone(),
                                });
                                break;
                            }
                        }

                        // Check for .csproj files
                        if name_str.ends_with(".csproj") {
                            files.push(ConfigFile {
                                path: path.clone(),
                                file_type: ConfigFileType::Build,
                            });
                        }

                        // Check for GitHub workflows
                        if current.ends_with(".github/workflows") && name_str.ends_with(".yml") {
                            files.push(ConfigFile {
                                path: path.clone(),
                                file_type: ConfigFileType::CI,
                            });
                        }
                    }
                } else if path.is_dir()
                    && path
                        .file_name()
                        .map(|n| n != "node_modules")
                        .unwrap_or(true)
                {
                    self.scan_for_config_files(root, &path, files, depth + 1)
                        .await?;
                }
            }

            Ok(())
        })
    }

    async fn find_entry_points(&self, root: &Path, src_dirs: &[PathBuf]) -> Result<Vec<PathBuf>> {
        let mut entry_points = Vec::new();

        // Check root directory
        for pattern in &self.entry_point_patterns {
            let path = root.join(pattern);
            if path.exists() {
                entry_points.push(path);
            }
        }

        // Check source directories
        for src_dir in src_dirs {
            for pattern in &self.entry_point_patterns {
                let path = src_dir.join(pattern);
                if path.exists() {
                    entry_points.push(path);
                }
            }
        }

        Ok(entry_points)
    }

    async fn find_important_files(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut important_files = Vec::new();

        for pattern in &self.important_file_patterns {
            let path = root.join(pattern);
            if path.exists() {
                important_files.push(path);
            }
        }

        Ok(important_files)
    }

    async fn load_gitignore_patterns(&self, root: &Path) -> Result<Vec<String>> {
        let gitignore_path = root.join(".gitignore");

        if gitignore_path.exists() {
            let content = fs::read_to_string(&gitignore_path).await?;
            Ok(content
                .lines()
                .filter(|line| !line.trim().is_empty() && !line.trim().starts_with('#'))
                .map(|line| line.trim().to_string())
                .collect())
        } else {
            Ok(Vec::new())
        }
    }
}

impl Default for StructureAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
