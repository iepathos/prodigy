//! Technical debt detection and mapping

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::path::{Path, PathBuf};

/// Trait for technical debt mapping
#[async_trait::async_trait]
pub trait TechnicalDebtMapper: Send + Sync {
    /// Map technical debt in the project
    async fn map_technical_debt(&self, project_path: &Path) -> Result<TechnicalDebtMap>;

    /// Update debt map based on changed files
    async fn update_debt_map(
        &self,
        project_path: &Path,
        current: &TechnicalDebtMap,
        changed_files: &[PathBuf],
    ) -> Result<TechnicalDebtMap>;
}

/// Map of technical debt in the project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalDebtMap {
    pub debt_items: Vec<DebtItem>,
    pub hotspots: Vec<ComplexityHotspot>,
    pub duplication_map: HashMap<String, Vec<CodeBlock>>,
    #[serde(skip)]
    pub priority_queue: BinaryHeap<DebtItem>,
}

/// A technical debt item
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct DebtItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub location: PathBuf,
    pub line_number: Option<u32>,
    pub debt_type: DebtType,
    pub impact: u32, // 1-10
    pub effort: u32, // 1-10
    pub tags: Vec<String>,
}

/// Type of technical debt
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum DebtType {
    CodeSmell,
    Duplication,
    Complexity,
    Todo,
    Fixme,
    Hack,
    Deprecated,
    Performance,
    Security,
    TestCoverage,
}

/// Code complexity hotspot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityHotspot {
    pub file: PathBuf,
    pub function: String,
    pub complexity: u32,
    pub lines: u32,
}

/// Duplicated code block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlock {
    pub file: PathBuf,
    pub start_line: u32,
    pub end_line: u32,
    pub content_hash: String,
}

impl Ord for DebtItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority = higher impact, lower effort
        let self_priority = (self.impact * 10) / (self.effort + 1);
        let other_priority = (other.impact * 10) / (other.effort + 1);
        self_priority.cmp(&other_priority)
    }
}

impl PartialOrd for DebtItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl TechnicalDebtMap {
    /// Get debt items for a specific file
    pub fn get_file_debt(&self, file: &Path) -> Vec<DebtItem> {
        self.debt_items
            .iter()
            .filter(|item| item.location == file)
            .cloned()
            .collect()
    }

    /// Get file complexity
    pub fn get_file_complexity(&self, file: &Path) -> u32 {
        self.hotspots
            .iter()
            .filter(|h| h.file == file)
            .map(|h| h.complexity)
            .sum()
    }

    /// Get top priority debt items
    pub fn get_priority_items(&self, count: usize) -> Vec<&DebtItem> {
        let mut items: Vec<_> = self.debt_items.iter().collect();
        items.sort_by(|a, b| b.cmp(a));
        items.into_iter().take(count).collect()
    }
}

/// Basic technical debt mapper implementation
pub struct BasicTechnicalDebtMapper;

impl Default for BasicTechnicalDebtMapper {
    fn default() -> Self {
        Self::new()
    }
}

impl BasicTechnicalDebtMapper {
    pub fn new() -> Self {
        Self
    }

    /// Extract TODO/FIXME/HACK comments
    async fn extract_comments(&self, file_path: &Path, content: &str) -> Vec<DebtItem> {
        let mut debt_items = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let line_upper = line.to_uppercase();

            // Check for debt markers
            let (debt_type, marker) = if line_upper.contains("TODO") {
                (DebtType::Todo, "TODO")
            } else if line_upper.contains("FIXME") {
                (DebtType::Fixme, "FIXME")
            } else if line_upper.contains("HACK") {
                (DebtType::Hack, "HACK")
            } else if line_upper.contains("DEPRECATED") {
                (DebtType::Deprecated, "DEPRECATED")
            } else {
                continue;
            };

            // Extract the comment text
            let description = if let Some(pos) = line.find(marker) {
                line[pos + marker.len()..]
                    .trim_start_matches(':')
                    .trim()
                    .to_string()
            } else {
                "No description provided".to_string()
            };

            let impact = match debt_type {
                DebtType::Fixme => 7,
                DebtType::Hack => 6,
                DebtType::Deprecated => 8,
                DebtType::Todo => 4,
                _ => 5,
            };

            debt_items.push(DebtItem {
                id: format!("{:?}_{}_L{}", debt_type, file_path.display(), line_num + 1),
                title: format!("{debt_type:?} comment"),
                description,
                location: file_path.to_path_buf(),
                line_number: Some(line_num as u32 + 1),
                debt_type,
                impact,
                effort: 3,
                tags: vec!["comment".to_string()],
            });
        }

