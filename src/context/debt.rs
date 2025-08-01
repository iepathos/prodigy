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
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
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

/// Configuration for debt aggregation
#[derive(Debug, Clone)]
pub struct DebtAggregationConfig {
    pub max_items_per_category: usize,
    pub max_total_items: usize,
    pub max_duplication_entries: usize,
    pub min_impact_threshold: u32,
}

impl Default for DebtAggregationConfig {
    fn default() -> Self {
        Self {
            max_items_per_category: 100,
            max_total_items: 500,
            max_duplication_entries: 100,
            min_impact_threshold: 3,
        }
    }
}

/// Aggregated debt summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtSummary {
    pub category: DebtType,
    pub total_count: usize,
    pub shown_count: usize,
    pub top_items: Vec<DebtItem>,
    pub aggregated_impact: f32,
}

/// Basic technical debt mapper implementation
pub struct BasicTechnicalDebtMapper {
    aggregation_config: DebtAggregationConfig,
}

impl Default for BasicTechnicalDebtMapper {
    fn default() -> Self {
        Self::new()
    }
}

impl BasicTechnicalDebtMapper {
    pub fn new() -> Self {
        Self {
            aggregation_config: DebtAggregationConfig::default(),
        }
    }
    
    pub fn with_config(config: DebtAggregationConfig) -> Self {
        Self {
            aggregation_config: config,
        }
    }

    /// Find debt comments in code (for tests)
    pub fn find_debt_comments(&self, content: &str, filename: &str) -> Vec<DebtItem> {
        let path = Path::new(filename);
        tokio::runtime::Runtime::new()
            .unwrap_or_else(|e| panic!("Failed to create tokio runtime: {e}"))
            .block_on(self.extract_comments(path, content))
    }

    /// Calculate complexity of code (for tests)
    pub fn calculate_complexity(&self, content: &str) -> u32 {
        self.calculate_cyclomatic_complexity(content)
    }

    /// Find code duplication (for tests)
    pub fn find_duplication(
        &self,
        file_contents: &HashMap<String, String>,
    ) -> HashMap<String, Vec<CodeBlock>> {
        let files: Vec<(PathBuf, String)> = file_contents
            .iter()
            .map(|(path, content)| (PathBuf::from(path), content.clone()))
            .collect();

        self.detect_duplicates(&files)
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
            } else if line_upper.contains("XXX") {
                (DebtType::Fixme, "XXX")
            } else if line_upper.contains("DEPRECATED") || line_upper.contains("@DEPRECATED") {
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
    fn calculate_cyclomatic_complexity(&self, function_content: &str) -> u32 {
        let mut complexity = 1; // Base complexity

        // Count decision points
        for line in function_content.lines() {
            let line = line.trim();

            complexity += self.count_conditionals(line);
            complexity += self.count_loops(line);
            complexity += self.count_match_arms(line);
            complexity += self.count_early_returns(line);
            complexity += self.count_error_propagation(line);
        }

        complexity
    }

    /// Count conditional statements in a line
    fn count_conditionals(&self, line: &str) -> u32 {
        if line.starts_with("if ") || line.contains(" if ") {
            1
        } else {
            0
        }
    }

    /// Count loop statements in a line
    fn count_loops(&self, line: &str) -> u32 {
        if line.starts_with("for ") || line.starts_with("while ") || line.starts_with("loop ") {
            1
        } else {
            0
        }
    }

    /// Count match arms in a line
    fn count_match_arms(&self, line: &str) -> u32 {
        let trimmed = line.trim_end();
        if trimmed.ends_with(" => ") || trimmed.ends_with(" => {") {
            1
        } else {
            0
        }
    }

    /// Count early returns in a line
    fn count_early_returns(&self, line: &str) -> u32 {
        if line.starts_with("return ") && !line.contains("fn ") {
            1
        } else {
            0
        }
    }

    /// Count error propagation operators in a line
    fn count_error_propagation(&self, line: &str) -> u32 {
        if line.contains("?") && !line.contains("Option<") && !line.contains("Result<") {
            1
        } else {
            0
        }
    }

    /// Extract functions and calculate complexity
    async fn analyze_complexity(&self, file_path: &Path, content: &str) -> Vec<ComplexityHotspot> {
        let mut hotspots = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            if let Some(function_name) = self.extract_function_name(line) {
                if let Some((start, end)) = self.find_function_bounds(&lines, i) {
                    let hotspot = self.analyze_function_complexity(
                        file_path,
                        &lines,
                        &function_name,
                        start,
                        end,
                    );
                    if let Some(hotspot) = hotspot {
                        hotspots.push(hotspot);
                    }
                }
            }
        }

        hotspots
    }

