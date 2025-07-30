//! Convention detection and pattern learning

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Trait for convention detection
#[async_trait::async_trait]
pub trait ConventionDetector: Send + Sync {
    /// Detect conventions in the project
    async fn detect_conventions(&self, project_path: &Path) -> Result<ProjectConventions>;

    /// Update conventions based on changed files
    async fn update_conventions(
        &self,
        project_path: &Path,
        current: &ProjectConventions,
        changed_files: &[PathBuf],
    ) -> Result<ProjectConventions>;
}

/// Project-wide conventions detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConventions {
    pub naming_patterns: NamingRules,
    pub code_patterns: HashMap<String, Pattern>,
    pub test_patterns: TestingConventions,
    pub project_idioms: Vec<Idiom>,
}

/// Naming rules detected in the project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamingRules {
    pub file_naming: NamingStyle,
    pub function_naming: NamingStyle,
    pub variable_naming: NamingStyle,
    pub type_naming: NamingStyle,
    pub constant_naming: NamingStyle,
}

/// Naming style enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NamingStyle {
    SnakeCase,
    CamelCase,
    PascalCase,
    KebabCase,
    ScreamingSnakeCase,
    Mixed,
}

/// Code pattern detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub name: String,
    pub description: String,
    pub examples: Vec<String>,
    pub frequency: u32,
}

/// Testing conventions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestingConventions {
    pub test_file_pattern: String,
    pub test_function_prefix: String,
    pub test_module_pattern: String,
    pub assertion_style: String,
}

/// Project-specific idiom
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Idiom {
    pub name: String,
    pub pattern: String,
    pub usage_count: u32,
}

/// File-specific convention information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConventionInfo {
    pub naming_style: String,
    pub patterns: Vec<String>,
    pub violations: Vec<String>,
}

impl ProjectConventions {
    /// Get conventions for a specific file
    pub fn get_file_conventions(&self, _file: &Path) -> FileConventionInfo {
        FileConventionInfo {
            naming_style: format!("{:?}", self.naming_patterns.function_naming),
            patterns: self.code_patterns.keys().cloned().collect(),
            violations: Vec::new(),
        }
    }

    /// Get naming violations across the project
    pub fn get_naming_violations(&self) -> HashMap<String, Vec<String>> {
        // This would be populated during analysis
        HashMap::new()
    }
}

/// Basic convention detector implementation
pub struct BasicConventionDetector;

impl Default for BasicConventionDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl BasicConventionDetector {
    pub fn new() -> Self {
        Self
    }

    /// Detect naming style from examples
    fn detect_naming_style(&self, names: &[String]) -> NamingStyle {
        let mut style_counts = HashMap::new();

        for name in names {
            let style = if name.chars().all(|c| c.is_lowercase() || c == '_') {
                NamingStyle::SnakeCase
            } else if name.chars().all(|c| c.is_uppercase() || c == '_') {
                NamingStyle::ScreamingSnakeCase
            } else if name.contains('_') {
                NamingStyle::Mixed
            } else if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                NamingStyle::PascalCase
            } else {
                NamingStyle::CamelCase
            };

            *style_counts.entry(format!("{style:?}")).or_insert(0) += 1;
        }

