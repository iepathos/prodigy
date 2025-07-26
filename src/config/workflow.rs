use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConfig {
    pub steps: Vec<WorkflowStep>,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    #[serde(default)]
    pub continue_on_error: bool,
    #[serde(default)]
    pub extractors: HashMap<String, Extractor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub command: String,
    pub name: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "from")]
pub enum Extractor {
    #[serde(rename = "git")]
    Git { pattern: String },
    #[serde(rename = "file")]
    File { path: String, pattern: String },
    #[serde(rename = "output")]
    Output { pattern: String },
}

fn default_max_iterations() -> u32 {
    10
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            steps: vec![
                WorkflowStep {
                    command: "mmm-code-review".to_string(),
                    name: "Code Review".to_string(),
                    args: vec![],
                },
                WorkflowStep {
                    command: "mmm-implement-spec".to_string(),
                    name: "Implementation".to_string(),
                    args: vec!["${SPEC_ID}".to_string()],
                },
                WorkflowStep {
                    command: "mmm-lint".to_string(),
                    name: "Linting".to_string(),
                    args: vec![],
                },
            ],
            max_iterations: 10,
            continue_on_error: false,
            extractors: {
                let mut map = HashMap::new();
                map.insert(
                    "SPEC_ID".to_string(),
                    Extractor::Git {
                        pattern: r"iteration-([^\s]+)".to_string(),
                    },
                );
                map
            },
        }
    }
}

impl WorkflowConfig {
    pub fn interpolate_args(
        &self,
        args: &[String],
        values: &HashMap<String, String>,
    ) -> Vec<String> {
        args.iter()
            .map(|arg| {
                let mut result = arg.clone();
                for (key, value) in values {
                    result = result.replace(&format!("${{{}}}", key), value);
                }
                result
            })
            .collect()
    }
}