    /// Extract function name from a line containing "fn"
    fn extract_function_name(&self, line: &str) -> Option<String> {
        if !line.trim().starts_with("fn ") && !line.contains(" fn ") {
            return None;
        }

        let name_start = line.find("fn ")?;
        let after_fn = &line[name_start + 3..];
        let name = after_fn.split(['(', '<']).next()?;
        Some(name.trim().to_string())
    }

    /// Find the start and end lines of a function body
    fn find_function_bounds(&self, lines: &[&str], start_index: usize) -> Option<(usize, usize)> {
        let mut depth = 0;
        let mut start_line = start_index;
        let mut end_line = start_index;
        let mut found_start = false;

        for (j, line) in lines[start_index..].iter().enumerate() {
            for ch in line.chars() {
                if ch == '{' {
                    if !found_start {
                        found_start = true;
                        start_line = start_index + j;
                    }
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                    if depth == 0 && found_start {
                        end_line = start_index + j;
                        return Some((start_line, end_line));
                    }
                }
            }
        }

        if found_start && end_line > start_line {
            Some((start_line, end_line))
        } else {
            None
        }
    }

    /// Analyze a single function's complexity
    fn analyze_function_complexity(
        &self,
        file_path: &Path,
        lines: &[&str],
        function_name: &str,
        start_line: usize,
        end_line: usize,
    ) -> Option<ComplexityHotspot> {
        let function_content = lines[start_line..=end_line].join("\n");
        let complexity = self.calculate_complexity(&function_content);

        if complexity > 10 {
            Some(ComplexityHotspot {
                file: file_path.to_path_buf(),
                function: function_name.to_string(),
                complexity,
                lines: (end_line - start_line + 1) as u32,
            })
        } else {
            None
        }
    }

