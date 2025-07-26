//! Code quality analysis

use anyhow::Result;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use tokio::fs;

use super::structure::ProjectStructure;

/// Code quality signals
#[derive(Debug, Clone)]
pub struct QualitySignals {
    pub avg_function_length: f32,
    pub max_function_length: usize,
    pub avg_file_length: f32,
    pub max_file_length: usize,
    pub duplicate_code_ratio: f32,
    pub comment_ratio: f32,
    pub test_ratio: f32,
    pub error_handling_score: f32,
}

/// Quality analyzer
pub struct QualityAnalyzer;

impl QualityAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub async fn analyze(&self, structure: &ProjectStructure) -> Result<QualitySignals> {
        let mut total_lines = 0;
        let mut total_files = 0;
        let mut max_file_length = 0;
        let mut total_functions = 0;
        let mut total_function_lines = 0;
        let mut max_function_length = 0;
        let mut comment_lines = 0;
        let mut test_files = 0;
        let mut source_files = 0;
        let mut error_handling_count = 0;
        let mut potential_error_sites = 0;

        // Analyze source directories
        for src_dir in &structure.src_dirs {
            self.analyze_directory(
                src_dir,
                &mut total_lines,
                &mut total_files,
                &mut max_file_length,
                &mut total_functions,
                &mut total_function_lines,
                &mut max_function_length,
                &mut comment_lines,
                &mut source_files,
                &mut error_handling_count,
                &mut potential_error_sites,
            )
            .await?;
        }

        // Count test files
        for test_dir in &structure.test_dirs {
            test_files += count_files_in_dir(test_dir).await?;
        }

        // Calculate metrics
        let avg_file_length = if total_files > 0 {
            total_lines as f32 / total_files as f32
        } else {
            0.0
        };

        let avg_function_length = if total_functions > 0 {
            total_function_lines as f32 / total_functions as f32
        } else {
            0.0
        };

        let comment_ratio = if total_lines > 0 {
            comment_lines as f32 / total_lines as f32
        } else {
            0.0
        };

        let test_ratio = if source_files > 0 {
            test_files as f32 / source_files as f32
        } else {
            0.0
        };

        let error_handling_score = if potential_error_sites > 0 {
            error_handling_count as f32 / potential_error_sites as f32
        } else {
            1.0
        };

        // TODO: Implement duplicate code detection
        let duplicate_code_ratio = 0.0;

