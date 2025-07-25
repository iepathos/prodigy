//! Context management with priority queue optimization

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Priority levels for context items
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum Priority {
    Critical = 1000,
    High = 100,
    Medium = 10,
    Low = 1,
}

impl Ord for Priority {
    fn cmp(&self, other: &Self) -> Ordering {
        (*self as u32).cmp(&(*other as u32))
    }
}

impl PartialOrd for Priority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Source of context information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextSource {
    Specification(String),
    Code {
        file: String,
        lines: Option<(usize, usize)>,
    },
    Documentation(String),
    History {
        iteration: u32,
    },
    Project {
        field: String,
    },
    Custom(String),
}

/// A single context item
#[derive(Debug, Clone)]
pub struct ContextItem {
    pub content: String,
    pub priority: Priority,
    pub tokens: usize,
    pub source: ContextSource,
}

impl Eq for ContextItem {}

impl PartialEq for ContextItem {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.tokens == other.tokens
    }
}

impl Ord for ContextItem {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.tokens.cmp(&self.tokens)) // Prefer smaller items when priority is equal
    }
}

impl PartialOrd for ContextItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Context manager with intelligent window optimization
pub struct ContextManager {
    max_tokens: usize,
    items: BinaryHeap<ContextItem>,
}

impl ContextManager {
    /// Create a new context manager
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            items: BinaryHeap::new(),
        }
    }

    /// Add a context item
    pub fn add_item(&mut self, content: String, priority: Priority, source: ContextSource) {
        let tokens = Self::estimate_tokens(&content);
        self.items.push(ContextItem {
            content,
            priority,
            tokens,
            source,
        });
    }

    /// Add specification content (always critical priority)
    pub fn add_specification(&mut self, spec_name: String, content: String) {
        self.add_item(
            format!("## Specification: {spec_name}\n\n{content}"),
            Priority::Critical,
            ContextSource::Specification(spec_name),
        );
    }

    /// Add code context
    pub fn add_code(&mut self, file: String, content: String, priority: Priority) {
        self.add_item(
            format!("## Code from {file}\n\n```\n{content}\n```"),
            priority,
            ContextSource::Code { file, lines: None },
        );
    }

    /// Add documentation context
    pub fn add_documentation(&mut self, doc_name: String, content: String) {
        self.add_item(
            format!("## Documentation: {doc_name}\n\n{content}"),
            Priority::Medium,
            ContextSource::Documentation(doc_name),
        );
    }

    /// Build final context string within token limits
    pub fn build_context(&mut self, base_prompt: &str) -> Result<String> {
        let mut total_tokens = Self::estimate_tokens(base_prompt);
        let mut context_parts = Vec::new();
        let mut included_items = Vec::new();

        // Process items by priority
        while let Some(item) = self.items.pop() {
            if total_tokens + item.tokens > self.max_tokens {
                // Try to fit critical items with truncation
                if item.priority == Priority::Critical {
                    let available_tokens = self.max_tokens - total_tokens;
                    if available_tokens > 100 {
                        // Minimum useful size
                        let truncated = self.truncate_intelligently(&item, available_tokens);
                        included_items.push(truncated);
                    }
                }
                // Skip non-critical items that don't fit
            } else {
                total_tokens += item.tokens;
                included_items.push(item);
            }
        }

        // Sort included items for better readability
        included_items.sort_by(|a, b| {
            // Sort by source type for logical grouping
            match (&a.source, &b.source) {
                (ContextSource::Specification(_), ContextSource::Specification(_)) => {
                    Ordering::Equal
                }
                (ContextSource::Specification(_), _) => Ordering::Less,
                (_, ContextSource::Specification(_)) => Ordering::Greater,
                _ => Ordering::Equal,
            }
        });

        // Build final context
        for item in included_items {
            context_parts.push(item.content);
        }

        let context = if context_parts.is_empty() {
            base_prompt.to_string()
        } else {
            format!(
                "# Context\n\n{}\n\n# Task\n\n{}",
                context_parts.join("\n\n---\n\n"),
                base_prompt
            )
        };

        Ok(context)
    }

    /// Truncate content intelligently while preserving key information
    fn truncate_intelligently(&self, item: &ContextItem, max_tokens: usize) -> ContextItem {
        let max_chars = max_tokens * 4; // Rough estimate
        let content = if item.content.len() > max_chars {
            let truncated = item
                .content
                .chars()
                .take(max_chars - 20)
                .collect::<String>();
            format!("{truncated}... [truncated]")
        } else {
            item.content.clone()
        };

        ContextItem {
            content,
            priority: item.priority,
            tokens: max_tokens,
            source: item.source.clone(),
        }
    }

    /// Estimate token count for a string
    fn estimate_tokens(text: &str) -> usize {
        // Rough estimate: ~4 characters per token
        text.len() / 4
    }

    /// Clear all context items
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Get current token usage
    pub fn current_tokens(&self) -> usize {
        self.items.iter().map(|item| item.tokens).sum()
    }
}