        debt_items
    }

    /// Calculate cyclomatic complexity for functions
    fn calculate_complexity(&self, function_content: &str) -> u32 {
        let mut complexity = 1; // Base complexity

        // Count decision points
        for line in function_content.lines() {
            let line = line.trim();

            // Conditionals
            if line.starts_with("if ") || line.contains(" if ") {
                complexity += 1;
            }

            // Loops
            if line.starts_with("for ") || line.starts_with("while ") || line.starts_with("loop ") {
                complexity += 1;
            }

            // Match arms
            if line.trim_end().ends_with(" => ") || line.trim_end().ends_with(" => {") {
                complexity += 1;
            }

            // Early returns
            if line.starts_with("return ") && !line.contains("fn ") {
                complexity += 1;
            }

            // Error propagation
            if line.contains("?") && !line.contains("Option<") && !line.contains("Result<") {
                complexity += 1;
            }
        }

        complexity
    }

    /// Extract functions and calculate complexity
    async fn analyze_complexity(&self, file_path: &Path, content: &str) -> Vec<ComplexityHotspot> {
        let mut hotspots = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            if line.trim().starts_with("fn ") || line.contains(" fn ") {
                // Extract function name
                if let Some(name_start) = line.find("fn ") {
                    let after_fn = &line[name_start + 3..];
                    if let Some(name) = after_fn.split(['(', '<']).next() {
                        let function_name = name.trim().to_string();

                        // Find function body
                        let mut depth = 0;
                        let mut start_line = i;
                        let mut end_line = i;
                        let mut found_start = false;

                        for (j, line) in lines[i..].iter().enumerate() {
                            for ch in line.chars() {
                                if ch == '{' {
                                    if !found_start {
                                        found_start = true;
                                        start_line = i + j;
                                    }
                                    depth += 1;
                                } else if ch == '}' {
                                    depth -= 1;
                                    if depth == 0 && found_start {
                                        end_line = i + j;
                                        break;
                                    }
                                }
                            }
                            if depth == 0 && found_start {
                                break;
                            }
                        }

                        if found_start && end_line > start_line {
                            let function_content = lines[start_line..=end_line].join("\n");
                            let complexity = self.calculate_complexity(&function_content);

                            if complexity > 10 {
                                hotspots.push(ComplexityHotspot {
                                    file: file_path.to_path_buf(),
                                    function: function_name,
                                    complexity,
                                    lines: (end_line - start_line + 1) as u32,
                                });
                            }
                        }
                    }
                }
            }
        }

        hotspots
    }

    /// Simple duplicate detection using line hashes
    fn detect_duplicates(&self, files: &[(PathBuf, String)]) -> HashMap<String, Vec<CodeBlock>> {
        let mut hash_to_blocks: HashMap<String, Vec<CodeBlock>> = HashMap::new();
        let min_duplicate_lines = 5;

        for (file_path, content) in files {
            let lines: Vec<&str> = content.lines().collect();

            // Check sliding windows of lines
            for window_size in min_duplicate_lines..20 {
                for i in 0..lines.len().saturating_sub(window_size) {
                    let window_content = lines[i..i + window_size].join("\n");

                    // Skip trivial duplicates (empty lines, single braces)
                    if window_content.trim().is_empty()
                        || window_content
                            .chars()
                            .all(|c| c.is_whitespace() || c == '{' || c == '}')
                    {
                        continue;
                    }

                    // Create hash of the content
                    let hash = format!("{:x}", md5::compute(&window_content));

                    hash_to_blocks
                        .entry(hash.clone())
                        .or_default()
                        .push(CodeBlock {
                            file: file_path.clone(),
                            start_line: i as u32 + 1,
                            end_line: (i + window_size) as u32,
                            content_hash: hash,
                        });
                }
            }
        }

        // Filter out non-duplicates
        hash_to_blocks.retain(|_, blocks| blocks.len() > 1);
        hash_to_blocks
    }
}

