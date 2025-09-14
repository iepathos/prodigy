use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result from validation command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Score from validation (0-100)
    pub score: u32,

    /// Whether threshold was met
    pub success: bool,

    /// Raw validation output
    pub output: String,

    /// Parsed gaps/issues (if validator outputs JSON)
    pub gaps: Option<serde_json::Value>,

    /// Additional structured data from validator
    pub data: HashMap<String, serde_json::Value>,
}

/// Trait for all validators
pub trait Validator: Send + Sync {
    /// Validate execution result against goal
    fn validate(&self, output: &str) -> Result<ValidationResult>;

    /// Get validator metadata
    fn name(&self) -> &str;
}

/// Score extractor that parses numeric scores from validation output
pub struct ScoreExtractor;

impl ScoreExtractor {
    pub fn extract_score_from_output(output: &str) -> u32 {
        // Look for patterns like "score: 85" or "85%" or "85/100"
        use regex::Regex;

        let patterns = vec![
            r"score:\s*(\d+)",
            r"(\d+)%",
            r"(\d+)/100",
            r"(\d+)\s*out\s*of\s*100",
        ];

        for pattern in patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(captures) = re.captures(output) {
                    if let Some(score_str) = captures.get(1) {
                        if let Ok(score) = score_str.as_str().parse::<u32>() {
                            return score.min(100);
                        }
                    }
                }
            }
        }

        // Default to 0 if no score found
        0
    }

    /// Try to parse validation output as JSON first
    pub fn parse_structured_validation(output: &str, threshold: u32) -> Result<ValidationResult> {
        // Debug: Log the output we're trying to parse
        tracing::debug!("Parsing validation output: '{}'", output);

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(output) {
            let score = json.get("score").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

            let gaps = json.get("gaps").cloned();
            let mut data = HashMap::new();
            data.insert("validation_json".to_string(), json);

            return Ok(ValidationResult {
                score,
                success: score >= threshold,
                output: output.to_string(),
                gaps,
                data,
            });
        }

        // Fallback: try to extract numeric score from output
        let score = Self::extract_score_from_output(output);
        tracing::debug!("Extracted score: {}", score);

        Ok(ValidationResult {
            score,
            success: score >= threshold,
            output: output.to_string(),
            gaps: None,
            data: HashMap::new(),
        })
    }
}

/// Basic validator that uses command output score extraction
pub struct BasicValidator {
    name: String,
}

impl BasicValidator {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

impl Validator for BasicValidator {
    fn validate(&self, output: &str) -> Result<ValidationResult> {
        ScoreExtractor::parse_structured_validation(output, 0)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_score_from_output() {
        assert_eq!(ScoreExtractor::extract_score_from_output("score: 85"), 85);
        assert_eq!(ScoreExtractor::extract_score_from_output("85%"), 85);
        assert_eq!(ScoreExtractor::extract_score_from_output("85/100"), 85);
        assert_eq!(
            ScoreExtractor::extract_score_from_output("85 out of 100"),
            85
        );
        assert_eq!(
            ScoreExtractor::extract_score_from_output("no score here"),
            0
        );
    }

    #[test]
    fn test_parse_structured_validation() {
        let json_output = r#"{"score": 85, "gaps": ["missing tests"]}"#;
        let result = ScoreExtractor::parse_structured_validation(json_output, 80).unwrap();

        assert_eq!(result.score, 85);
        assert!(result.success);
        assert!(result.gaps.is_some());
    }
}
