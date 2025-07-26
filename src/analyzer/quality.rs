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

/// Analysis metrics collected during quality analysis
struct AnalysisMetrics {
    total_lines: usize,
    total_files: usize,
    max_file_length: usize,
    total_functions: usize,
    total_function_lines: usize,
    max_function_length: usize,
    comment_lines: usize,
    source_files: usize,
    error_handling_count: usize,
    potential_error_sites: usize,
}

impl AnalysisMetrics {
    fn new() -> Self {
        Self {
            total_lines: 0,
            total_files: 0,
            max_file_length: 0,
            total_functions: 0,
            total_function_lines: 0,
            max_function_length: 0,
            comment_lines: 0,
            source_files: 0,
            error_handling_count: 0,
            potential_error_sites: 0,
        }
    }
}

/// Quality analyzer
pub struct QualityAnalyzer;

impl QualityAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub async fn analyze(&self, structure: &ProjectStructure) -> Result<QualitySignals> {
        let mut metrics = AnalysisMetrics::new();
        let mut test_files = 0;

        // Analyze source directories
        for src_dir in &structure.src_dirs {
            self.analyze_directory(src_dir, &mut metrics).await?;
        }

        // Count test files
        for test_dir in &structure.test_dirs {
            test_files += count_files_in_dir(test_dir).await?;
        }

        // Calculate metrics
        let avg_file_length = if metrics.total_files > 0 {
            metrics.total_lines as f32 / metrics.total_files as f32
        } else {
            0.0
        };

        let avg_function_length = if metrics.total_functions > 0 {
            metrics.total_function_lines as f32 / metrics.total_functions as f32
        } else {
            0.0
        };

        let comment_ratio = if metrics.total_lines > 0 {
            metrics.comment_lines as f32 / metrics.total_lines as f32
        } else {
            0.0
        };

        let test_ratio = if metrics.source_files > 0 {
            test_files as f32 / metrics.source_files as f32
        } else {
            0.0
        };

        let error_handling_score = if metrics.potential_error_sites > 0 {
            metrics.error_handling_count as f32 / metrics.potential_error_sites as f32
        } else {
            1.0
        };

        // TODO: Implement duplicate code detection
        let duplicate_code_ratio = 0.0;

        Ok(QualitySignals {
            avg_function_length,
            max_function_length: metrics.max_function_length,
            avg_file_length,
            max_file_length: metrics.max_file_length,
            duplicate_code_ratio,
            comment_ratio,
            test_ratio,
            error_handling_score,
        })
    }

    fn analyze_directory<'a>(
        &'a self,
        dir: &'a Path,
        metrics: &'a mut AnalysisMetrics,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut entries = fs::read_dir(dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();

                if path.is_file() {
                    if is_source_file(&path) {
                        self.analyze_file(&path, metrics).await?;
                    }
                } else if path.is_dir() && should_analyze_dir(&path) {
                    // Recurse into subdirectory
                    self.analyze_directory(&path, metrics).await?;
                }
            }

            Ok(())
        })
    }

    async fn analyze_file(&self, path: &Path, metrics: &mut AnalysisMetrics) -> Result<()> {
        let content = fs::read_to_string(path).await?;
        let lines: Vec<&str> = content.lines().collect();
        let file_lines = lines.len();

        metrics.total_lines += file_lines;
        metrics.total_files += 1;
        metrics.source_files += 1;
        metrics.max_file_length = metrics.max_file_length.max(file_lines);

        // Detect language for syntax-aware analysis
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match ext {
            "rs" => {
                self.analyze_rust_file(&lines, metrics);
            }
            "py" => {
                self.analyze_python_file(&lines, metrics);
            }
            "js" | "ts" | "jsx" | "tsx" => {
                self.analyze_javascript_file(&lines, metrics);
            }
            _ => {
                // Generic analysis for other languages
                self.analyze_generic_file(&lines, &mut metrics.comment_lines);
            }
        }

        Ok(())
    }

    fn analyze_rust_file(&self, lines: &[&str], metrics: &mut AnalysisMetrics) {
        let mut in_function = false;
        let mut function_start = 0;
        let mut brace_count = 0;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Count comments
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                metrics.comment_lines += 1;
            }

            // Detect function declarations
            if !in_function && (trimmed.contains("fn ") || trimmed.contains("async fn ")) {
                in_function = true;
                function_start = i;
                metrics.total_functions += 1;
                brace_count = 0;
            }

            // Count braces to track function boundaries
            if in_function {
                brace_count += line.matches('{').count() as i32;
                brace_count -= line.matches('}').count() as i32;

                if brace_count == 0 && line.contains('}') {
                    let function_length = i - function_start + 1;
                    metrics.total_function_lines += function_length;
                    metrics.max_function_length = metrics.max_function_length.max(function_length);
                    in_function = false;
                }
            }

            // Count error handling
            if line.contains(".unwrap()") || line.contains(".expect(") {
                metrics.potential_error_sites += 1;
            } else if line.contains("?") || line.contains("Result<") || line.contains("Option<") {
                metrics.error_handling_count += 1;
            }
        }
    }

    fn analyze_python_file(&self, lines: &[&str], metrics: &mut AnalysisMetrics) {
        let mut in_function = false;
        let mut function_start = 0;
        let mut current_indent = 0;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Count comments
            if trimmed.starts_with('#') {
                metrics.comment_lines += 1;
            }

            // Detect function declarations
            if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                if in_function {
                    // End previous function
                    let function_length = i - function_start;
                    metrics.total_function_lines += function_length;
                    metrics.max_function_length = metrics.max_function_length.max(function_length);
                }

                in_function = true;
                function_start = i;
                current_indent = line.len() - trimmed.len();
                metrics.total_functions += 1;
            } else if in_function && !line.is_empty() {
                let line_indent = line.len() - line.trim_start().len();
                if line_indent <= current_indent {
                    // Function ended
                    let function_length = i - function_start;
                    metrics.total_function_lines += function_length;
                    metrics.max_function_length = metrics.max_function_length.max(function_length);
                    in_function = false;
                }
            }

            // Count error handling
            if line.contains("try:") {
                metrics.error_handling_count += 1;
            }
            if line.contains("except") && !line.contains("except:") {
                metrics.error_handling_count += 1;
            }
            if line.contains("raise ") {
                metrics.potential_error_sites += 1;
            }
        }

        // Handle last function
        if in_function {
            let function_length = lines.len() - function_start;
            metrics.total_function_lines += function_length;
            metrics.max_function_length = metrics.max_function_length.max(function_length);
        }
    }

    fn analyze_javascript_file(&self, lines: &[&str], metrics: &mut AnalysisMetrics) {
        let mut in_function = false;
        let mut function_start = 0;
        let mut brace_count = 0;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Count comments
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                metrics.comment_lines += 1;
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
                metrics.total_functions += 1;
                brace_count = 0;
            }

            // Count braces
            if in_function {
                brace_count += line.matches('{').count() as i32;
                brace_count -= line.matches('}').count() as i32;

                if brace_count == 0 && line.contains('}') {
                    let function_length = i - function_start + 1;
                    metrics.total_function_lines += function_length;
                    metrics.max_function_length = metrics.max_function_length.max(function_length);
                    in_function = false;
                }
            }

            // Count error handling
            if line.contains("try {") {
                metrics.error_handling_count += 1;
            }
            if line.contains("catch") {
                metrics.error_handling_count += 1;
            }
            if line.contains("throw ") {
                metrics.potential_error_sites += 1;
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
