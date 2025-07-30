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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
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

    /// Detect naming style from a collection of names
    pub fn detect_naming_style(&self, names: &[&str]) -> NamingStyle {
        if names.is_empty() {
            return NamingStyle::Mixed;
        }

        let mut style_counts = HashMap::new();

        for name in names {
            let style = if name.chars().all(|c| c.is_lowercase() || c == '_') {
                NamingStyle::SnakeCase
            } else if name.chars().all(|c| c.is_uppercase() || c == '_') {
                NamingStyle::ScreamingSnakeCase
            } else if name.contains('-') {
                NamingStyle::KebabCase
            } else if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                NamingStyle::PascalCase
            } else if name.chars().next().is_some_and(|c| c.is_lowercase())
                && name.chars().any(|c| c.is_uppercase())
            {
                NamingStyle::CamelCase
            } else {
                NamingStyle::Mixed
            };

            *style_counts.entry(style).or_insert(0) += 1;
        }

        // If there are multiple styles, return Mixed
        if style_counts.len() > 1 {
            NamingStyle::Mixed
        } else {
            style_counts
                .into_iter()
                .next()
                .map(|(style, _)| style)
                .unwrap_or(NamingStyle::Mixed)
        }
    }

    /// Detect patterns in code content
    pub fn detect_patterns(&self, content: &str) -> HashMap<String, Pattern> {
        self.detect_code_patterns(content)
    }

    /// Extract function names from code (public version)
    pub fn extract_function_names(content: &str) -> Vec<String> {
        let detector = Self::new();
        detector.extract_function_names_internal(content)
    }

    /// Extract variable names from code (public version)
    pub fn extract_variable_names(content: &str) -> Vec<String> {
        let detector = Self::new();
        detector.extract_variable_names_internal(content)
    }

    /// Extract type names from code (public version)
    pub fn extract_type_names(content: &str) -> Vec<String> {
        let detector = Self::new();
        detector.extract_type_names_internal(content)
    }

    /// Extract constant names from code (public version)
    pub fn extract_constant_names(content: &str) -> Vec<String> {
        let mut names = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("const ")
                || line.starts_with("static ")
                || line.starts_with("pub const ")
                || line.starts_with("pub static ")
            {
                if let Some(rest) = line.split_whitespace().nth(1) {
                    if rest == "const" || rest == "static" {
                        // pub const or pub static
                        if let Some(name) = line.split_whitespace().nth(2) {
                            names.push(name.split(':').next().unwrap_or("").to_string());
                        }
                    } else {
                        // const or static
                        names.push(rest.split(':').next().unwrap_or("").to_string());
                    }
                }
            }
        }

        names
    }

    /// Detect testing conventions from test content
    pub fn detect_test_conventions(content: &str) -> TestingConventions {
        let mut test_function_prefix = "test_".to_string();
        let test_module_pattern = "#[cfg(test)]".to_string();
        let mut assertion_style = "assert_eq!".to_string();

        // Detect test function prefix
        if content.contains("fn test_") {
            test_function_prefix = "test_".to_string();
        } else if content.contains("fn should_") {
            test_function_prefix = "should_".to_string();
        } else if content.contains("fn it_") {
            test_function_prefix = "it_".to_string();
        }

        // Detect assertion style
        if content.contains("assert_eq!") {
            assertion_style = "assert_eq!".to_string();
        } else if content.contains("assert!") {
            assertion_style = "assert!".to_string();
        } else if content.contains("expect(") {
            assertion_style = "expect".to_string();
        }

        TestingConventions {
            test_file_pattern: "tests/".to_string(),
            test_function_prefix,
            test_module_pattern,
            assertion_style,
        }
    }

    /// Detect naming style from examples
    fn detect_naming_style_internal(&self, names: &[String]) -> NamingStyle {
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

    /// Extract function names from Rust code (internal)
    fn extract_function_names_internal(&self, content: &str) -> Vec<String> {
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

    /// Extract variable names from Rust code (internal)
    fn extract_variable_names_internal(&self, content: &str) -> Vec<String> {
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

    /// Extract type names from Rust code (internal)
    fn extract_type_names_internal(&self, content: &str) -> Vec<String> {
        let mut names = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("struct ")
                || line.starts_with("enum ")
                || line.starts_with("trait ")
                || line.starts_with("type ")
            {
                if let Some(name) = line.split_whitespace().nth(1) {
                    names.push(name.trim_end_matches(['{', ';', '<']).to_string());
                }
            }
        }

        names
    }

    /// Detect common code patterns
    fn detect_code_patterns(&self, content: &str) -> HashMap<String, Pattern> {
        let mut patterns = HashMap::new();

        // Error handling pattern
        let mut error_count = 0;
        error_count += content.matches("Result<").count();
        error_count += content.matches("Err(").count();
        error_count += content.matches("Ok(").count();
        error_count += content.matches(".map_err").count();
        error_count += content.matches("?").count();

        if error_count > 0 {
            patterns.insert(
                "error_handling".to_string(),
                Pattern {
                    name: "Result-based error handling".to_string(),
                    description: "Uses Result<T, E> for error handling".to_string(),
                    examples: vec!["Result<String, Error>".to_string()],
                    frequency: error_count as u32,
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

        // Logging pattern
        let mut log_count = 0;
        log_count += content.matches("println!").count();
        log_count += content.matches("eprintln!").count();
        log_count += content.matches("log::").count();
        log_count += content.matches("tracing::").count();

        if log_count > 0 {
            patterns.insert(
                "logging".to_string(),
                Pattern {
                    name: "Logging pattern".to_string(),
                    description: "Uses logging for debugging and monitoring".to_string(),
                    examples: vec!["println!".to_string(), "log::info!".to_string()],
                    frequency: log_count as u32,
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
                // Don't filter out the root directory
                e.depth() == 0
                    || (!name.starts_with('.') && name != "target" && name != "node_modules")
            })
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
        {
            if let Ok(content) = tokio::fs::read_to_string(entry.path()).await {
                all_function_names.extend(self.extract_function_names_internal(&content));
                all_variable_names.extend(self.extract_variable_names_internal(&content));
                all_type_names.extend(self.extract_type_names_internal(&content));

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
            function_naming: self.detect_naming_style_internal(&all_function_names),
            variable_naming: self.detect_naming_style_internal(&all_variable_names),
            type_naming: self.detect_naming_style_internal(&all_type_names),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_naming_style_detection() {
        let detector = BasicConventionDetector::new();

        // Test snake_case
        assert_eq!(
            detector.detect_naming_style(&["hello_world", "foo_bar", "get_user_name"]),
            NamingStyle::SnakeCase
        );

        // Test camelCase
        assert_eq!(
            detector.detect_naming_style(&["helloWorld", "fooBar", "getUserName"]),
            NamingStyle::CamelCase
        );

        // Test PascalCase
        assert_eq!(
            detector.detect_naming_style(&["HelloWorld", "FooBar", "GetUserName"]),
            NamingStyle::PascalCase
        );

        // Test kebab-case
        assert_eq!(
            detector.detect_naming_style(&["hello-world", "foo-bar", "get-user-name"]),
            NamingStyle::KebabCase
        );

        // Test SCREAMING_SNAKE_CASE
        assert_eq!(
            detector.detect_naming_style(&["HELLO_WORLD", "FOO_BAR", "GET_USER_NAME"]),
            NamingStyle::ScreamingSnakeCase
        );

        // Test mixed styles
        assert_eq!(
            detector.detect_naming_style(&["hello_world", "fooBar", "GET_USER"]),
            NamingStyle::Mixed
        );
    }

    #[test]
    fn test_extract_function_names() {
        let content = r#"
            fn hello_world() {}
            pub fn get_user(id: u32) -> User {}
            async fn process_data(data: &str) {
                println!("Processing");
            }
            pub(crate) fn internal_helper() {}
        "#;

        let names = BasicConventionDetector::extract_function_names(content);
        assert_eq!(names.len(), 4);
        assert!(names.contains(&"hello_world".to_string()));
        assert!(names.contains(&"get_user".to_string()));
        assert!(names.contains(&"process_data".to_string()));
        assert!(names.contains(&"internal_helper".to_string()));
    }

    #[test]
    fn test_extract_variable_names() {
        let content = r#"
            let user_name = "John";
            let mut counter = 0;
            const MAX_SIZE: usize = 100;
            let (x, y) = get_coordinates();
            let Some(value) = option else { return };
        "#;

        let names = BasicConventionDetector::extract_variable_names(content);
        assert!(names.contains(&"user_name".to_string()));
        assert!(names.contains(&"counter".to_string()));
        assert!(!names.contains(&"MAX_SIZE".to_string())); // Constants are separate
    }

    #[test]
    fn test_extract_type_names() {
        let content = r#"
            struct User {
                name: String,
            }
            enum Status {
                Active,
                Inactive,
            }
            type UserId = u32;
            trait Validator {
                fn validate(&self) -> bool;
            }
        "#;

        let names = BasicConventionDetector::extract_type_names(content);
        assert_eq!(names.len(), 4);
        assert!(names.contains(&"User".to_string()));
        assert!(names.contains(&"Status".to_string()));
        assert!(names.contains(&"UserId".to_string()));
        assert!(names.contains(&"Validator".to_string()));
    }

    #[test]
    fn test_extract_constant_names() {
        let content = r#"
            const MAX_RETRIES: u32 = 3;
            const DATABASE_URL: &str = "localhost";
            pub const API_VERSION: &str = "v1";
            static COUNTER: AtomicU32 = AtomicU32::new(0);
        "#;

        let names = BasicConventionDetector::extract_constant_names(content);
        assert_eq!(names.len(), 4);
        assert!(names.contains(&"MAX_RETRIES".to_string()));
        assert!(names.contains(&"DATABASE_URL".to_string()));
        assert!(names.contains(&"API_VERSION".to_string()));
        assert!(names.contains(&"COUNTER".to_string()));
    }

    #[test]
    fn test_pattern_detection() {
        let detector = BasicConventionDetector::new();

        // Test error handling pattern
        let error_content = r#"
            match result {
                Ok(value) => value,
                Err(e) => return Err(e),
            }
            result.map_err(|e| CustomError::from(e))?;
        "#;

        let patterns = detector.detect_patterns(error_content);
        assert!(patterns.contains_key("error_handling"));
        assert!(patterns["error_handling"].frequency >= 2);

        // Test logging pattern
        let log_content = r#"
            log::info!("Starting process");
            tracing::debug!("Debug info");
            println!("Output");
            eprintln!("Error");
        "#;

        let patterns = detector.detect_patterns(log_content);
        assert!(patterns.contains_key("logging"));
        assert!(patterns["logging"].frequency >= 2);
    }

    #[test]
    fn test_testing_convention_detection() {
        let test_content = r#"
            #[cfg(test)]
            mod tests {
                use super::*;
                
                #[test]
                fn test_addition() {
                    assert_eq!(2 + 2, 4);
                }
                
                #[test]
                fn should_parse_correctly() {
                    assert!(parse("input").is_ok());
                }
            }
        "#;

        let conventions = BasicConventionDetector::detect_test_conventions(test_content);
        assert_eq!(conventions.test_module_pattern, "#[cfg(test)]".to_string());
        assert!(
            conventions.test_function_prefix == "test_"
                || conventions.test_function_prefix == "should_"
        );
        assert!(conventions.assertion_style.contains("assert"));
    }

    #[tokio::test]
    async fn test_full_convention_detection() {
        use std::fs;
        use tempfile::TempDir;

        let detector = BasicConventionDetector::new();
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        // Create test files
        fs::create_dir_all(project_path.join("src")).unwrap();
        fs::write(
            project_path.join("src/main.rs"),
            r#"
            use std::error::Error;
            
            fn main() -> Result<(), Box<dyn Error>> {
                let user_name = "test";
                println!("Hello, {}", user_name);
                process_data()?;
                Ok(())
            }
            
            fn process_data() -> Result<String, Box<dyn Error>> {
                Ok("data".to_string())
            }
            "#,
        )
        .unwrap();

        // Wait a bit to ensure file is written
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let conventions = detector.detect_conventions(project_path).await.unwrap();

        assert_eq!(
            conventions.naming_patterns.function_naming,
            NamingStyle::SnakeCase
        );
        assert_eq!(
            conventions.naming_patterns.variable_naming,
            NamingStyle::SnakeCase
        );
        assert!(!conventions.code_patterns.is_empty());
    }
}
