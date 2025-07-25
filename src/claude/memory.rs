//! Conversation memory management

use crate::claude::response::ParsedResponse;
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use petgraph::graph::{Graph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::PathBuf;

/// Exchange between user and Claude
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exchange {
    pub prompt: String,
    pub response: String,
    pub metadata: ExchangeMetadata,
}

/// Metadata for an exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeMetadata {
    pub timestamp: DateTime<Utc>,
    pub tokens_used: usize,
    pub spec_name: String,
    pub iteration: u32,
    pub success: bool,
    pub key_concepts: Vec<String>,
}

/// Summary of multiple exchanges
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub content: String,
    pub exchanges_covered: usize,
    pub timestamp: DateTime<Utc>,
    pub key_points: Vec<String>,
}

/// Conversation memory with short-term and long-term storage
pub struct ConversationMemory {
    short_term: VecDeque<Exchange>,
    short_term_limit: usize,
    summaries: Vec<Summary>,
    concept_graph: Graph<String, f32>,
    concept_index: HashMap<String, NodeIndex>,
    memory_file: PathBuf,
}

impl ConversationMemory {
    /// Create a new conversation memory
    pub fn new() -> Self {
        Self::with_limit(10) // Keep last 10 exchanges by default
    }

    /// Create with custom short-term limit
    pub fn with_limit(limit: usize) -> Self {
        let memory_file = PathBuf::from(".mmm/conversation_memory.json");
        let (short_term, summaries) = Self::load_memory(&memory_file).unwrap_or_default();

        Self {
            short_term,
            short_term_limit: limit,
            summaries,
            concept_graph: Graph::new(),
            concept_index: HashMap::new(),
            memory_file,
        }
    }

    /// Add an exchange to memory
    pub fn add_exchange(&mut self, prompt: &str, response: &ParsedResponse) -> Result<()> {
        // Extract key concepts
        let key_concepts = self.extract_concepts(prompt, &response.content);

        let exchange = Exchange {
            prompt: prompt.to_string(),
            response: response.content.clone(),
            metadata: ExchangeMetadata {
                timestamp: Utc::now(),
                tokens_used: (prompt.len() + response.content.len()) / 4, // Rough estimate
                spec_name: "unknown".to_string(),                         // TODO: Get from context
                iteration: 0,                                             // TODO: Track iterations
                success: response.metadata.is_complete,
                key_concepts: key_concepts.clone(),
            },
        };

        // Update concept graph
        self.update_concept_graph(&key_concepts);

        // Add to short-term memory
        self.short_term.push_back(exchange);

        // Check if we need to summarize and compress
        if self.short_term.len() > self.short_term_limit {
            self.compress_to_summary()?;
        }

        self.save_memory()?;
        Ok(())
    }

    /// Get recent exchanges
    pub fn get_recent(&self, count: usize) -> Vec<&Exchange> {
        self.short_term.iter().rev().take(count).collect()
    }

    /// Get exchanges related to a specific spec
    pub fn get_by_spec(&self, spec_name: &str) -> Vec<&Exchange> {
        self.short_term
            .iter()
            .filter(|e| e.metadata.spec_name == spec_name)
            .collect()
    }