#[async_trait::async_trait]
impl TechnicalDebtMapper for BasicTechnicalDebtMapper {
    async fn map_technical_debt(&self, project_path: &Path) -> Result<TechnicalDebtMap> {
        use walkdir::WalkDir;

        let mut all_debt_items = Vec::new();
        let mut all_hotspots = Vec::new();
        let mut files_content = Vec::new();

        // Walk through source files
        for entry in WalkDir::new(project_path)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.') && name != "target" && name != "node_modules"
            })
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
        {
            let file_path = entry.path();
            if let Ok(content) = tokio::fs::read_to_string(file_path).await {
                // Extract debt comments
                let debt_items = self.extract_comments(file_path, &content).await;
                all_debt_items.extend(debt_items);

                // Analyze complexity
                let hotspots = self.analyze_complexity(file_path, &content).await;
                all_hotspots.extend(hotspots);

                // Store for duplicate detection
                files_content.push((file_path.to_path_buf(), content));
            }
        }

        // Add complexity hotspots as debt items
        for hotspot in &all_hotspots {
            all_debt_items.push(DebtItem {
                id: format!("COMPLEX_{}_{}", hotspot.file.display(), hotspot.function),
                title: format!("High complexity in {}", hotspot.function),
                description: format!(
                    "Function has cyclomatic complexity of {}",
                    hotspot.complexity
                ),
                location: hotspot.file.clone(),
                line_number: None,
                debt_type: DebtType::Complexity,
                impact: if hotspot.complexity > 20 { 8 } else { 6 },
                effort: 5,
                tags: vec!["complexity".to_string()],
            });
        }

        // Detect duplicates
        let duplication_map = self.detect_duplicates(&files_content);

        // Add duplication as debt items
        for (hash, blocks) in &duplication_map {
            if blocks.len() > 1 {
                let first_block = &blocks[0];
                all_debt_items.push(DebtItem {
                    id: format!("DUP_{hash}"),
                    title: "Code duplication detected".to_string(),
                    description: format!(
                        "Found {} instances of duplicated code ({} lines)",
                        blocks.len(),
                        first_block.end_line - first_block.start_line + 1
                    ),
                    location: first_block.file.clone(),
                    line_number: Some(first_block.start_line),
                    debt_type: DebtType::Duplication,
                    impact: 5,
                    effort: 4,
                    tags: vec!["duplication".to_string()],
                });
            }
        }

        // Create priority queue
        let mut priority_queue = BinaryHeap::new();
        for item in &all_debt_items {
            priority_queue.push(item.clone());
        }

        Ok(TechnicalDebtMap {
            debt_items: all_debt_items,
            hotspots: all_hotspots,
            duplication_map,
            priority_queue,
        })
    }

    async fn update_debt_map(
        &self,
        project_path: &Path,
        current: &TechnicalDebtMap,
        changed_files: &[PathBuf],
    ) -> Result<TechnicalDebtMap> {
        let mut updated_map = current.clone();

        // Remove debt items for changed files
        updated_map
            .debt_items
            .retain(|item| !changed_files.contains(&item.location));
        updated_map
            .hotspots
            .retain(|hotspot| !changed_files.contains(&hotspot.file));

        // Re-analyze changed files
        for file in changed_files {
            if file.extension().and_then(|s| s.to_str()) == Some("rs") {
                let full_path = project_path.join(file);
                if let Ok(content) = tokio::fs::read_to_string(&full_path).await {
                    // Extract debt comments
                    let debt_items = self.extract_comments(file, &content).await;
                    updated_map.debt_items.extend(debt_items);

                    // Analyze complexity
                    let hotspots = self.analyze_complexity(file, &content).await;
                    updated_map.hotspots.extend(hotspots);
                }
            }
        }

        // Rebuild priority queue
        updated_map.priority_queue.clear();
        for item in &updated_map.debt_items {
            updated_map.priority_queue.push(item.clone());
        }

        Ok(updated_map)
    }
}
