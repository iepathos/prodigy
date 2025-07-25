use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub mod engine;
pub mod parser;
pub mod template;

pub use engine::SpecificationEngine;
pub use parser::SpecParser;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Specification {
    pub id: String,
    pub name: String,
    pub content: String,
    pub metadata: SpecMetadata,
    pub status: SpecStatus,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecMetadata {
    pub objective: Option<String>,
    pub acceptance_criteria: Vec<String>,
    pub tags: Vec<String>,
    pub priority: Option<u32>,
    pub estimated_hours: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SpecStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Blocked,
}

impl Default for SpecStatus {
    fn default() -> Self {
        SpecStatus::Pending
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecExecution {
    pub spec_id: String,
    pub iteration: u32,
    pub command: String,
    pub input: String,
    pub output: String,
    pub status: ExecutionStatus,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Running,
    Success,
    Failed,
    Timeout,
}

impl Specification {
    pub fn new(id: String, name: String, content: String) -> Self {
        Self {
            id,
            name,
            content,
            metadata: SpecMetadata {
                objective: None,
                acceptance_criteria: Vec::new(),
                tags: Vec::new(),
                priority: None,
                estimated_hours: None,
            },
            status: SpecStatus::default(),
            dependencies: Vec::new(),
        }
    }
    
    pub fn is_ready(&self, completed_specs: &[String]) -> bool {
        self.dependencies.iter().all(|dep| completed_specs.contains(dep))
    }
    
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(Error::Specification("Specification ID cannot be empty".to_string()));
        }
        
        if self.name.is_empty() {
            return Err(Error::Specification("Specification name cannot be empty".to_string()));
        }
        
        if self.content.is_empty() {
            return Err(Error::Specification("Specification content cannot be empty".to_string()));
        }
        
        Ok(())
    }
}