    /// Get related concepts
    pub fn get_related_concepts(&self, concept: &str) -> Vec<String> {
        if let Some(&node) = self.concept_index.get(concept) {
            self.concept_graph
                .neighbors(node)
                .filter_map(|n| self.concept_graph.node_weight(n))
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Build context from memory
    pub fn build_memory_context(&self, spec_name: &str, max_tokens: usize) -> String {
        let mut context_parts = Vec::new();
        let mut tokens_used = 0;

        // Add recent exchanges for this spec
        for exchange in self.get_by_spec(spec_name).into_iter().rev() {
            let exchange_text = format!(
                "Previous attempt ({}): {}\nResult: {}",
                exchange.metadata.timestamp.format("%Y-%m-%d %H:%M"),
                Self::truncate(&exchange.prompt, 200),
                Self::truncate(&exchange.response, 500)
            );

            let exchange_tokens = exchange_text.len() / 4;
            if tokens_used + exchange_tokens > max_tokens {
                break;
            }

            context_parts.push(exchange_text);
            tokens_used += exchange_tokens;
        }

        // Add relevant summaries
        for summary in &self.summaries {
            if summary.key_points.iter().any(|p| p.contains(spec_name)) {
                let summary_text = format!("Summary: {}", Self::truncate(&summary.content, 300));
                let summary_tokens = summary_text.len() / 4;

                if tokens_used + summary_tokens > max_tokens {
                    break;
                }

                context_parts.push(summary_text);
                tokens_used += summary_tokens;
            }
        }

        if context_parts.is_empty() {
            String::new()
        } else {
            format!("## Conversation History\n\n{}", context_parts.join("\n\n"))
        }
    }

    /// Extract key concepts from text
    fn extract_concepts(&self, prompt: &str, response: &str) -> Vec<String> {
        let mut concepts = Vec::new();
        let text = format!("{} {}", prompt, response);

        // Simple concept extraction - could be improved with NLP
        let keywords = [
            "spec",
            "implement",
            "feature",
            "module",
            "function",
            "class",
            "error",
            "test",
        ];

        for keyword in &keywords {
            if text.contains(keyword) {
                // Extract words around keyword
                let words: Vec<&str> = text.split_whitespace().collect();
                for (i, word) in words.iter().enumerate() {
                    if word.contains(keyword) && i + 1 < words.len() {
                        concepts.push(format!("{} {}", word, words[i + 1]));
                    }
                }
            }
        }

        concepts.dedup();
        concepts
    }

    /// Update concept graph with new concepts
    fn update_concept_graph(&mut self, concepts: &[String]) {
        // Add nodes for new concepts
        for concept in concepts {
            if !self.concept_index.contains_key(concept) {
                let idx = self.concept_graph.add_node(concept.clone());
                self.concept_index.insert(concept.clone(), idx);
            }
        }

        // Add edges between co-occurring concepts
        for i in 0..concepts.len() {
            for j in i + 1..concepts.len() {
                if let (Some(&idx1), Some(&idx2)) = (
                    self.concept_index.get(&concepts[i]),
                    self.concept_index.get(&concepts[j]),
                ) {
                    self.concept_graph.add_edge(idx1, idx2, 1.0);
                }
            }
        }
    }

    /// Compress old exchanges into a summary
    fn compress_to_summary(&mut self) -> Result<()> {
        // Take oldest exchanges
        let to_compress: Vec<Exchange> =
            (0..5).filter_map(|_| self.short_term.pop_front()).collect();

        if to_compress.is_empty() {
            return Ok(());
        }

        // Create summary
        let key_points: Vec<String> = to_compress
            .iter()
            .flat_map(|e| e.metadata.key_concepts.clone())
            .collect();

        let content = format!(
            "Summarized {} exchanges from {} to {}. Key topics: {}",
            to_compress.len(),
            to_compress
                .first()
                .unwrap()
                .metadata
                .timestamp
                .format("%Y-%m-%d"),
            to_compress
                .last()
                .unwrap()
                .metadata
                .timestamp
                .format("%Y-%m-%d"),
            key_points.join(", ")
        );

        let summary = Summary {
            content,
            exchanges_covered: to_compress.len(),
            timestamp: Utc::now(),
            key_points,
        };

        self.summaries.push(summary);
        Ok(())
    }

    /// Truncate text to max length
    fn truncate(text: &str, max_len: usize) -> &str {
        if text.len() <= max_len {
            text
        } else {
            &text[..max_len]
        }
    }

    /// Load memory from file
    fn load_memory(path: &PathBuf) -> Result<(VecDeque<Exchange>, Vec<Summary>)> {
        if !path.exists() {
            return Ok((VecDeque::new(), Vec::new()));
        }

        let content = fs::read_to_string(path).map_err(|e| Error::Io(e))?;

        let data: MemoryData = serde_json::from_str(&content)
            .map_err(|e| Error::Parse(format!("Invalid memory JSON: {}", e)))?;

        Ok((data.short_term.into(), data.summaries))
    }

    /// Save memory to file
    fn save_memory(&self) -> Result<()> {
        let data = MemoryData {
            short_term: self.short_term.iter().cloned().collect(),
            summaries: self.summaries.clone(),
        };

        // Create directory if needed
        if let Some(parent) = self.memory_file.parent() {
            fs::create_dir_all(parent).map_err(|e| Error::Io(e))?;
        }

        let json = serde_json::to_string_pretty(&data)
            .map_err(|e| Error::Parse(format!("Failed to serialize memory: {}", e)))?;

        fs::write(&self.memory_file, json).map_err(|e| Error::Io(e))?;

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct MemoryData {
    short_term: Vec<Exchange>,
    summaries: Vec<Summary>,
}