        // Return most common style
        style_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(style_str, _)| match style_str.as_str() {
                "SnakeCase" => NamingStyle::SnakeCase,
                "CamelCase" => NamingStyle::CamelCase,
                "PascalCase" => NamingStyle::PascalCase,
                "ScreamingSnakeCase" => NamingStyle::ScreamingSnakeCase,
                _ => NamingStyle::Mixed,
            })
            .unwrap_or(NamingStyle::SnakeCase)
    }

    /// Extract function names from Rust code
    fn extract_function_names(&self, content: &str) -> Vec<String> {
        let mut names = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("fn ") || line.contains(" fn ") {
                if let Some(start) = line.find("fn ") {
                    let after_fn = &line[start + 3..];
                    if let Some(name) = after_fn.split(['(', '<']).next() {
                        names.push(name.trim().to_string());
                    }
                }
            }
        }

        names
    }

    /// Extract variable names from Rust code
    fn extract_variable_names(&self, content: &str) -> Vec<String> {
        let mut names = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.contains("let ") {
                if let Some(start) = line.find("let ") {
                    let after_let = &line[start + 4..];
                    // Handle mutable variables
                    let after_let = after_let.strip_prefix("mut ").unwrap_or(after_let);
                    if let Some(name) = after_let.split([':', '=']).next() {
                        names.push(name.trim().to_string());
                    }
                }
            }
        }

        names
    }

    /// Extract type names from Rust code
    fn extract_type_names(&self, content: &str) -> Vec<String> {
        let mut names = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("struct ")
                || line.starts_with("enum ")
                || line.starts_with("trait ")
                || line.starts_with("type ")
            {
                if let Some(name) = line.split_whitespace().nth(1) {
                    names.push(
                        name.trim_end_matches(['{', ';', '<'])
                            .to_string(),
                    );
                }
            }
        }

        names
    }

    /// Detect common code patterns
    fn detect_code_patterns(&self, content: &str) -> HashMap<String, Pattern> {
        let mut patterns = HashMap::new();

        // Error handling pattern
        let result_count = content.matches("Result<").count();
        if result_count > 0 {
            patterns.insert(
                "error_handling".to_string(),
                Pattern {
                    name: "Result-based error handling".to_string(),
                    description: "Uses Result<T, E> for error handling".to_string(),
                    examples: vec!["Result<String, Error>".to_string()],
                    frequency: result_count as u32,
                },
            );
        }

        // Async pattern
        let async_count = content.matches("async fn").count();
        if async_count > 0 {
            patterns.insert(
                "async".to_string(),
                Pattern {
                    name: "Async/await pattern".to_string(),
                    description: "Uses async/await for asynchronous code".to_string(),
                    examples: vec!["async fn process()".to_string()],
                    frequency: async_count as u32,
                },
            );
        }

        // Builder pattern
        let builder_count = content.matches("Builder").count();
        if builder_count > 0 {
            patterns.insert(
                "builder".to_string(),
                Pattern {
                    name: "Builder pattern".to_string(),
                    description: "Uses builder pattern for object construction".to_string(),
                    examples: vec!["RequestBuilder::new()".to_string()],
                    frequency: builder_count as u32,
                },
            );
        }

        patterns
    }

    /// Detect testing conventions
    fn detect_test_patterns(&self, project_path: &Path) -> TestingConventions {
        let mut test_file_pattern = "tests/".to_string();
        let test_function_prefix = "test_".to_string();
        let test_module_pattern = "#[cfg(test)]".to_string();
        let assertion_style = "assert_eq!".to_string();

        // Check for different test configurations
        if project_path.join("tests").exists() {
            test_file_pattern = "tests/".to_string();
        } else if project_path.join("src/tests").exists() {
            test_file_pattern = "src/tests/".to_string();
        }

        // TODO: Analyze actual test files to detect patterns

        TestingConventions {
            test_file_pattern,
            test_function_prefix,
            test_module_pattern,
            assertion_style,
        }
    }
}

#[async_trait::async_trait]
impl ConventionDetector for BasicConventionDetector {
    async fn detect_conventions(&self, project_path: &Path) -> Result<ProjectConventions> {
        use walkdir::WalkDir;

        let mut all_function_names = Vec::new();
        let mut all_variable_names = Vec::new();
        let mut all_type_names = Vec::new();
        let mut all_patterns = HashMap::new();
        let mut pattern_counts = HashMap::new();

        // Walk through Rust files
        for entry in WalkDir::new(project_path)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.') && name != "target" && name != "node_modules"
            })
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
        {
            if let Ok(content) = tokio::fs::read_to_string(entry.path()).await {
                all_function_names.extend(self.extract_function_names(&content));
                all_variable_names.extend(self.extract_variable_names(&content));
                all_type_names.extend(self.extract_type_names(&content));

                // Detect patterns
                let file_patterns = self.detect_code_patterns(&content);
                for (key, pattern) in file_patterns {
                    *pattern_counts.entry(key.clone()).or_insert(0) += pattern.frequency;
                    all_patterns.insert(key, pattern);
                }
            }
        }

        // Detect naming styles
        let naming_patterns = NamingRules {
            file_naming: NamingStyle::SnakeCase, // Rust standard
            function_naming: self.detect_naming_style(&all_function_names),
            variable_naming: self.detect_naming_style(&all_variable_names),
            type_naming: self.detect_naming_style(&all_type_names),
            constant_naming: NamingStyle::ScreamingSnakeCase, // Rust standard
        };

        // Detect test patterns
        let test_patterns = self.detect_test_patterns(project_path);

        // Common Rust idioms
        let project_idioms = vec![
            Idiom {
                name: "Option matching".to_string(),
                pattern: "match option { Some(x) => ..., None => ... }".to_string(),
                usage_count: all_patterns.get("option_match").map_or(0, |p| p.frequency),
            },
            Idiom {
                name: "Result propagation".to_string(),
                pattern: "function()?".to_string(),
                usage_count: all_patterns
                    .get("result_propagation")
                    .map_or(0, |p| p.frequency),
            },
        ];

        Ok(ProjectConventions {
            naming_patterns,
            code_patterns: all_patterns,
            test_patterns,
            project_idioms,
        })
    }

    async fn update_conventions(
        &self,
        project_path: &Path,
        current: &ProjectConventions,
        changed_files: &[PathBuf],
    ) -> Result<ProjectConventions> {
        // For small updates, just return current conventions
        // Full re-analysis only if many files changed
        if changed_files.len() > 10 {
            self.detect_conventions(project_path).await
        } else {
            Ok(current.clone())
        }
    }
}
