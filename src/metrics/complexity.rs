//! Code complexity metrics calculation

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use syn::{visit::Visit, Block, Expr};
use tracing::debug;

/// Complexity metrics data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityMetrics {
    pub cyclomatic_complexity: HashMap<String, u32>,
    pub cognitive_complexity: HashMap<String, u32>,
    pub max_nesting_depth: u32,
    pub total_lines: u32,
}

/// Calculates code complexity metrics
pub struct ComplexityCalculator;

impl ComplexityCalculator {
    /// Create a new complexity calculator
    pub fn new() -> Self {
        Self
    }

    /// Calculate complexity metrics for the project
    pub fn calculate(&self, project_path: &Path) -> Result<ComplexityMetrics> {
        let mut metrics = ComplexityMetrics {
            cyclomatic_complexity: HashMap::new(),
            cognitive_complexity: HashMap::new(),
            max_nesting_depth: 0,
            total_lines: 0,
        };

        let src_dir = project_path.join("src");
        if !src_dir.exists() {
            return Ok(metrics);
        }

        // Walk through all Rust files
        for entry in walkdir::WalkDir::new(&src_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
        {
            let path = entry.path();
            let content = std::fs::read_to_string(path).context("Failed to read source file")?;

            // Count lines
            metrics.total_lines += content.lines().count() as u32;

            // Parse and analyze the file
            if let Ok(file) = syn::parse_file(&content) {
                let mut visitor = ComplexityVisitor::new(path.to_string_lossy().to_string());
                visitor.visit_file(&file);

                // Merge results
                for (name, complexity) in visitor.cyclomatic_complexity {
                    metrics
                        .cyclomatic_complexity
                        .insert(name.clone(), complexity);

                    // Estimate cognitive complexity (simplified)
                    let cognitive = complexity + visitor.nesting_penalties.get(&name).unwrap_or(&0);
                    metrics.cognitive_complexity.insert(name, cognitive);
                }

                metrics.max_nesting_depth = metrics.max_nesting_depth.max(visitor.max_nesting);
            }
        }

        debug!(
            "Analyzed {} functions with max nesting depth {}",
            metrics.cyclomatic_complexity.len(),
            metrics.max_nesting_depth
        );

        Ok(metrics)
    }
}

/// AST visitor for complexity analysis
struct ComplexityVisitor {
    file_path: String,
    current_function: Option<String>,
    cyclomatic_complexity: HashMap<String, u32>,
    nesting_penalties: HashMap<String, u32>,
    current_nesting: u32,
    max_nesting: u32,
}

impl ComplexityVisitor {
    fn new(file_path: String) -> Self {
        Self {
            file_path,
            current_function: None,
            cyclomatic_complexity: HashMap::new(),
            nesting_penalties: HashMap::new(),
            current_nesting: 0,
            max_nesting: 0,
        }
    }

    fn enter_scope(&mut self) {
        self.current_nesting += 1;
        self.max_nesting = self.max_nesting.max(self.current_nesting);

        // Add nesting penalty for cognitive complexity
        if let Some(ref func) = self.current_function {
            *self.nesting_penalties.entry(func.clone()).or_insert(0) += self.current_nesting;
        }
    }

    fn exit_scope(&mut self) {
        self.current_nesting = self.current_nesting.saturating_sub(1);
    }

    fn increment_complexity(&mut self) {
        if let Some(ref func) = self.current_function {
            *self.cyclomatic_complexity.entry(func.clone()).or_insert(1) += 1;
        }
    }
}

impl<'ast> Visit<'ast> for ComplexityVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let func_name = format!("{}::{}", self.file_path, node.sig.ident);
        self.current_function = Some(func_name.clone());
        self.cyclomatic_complexity.insert(func_name, 1); // Base complexity

        // Visit function body
        syn::visit::visit_item_fn(self, node);

        self.current_function = None;
        self.current_nesting = 0;
    }

    fn visit_expr(&mut self, node: &'ast Expr) {
        match node {
            // Control flow expressions increase complexity
            Expr::If(_) => {
                self.increment_complexity();
                self.enter_scope();
                syn::visit::visit_expr(self, node);
                self.exit_scope();
            }
            Expr::Match(expr_match) => {
                // Each arm adds to complexity
                for _ in &expr_match.arms {
                    self.increment_complexity();
                }
                self.enter_scope();
                syn::visit::visit_expr(self, node);
                self.exit_scope();
            }
            Expr::While(_) | Expr::ForLoop(_) | Expr::Loop(_) => {
                self.increment_complexity();
                self.enter_scope();
                syn::visit::visit_expr(self, node);
                self.exit_scope();
            }
            // Closure expressions
            Expr::Closure(_) => {
                self.enter_scope();
                syn::visit::visit_expr(self, node);
                self.exit_scope();
            }
            // Binary operations with && or ||
            Expr::Binary(expr_binary) => {
                match expr_binary.op {
                    syn::BinOp::And(_) | syn::BinOp::Or(_) => {
                        self.increment_complexity();
                    }
                    _ => {}
                }
                syn::visit::visit_expr(self, node);
            }
            _ => syn::visit::visit_expr(self, node),
        }
    }

    fn visit_block(&mut self, node: &'ast Block) {
        // Track nesting depth
        self.enter_scope();
        syn::visit::visit_block(self, node);
        self.exit_scope();
    }
}

impl Default for ComplexityCalculator {
    fn default() -> Self {
        Self::new()
    }
}
