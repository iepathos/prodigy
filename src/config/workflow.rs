use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConfig {
    pub commands: Vec<String>,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
}

fn default_max_iterations() -> u32 {
    10
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            commands: vec![
                "mmm-code-review".to_string(),
                "mmm-implement-spec".to_string(),
                "mmm-lint".to_string(),
            ],
            max_iterations: 10,
        }
    }
}