        Ok(QualitySignals {
            avg_function_length,
            max_function_length,
            avg_file_length,
            max_file_length,
            duplicate_code_ratio,
            comment_ratio,
            test_ratio,
            error_handling_score,
        })
    }

    fn analyze_directory<'a>(
        &'a self,
        dir: &'a Path,
        total_lines: &'a mut usize,
        total_files: &'a mut usize,
        max_file_length: &'a mut usize,
        total_functions: &'a mut usize,
        total_function_lines: &'a mut usize,
        max_function_length: &'a mut usize,
        comment_lines: &'a mut usize,
        source_files: &'a mut usize,
        error_handling_count: &'a mut usize,
        potential_error_sites: &'a mut usize,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut entries = fs::read_dir(dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();

                if path.is_file() {
                    if is_source_file(&path) {
                        self.analyze_file(
                            &path,
                            total_lines,
                            total_files,
                            max_file_length,
                            total_functions,
                            total_function_lines,
                            max_function_length,
                            comment_lines,
                            source_files,
                            error_handling_count,
                            potential_error_sites,
                        )
                        .await?;
                    }
                } else if path.is_dir() && should_analyze_dir(&path) {
                    // Recurse into subdirectory
                    self.analyze_directory(
                        &path,
                        total_lines,
                        total_files,
                        max_file_length,
                        total_functions,
                        total_function_lines,
                        max_function_length,
                        comment_lines,
                        source_files,
                        error_handling_count,
                        potential_error_sites,
                    )
                    .await?;
                }
            }

            Ok(())
        })
    }

    async fn analyze_file(
        &self,
        path: &Path,
        total_lines: &mut usize,
        total_files: &mut usize,
        max_file_length: &mut usize,
        total_functions: &mut usize,
        total_function_lines: &mut usize,
        max_function_length: &mut usize,
        comment_lines: &mut usize,
        source_files: &mut usize,
        error_handling_count: &mut usize,
        potential_error_sites: &mut usize,
    ) -> Result<()> {
        let content = fs::read_to_string(path).await?;
        let lines: Vec<&str> = content.lines().collect();
        let file_lines = lines.len();

        *total_lines += file_lines;
        *total_files += 1;
        *source_files += 1;
        *max_file_length = (*max_file_length).max(file_lines);

        // Detect language for syntax-aware analysis
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match ext {
            "rs" => {
                self.analyze_rust_file(
                    &lines,
                    total_functions,
                    total_function_lines,
                    max_function_length,
                    comment_lines,
                    error_handling_count,
                    potential_error_sites,
                );
            }
            "py" => {
                self.analyze_python_file(
                    &lines,
                    total_functions,
                    total_function_lines,
                    max_function_length,
                    comment_lines,
                    error_handling_count,
                    potential_error_sites,
                );
            }
            "js" | "ts" | "jsx" | "tsx" => {
                self.analyze_javascript_file(
                    &lines,
                    total_functions,
                    total_function_lines,
                    max_function_length,
                    comment_lines,
                    error_handling_count,
                    potential_error_sites,
                );
            }
            _ => {
                // Generic analysis for other languages
                self.analyze_generic_file(&lines, comment_lines);
            }
        }

        Ok(())
    }

    fn analyze_rust_file(
        &self,
        lines: &[&str],
        total_functions: &mut usize,
        total_function_lines: &mut usize,
        max_function_length: &mut usize,
        comment_lines: &mut usize,
        error_handling_count: &mut usize,
        potential_error_sites: &mut usize,
    ) {
        let mut in_function = false;
        let mut function_start = 0;
        let mut brace_count = 0;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Count comments
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                *comment_lines += 1;
            }

            // Detect function declarations
            if !in_function && (trimmed.contains("fn ") || trimmed.contains("async fn ")) {
                in_function = true;
                function_start = i;
                *total_functions += 1;
                brace_count = 0;
            }

            // Count braces to track function boundaries
            if in_function {
                brace_count += line.matches('{').count() as i32;
                brace_count -= line.matches('}').count() as i32;

                if brace_count == 0 && line.contains('}') {
                    let function_length = i - function_start + 1;
                    *total_function_lines += function_length;
                    *max_function_length = (*max_function_length).max(function_length);
                    in_function = false;
                }
            }

            // Count error handling
            if line.contains(".unwrap()") || line.contains(".expect(") {
                *potential_error_sites += 1;
            } else if line.contains("?") || line.contains("Result<") || line.contains("Option<") {
                *error_handling_count += 1;
            }
        }
    }

    fn analyze_python_file(
        &self,
        lines: &[&str],
        total_functions: &mut usize,
        total_function_lines: &mut usize,
        max_function_length: &mut usize,
        comment_lines: &mut usize,
        error_handling_count: &mut usize,
        potential_error_sites: &mut usize,
    ) {
        let mut in_function = false;
        let mut function_start = 0;
        let mut current_indent = 0;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Count comments
            if trimmed.starts_with('#') {
                *comment_lines += 1;
            }

            // Detect function declarations
            if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                if in_function {
                    // End previous function
                    let function_length = i - function_start;
                    *total_function_lines += function_length;
                    *max_function_length = (*max_function_length).max(function_length);
                }

                in_function = true;
                function_start = i;
                current_indent = line.len() - trimmed.len();
                *total_functions += 1;
            } else if in_function && !line.is_empty() {
                let line_indent = line.len() - line.trim_start().len();
                if line_indent <= current_indent {
                    // Function ended
                    let function_length = i - function_start;
                    *total_function_lines += function_length;
                    *max_function_length = (*max_function_length).max(function_length);
                    in_function = false;
                }
            }

            // Count error handling
            if line.contains("try:") {
                *error_handling_count += 1;
            }
            if line.contains("except") && !line.contains("except:") {
                *error_handling_count += 1;
            }
            if line.contains("raise ") {
                *potential_error_sites += 1;
            }
        }

        // Handle last function
        if in_function {
            let function_length = lines.len() - function_start;
            *total_function_lines += function_length;
            *max_function_length = (*max_function_length).max(function_length);
        }
    }

    fn analyze_javascript_file(
        &self,
        lines: &[&str],
        total_functions: &mut usize,
        total_function_lines: &mut usize,
        max_function_length: &mut usize,
        comment_lines: &mut usize,
        error_handling_count: &mut usize,
        potential_error_sites: &mut usize,
    ) {
        let mut in_function = false;
        let mut function_start = 0;
        let mut brace_count = 0;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Count comments
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                *comment_lines += 1;
            }

            // Detect function declarations
            if !in_function
                && (trimmed.contains("function ")
                    || trimmed.contains("=>")
                    || trimmed.contains("async ")
                    || trimmed.contains("const ")
                    || trimmed.contains("let ")
                    || trimmed.contains("var "))
                && (line.contains('{') || line.contains("=>"))
            {
                in_function = true;
                function_start = i;
                *total_functions += 1;
                brace_count = 0;
            }

            // Count braces
            if in_function {
                brace_count += line.matches('{').count() as i32;
                brace_count -= line.matches('}').count() as i32;

                if brace_count == 0 && line.contains('}') {
                    let function_length = i - function_start + 1;
                    *total_function_lines += function_length;
                    *max_function_length = (*max_function_length).max(function_length);
                    in_function = false;
                }
            }

            // Count error handling
            if line.contains("try {") {
                *error_handling_count += 1;
            }
            if line.contains("catch") {
                *error_handling_count += 1;
            }
            if line.contains("throw ") {
                *potential_error_sites += 1;
            }
        }
    }

    fn analyze_generic_file(&self, lines: &[&str], comment_lines: &mut usize) {
        for line in lines {
            let trimmed = line.trim();
            // Generic comment detection
            if trimmed.starts_with("//")
                || trimmed.starts_with("#")
                || trimmed.starts_with("/*")
                || trimmed.starts_with("--")
            {
                *comment_lines += 1;
            }
        }
    }
}

impl Default for QualityAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

fn is_source_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        matches!(
            ext.to_str().unwrap_or_default(),
            "rs" | "py"
                | "js"
                | "ts"
                | "jsx"
                | "tsx"
                | "go"
                | "java"
                | "cs"
                | "rb"
                | "swift"
                | "kt"
                | "cpp"
                | "c"
                | "h"
                | "hpp"
        )
    } else {
        false
    }
}

fn should_analyze_dir(path: &Path) -> bool {
    if let Some(name) = path.file_name() {
        !matches!(
            name.to_str().unwrap_or_default(),
            "node_modules" | "target" | ".git" | "dist" | "build" | "__pycache__" | ".pytest_cache"
        )
    } else {
        true
    }
}

fn count_files_in_dir(dir: &Path) -> Pin<Box<dyn Future<Output = Result<usize>> + Send + '_>> {
    Box::pin(async move {
        let mut count = 0;
        let mut entries = fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() && is_source_file(&path) {
                count += 1;
            } else if path.is_dir() && should_analyze_dir(&path) {
                count += count_files_in_dir(&path).await?;
            }
        }

        Ok(count)
    })
}