    /// Detect maximal duplicates using efficient algorithm
    fn detect_duplicates(&self, files: &[(PathBuf, String)]) -> HashMap<String, Vec<CodeBlock>> {
        let mut hash_to_blocks: HashMap<String, Vec<CodeBlock>> = HashMap::new();
        let min_duplicate_lines = 3;
        let max_duplicate_lines = 100;

        // Build a map of line hashes to locations
        let mut line_hash_to_locations: HashMap<String, Vec<(PathBuf, usize, String)>> = HashMap::new();
        
        for (file_path, content) in files {
            for (line_idx, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                // Skip trivial lines
                if trimmed.is_empty() || trimmed == "{" || trimmed == "}" || trimmed.starts_with("//") {
                    continue;
                }
                
                let line_hash = format!("{:x}", md5::compute(trimmed));
                line_hash_to_locations
                    .entry(line_hash)
                    .or_default()
                    .push((file_path.clone(), line_idx, line.to_string()));
            }
        }

        // Find maximal duplicate blocks
        let mut processed_blocks: std::collections::HashSet<(PathBuf, usize, usize)> = std::collections::HashSet::new();
        
        for (file_path, content) in files {
            let lines: Vec<&str> = content.lines().collect();
            
            for start_idx in 0..lines.len() {
                if processed_blocks.contains(&(file_path.clone(), start_idx, start_idx)) {
                    continue;
                }
                
                let trimmed_start = lines[start_idx].trim();
                if trimmed_start.is_empty() || trimmed_start == "{" || trimmed_start == "}" {
                    continue;
                }
                
                let start_hash = format!("{:x}", md5::compute(trimmed_start));
                
                if let Some(locations) = line_hash_to_locations.get(&start_hash) {
                    if locations.len() > 1 {
                        // Found potential duplicate start, expand to find maximal block
                        for (other_file, other_start, _) in locations {
                            if other_file == file_path && *other_start == start_idx {
                                continue; // Skip self
                            }
                            
                            // Expand the duplicate block
                            let max_len = self.find_max_duplicate_length(
                                files,
                                file_path,
                                start_idx,
                                other_file,
                                *other_start,
                                max_duplicate_lines,
                            );
                            
                            if max_len >= min_duplicate_lines {
                                // Create hash for the entire block
                                let block_content = lines[start_idx..start_idx + max_len].join("\n");
                                let block_hash = format!("{:x}", md5::compute(&block_content));
                                
                                // Add both blocks
                                let blocks = hash_to_blocks.entry(block_hash.clone()).or_default();
                                
                                // Check if we already have this block (avoid duplicates)
                                let block1 = CodeBlock {
                                    file: file_path.clone(),
                                    start_line: start_idx as u32 + 1,
                                    end_line: (start_idx + max_len) as u32,
                                    content_hash: block_hash.clone(),
                                };
                                
                                let block2 = CodeBlock {
                                    file: other_file.clone(),
                                    start_line: *other_start as u32 + 1,
                                    end_line: (*other_start + max_len) as u32,
                                    content_hash: block_hash.clone(),
                                };
                                
                                if !blocks.iter().any(|b| b.file == block1.file && b.start_line == block1.start_line) {
                                    blocks.push(block1);
                                }
                                if !blocks.iter().any(|b| b.file == block2.file && b.start_line == block2.start_line) {
                                    blocks.push(block2);
                                }
                                
                                // Mark these ranges as processed
                                for i in 0..max_len {
                                    processed_blocks.insert((file_path.clone(), start_idx + i, start_idx + i));
                                    processed_blocks.insert((other_file.clone(), *other_start + i, *other_start + i));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Filter out single blocks and merge overlapping blocks
        hash_to_blocks.retain(|_, blocks| blocks.len() > 1);
        self.merge_overlapping_blocks(&mut hash_to_blocks);
        
        // Apply size limits to duplication map
        self.limit_duplication_entries(&mut hash_to_blocks);
        
        hash_to_blocks
    }
    
    /// Find maximum length of duplicate block between two positions
    fn find_max_duplicate_length(
        &self,
        files: &[(PathBuf, String)],
        file1: &PathBuf,
        start1: usize,
        file2: &PathBuf,
        start2: usize,
        max_len: usize,
    ) -> usize {
        let content1 = files.iter().find(|(p, _)| p == file1).map(|(_, c)| c).unwrap();
        let content2 = files.iter().find(|(p, _)| p == file2).map(|(_, c)| c).unwrap();
        
        let lines1: Vec<&str> = content1.lines().collect();
        let lines2: Vec<&str> = content2.lines().collect();
        
        let mut length = 0;
        while length < max_len 
            && start1 + length < lines1.len() 
            && start2 + length < lines2.len() 
            && lines1[start1 + length].trim() == lines2[start2 + length].trim() 
        {
            length += 1;
        }
        
        length
    }
    
    /// Merge overlapping duplicate blocks
    fn merge_overlapping_blocks(&self, hash_to_blocks: &mut HashMap<String, Vec<CodeBlock>>) {
        for blocks in hash_to_blocks.values_mut() {
            // Sort blocks by file and start line
            blocks.sort_by_key(|b| (b.file.clone(), b.start_line));
            
            // Merge overlapping blocks
            let mut merged: Vec<CodeBlock> = Vec::new();
            for block in blocks.drain(..) {
                if let Some(last) = merged.last_mut() {
                    if last.file == block.file && last.end_line >= block.start_line {
                        // Overlapping, extend the last block
                        last.end_line = last.end_line.max(block.end_line);
                        continue;
                    }
                }
                merged.push(block);
            }
            *blocks = merged;
        }
    }
    
    /// Limit the number of duplication entries to reduce file size
    fn limit_duplication_entries(&self, hash_to_blocks: &mut HashMap<String, Vec<CodeBlock>>) {
        if hash_to_blocks.len() <= self.aggregation_config.max_duplication_entries {
            return;
        }
        
        // Calculate impact score for each duplication group
        let mut scored_entries: Vec<(String, Vec<CodeBlock>, f32)> = hash_to_blocks
            .drain()
            .map(|(hash, blocks)| {
                let lines_duplicated = if let Some(first) = blocks.first() {
                    first.end_line - first.start_line + 1
                } else {
                    0
                };
                let impact_score = (blocks.len() as f32) * (lines_duplicated as f32);
                (hash, blocks, impact_score)
            })
            .collect();
        
        // Sort by impact score (descending)
        scored_entries.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(Ordering::Equal));
        
        // Keep only top entries
        hash_to_blocks.clear();
        for (hash, blocks, _) in scored_entries.into_iter().take(self.aggregation_config.max_duplication_entries) {
            hash_to_blocks.insert(hash, blocks);
        }
    }
    
    /// Aggregate debt items by category to reduce file size
    fn aggregate_debt_items(&self, debt_items: Vec<DebtItem>) -> Vec<DebtItem> {
        if debt_items.len() <= self.aggregation_config.max_total_items {
            return debt_items;
        }
        
        // Group by debt type
        let mut items_by_type: HashMap<DebtType, Vec<DebtItem>> = HashMap::new();
        for item in debt_items {
            items_by_type.entry(item.debt_type.clone()).or_default().push(item);
        }
        
        let mut aggregated_items = Vec::new();
        
        for (_debt_type, mut items) in items_by_type {
            // Sort by priority (impact/effort)
            items.sort_by(|a, b| b.cmp(a));
            
            // Keep top items per category
            let top_items: Vec<DebtItem> = items
                .into_iter()
                .filter(|item| item.impact >= self.aggregation_config.min_impact_threshold)
                .take(self.aggregation_config.max_items_per_category)
                .collect();
            
            aggregated_items.extend(top_items);
        }
        
        // Final limit on total items
        aggregated_items.sort_by(|a, b| b.cmp(a));
        aggregated_items.truncate(self.aggregation_config.max_total_items);
        
        aggregated_items
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
                // Don't filter out the root directory
                e.depth() == 0
                    || (!name.starts_with('.') && name != "target" && name != "node_modules")
            })
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_file()
                    && e.path().extension().and_then(|s| s.to_str()) == Some("rs")
            })
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

        // Apply aggregation to reduce file size
        let aggregated_debt_items = self.aggregate_debt_items(all_debt_items);
        
        // Create priority queue from aggregated items
        let mut priority_queue = BinaryHeap::new();
        for item in &aggregated_debt_items {
            priority_queue.push(item.clone());
        }

        Ok(TechnicalDebtMap {
            debt_items: aggregated_debt_items,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::test_helpers::*;

    #[test]
    fn test_debt_pattern_detection() {
        let mapper = BasicTechnicalDebtMapper::new();

        let content = r#"
            // TODO: Refactor this function
            fn messy_function() {
                // FIXME: This is a hack
                let x = 42; // HACK: Magic number
                
                // TODO(high): Critical security issue
                unsafe {
                    // XXX: This needs review
                }
            }
        "#;

        let debt_items = mapper.find_debt_comments(content, "test.rs");
        assert_eq!(debt_items.len(), 5);

        // Check different debt types
        assert!(debt_items
            .iter()
            .any(|d| matches!(d.debt_type, DebtType::Todo)));
        assert!(debt_items
            .iter()
            .any(|d| matches!(d.debt_type, DebtType::Fixme)));
        assert!(debt_items
            .iter()
            .any(|d| matches!(d.debt_type, DebtType::Hack)));

        // Check high priority items (high impact)
        assert!(debt_items.iter().any(|d| d.impact >= 7));
    }

    #[test]
    fn test_complexity_analysis() {
        let mapper = BasicTechnicalDebtMapper::new();

        // Simple function
        let simple = r#"
            fn add(a: i32, b: i32) -> i32 {
                a + b
            }
        "#;
        assert_eq!(mapper.calculate_complexity(simple), 1);

        // Complex function with conditions
        let complex = r#"
            fn complex_logic(x: i32) -> i32 {
                if x > 0 {
                    if x > 10 {
                        for i in 0..x {
                            if i % 2 == 0 {
                                continue;
                            }
                        }
                    } else {
                        while x < 10 {
                            x += 1;
                        }
                    }
                } else if x < -10 {
                    match x {
                        -20 => 1,
                        -30 => 2,
                        _ => 3,
                    }
                }
                x
            }
        "#;
        let complexity = mapper.calculate_complexity(complex);
        assert!(
            complexity > 5,
            "Complex function should have high complexity"
        );
    }

    #[test]
    fn test_duplication_detection() {
        let mapper = BasicTechnicalDebtMapper::new();

        let mut file_contents = std::collections::HashMap::new();
        file_contents.insert(
            "file1.rs".to_string(),
            r#"
            fn process_data() {
                validate();
                save();
                notify();
            }
            
            fn other_function() {
                do_something();
            }
            "#
            .to_string(),
        );
        file_contents.insert(
            "file2.rs".to_string(),
            r#"
            fn handle_request() {
                validate();
                save();
                notify();
            }
            
            fn another_function() {
                do_something_else();
            }
            "#
            .to_string(),
        );

        let duplication_map = mapper.find_duplication(&file_contents);

        assert!(!duplication_map.is_empty(), "Expected to find duplicates");

        // Should detect similar code blocks
        let duplicates: Vec<_> = duplication_map.values().flatten().collect();
        assert!(duplicates.len() >= 2);
    }

    #[test]
    fn test_debt_priority_ordering() {
        let high_debt = DebtItem {
            id: "1".to_string(),
            title: "High priority".to_string(),
            description: "Critical issue".to_string(),
            location: PathBuf::from("test.rs"),
            line_number: Some(1),
            debt_type: DebtType::Fixme,
            impact: 10,
            effort: 1,
            tags: vec![],
        };

        let low_debt = DebtItem {
            id: "2".to_string(),
            title: "Low priority".to_string(),
            description: "Minor issue".to_string(),
            location: PathBuf::from("test.rs"),
            line_number: Some(2),
            debt_type: DebtType::Todo,
            impact: 2,
            effort: 5,
            tags: vec![],
        };

        assert!(high_debt > low_debt);
    }

    #[tokio::test]
    async fn test_full_debt_mapping() {
        let mapper = BasicTechnicalDebtMapper::new();
        let temp_dir = TempDir::new().unwrap();
        let project_path = setup_test_project(&temp_dir);

        // Create test files with various debt indicators
        create_test_file(
            &project_path,
            "src/main.rs",
            r#"
            // TODO: Add error handling
            fn main() {
                let data = load_data(); // FIXME: This can panic
                process(data);
            }
            
            fn complex_function(x: i32) -> i32 {
                let mut result = 0;
                if x > 0 {
                    if x > 10 {
                        for i in 0..x {
                            if i % 2 == 0 {
                                continue;
                            }
                            if i % 3 == 0 {
                                result += 1;
                            }
                        }
                    } else {
                        while result < 10 {
                            result += 1;
                            if result == 5 {
                                break;
                            }
                        }
                    }
                } else if x < -10 {
                    match x {
                        -20 => return 1,
                        -30 => return 2,
                        -40 => return 3,
                        -50 => return 4,
                        _ => return 5,
                    }
                } else if x == 0 {
                    return 0;
                }
                
                for i in 0..5 {
                    if i > 3 {
                        result += 2;
                    }
                    if result > 100 {
                        return result;
                    }
                }
                
                if result % 2 == 0 {
                    result * 2
                } else {
                    result * 3
                }
            }
            "#,
        );

        // Ensure the file is flushed to disk
        std::fs::File::open(project_path.join("src/main.rs"))
            .unwrap()
            .sync_all()
            .unwrap();

        // Wait a bit to ensure file is written
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let debt_map = mapper.map_technical_debt(&project_path).await.unwrap();

        // The test should find at least the TODO and FIXME comments
        assert!(
            debt_map.debt_items.len() >= 2,
            "Expected at least 2 debt items, found {}",
            debt_map.debt_items.len()
        );
        assert!(
            !debt_map.hotspots.is_empty(),
            "Expected at least one complexity hotspot"
        );
        assert_eq!(debt_map.debt_items.len(), debt_map.priority_queue.len());
    }
}
