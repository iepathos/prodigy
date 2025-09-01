//! Tests for the spec validation system

#[cfg(test)]
mod tests {
    use crate::cook::workflow::validation::*;
    use crate::cook::workflow::{WorkflowContext, WorkflowStep, CaptureOutput};
    use crate::cook::interaction::MockUserInteraction;
    use crate::cook::execution::ClaudeExecutor;
    use crate::cook::orchestrator::ExecutionEnvironment;
    use crate::cook::session::SessionManager;
    use crate::cook::workflow::WorkflowExecutor as WorkflowExecutorImpl;
    use anyhow::Result;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;
    
    // Mock implementations for testing
    struct MockClaudeExecutor;
    
    #[async_trait]
    impl ClaudeExecutor for MockClaudeExecutor {
        async fn execute_claude_command(
            &self,
            _command: &str,
            _working_dir: &PathBuf,
            _env_vars: HashMap<String, String>,
        ) -> Result<crate::cook::execution::ExecutionResult> {
            Ok(crate::cook::execution::ExecutionResult {
                success: true,
                stdout: "Command executed".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
            })
        }
    }
    
    struct MockSessionManager;
    
    #[async_trait]
    impl SessionManager for MockSessionManager {
        async fn update_session(&self, _update: crate::cook::session::SessionUpdate) -> Result<()> {
            Ok(())
        }
        
        async fn get_session_state(&self) -> Result<crate::cook::session::SessionState> {
            Ok(crate::cook::session::SessionState::default())
        }
        
        async fn save_checkpoint(&self, _checkpoint: crate::cook::session::SessionCheckpoint) -> Result<()> {
            Ok(())
        }
    }
    
    #[test]
    fn test_validation_config_creation() {
        let config = ValidationConfig {
            validation_type: ValidationType::SpecCoverage,
            command: "/mmm-validate-spec 01".to_string(),
            expected_schema: None,
            threshold: 95.0,
            timeout: Some(30),
            on_incomplete: Some(OnIncompleteConfig {
                strategy: CompletionStrategy::PatchGaps,
                claude: Some("/mmm-fix-gaps".to_string()),
                shell: None,
                prompt: None,
                max_attempts: 2,
                fail_workflow: true,
            }),
        };
        
        assert!(config.validate().is_ok());
        assert_eq!(config.threshold, 95.0);
        assert_eq!(config.validation_type, ValidationType::SpecCoverage);
    }
    
    #[test]
    fn test_validation_result_helpers() {
        // Test complete result
        let result = ValidationResult::complete();
        assert_eq!(result.status, ValidationStatus::Complete);
        assert_eq!(result.completion_percentage, 100.0);
        
        // Test incomplete result
        let mut gaps = HashMap::new();
        gaps.insert(
            "auth".to_string(),
            GapDetail {
                description: "Missing authentication".to_string(),
                location: Some("src/auth.rs".to_string()),
                severity: Severity::Critical,
                suggested_fix: None,
            },
        );
        
        let incomplete = ValidationResult::incomplete(
            75.0,
            vec!["Authentication".to_string()],
            gaps,
        );
        assert_eq!(incomplete.status, ValidationStatus::Incomplete);
        assert_eq!(incomplete.completion_percentage, 75.0);
        assert_eq!(incomplete.missing.len(), 1);
        
        // Test failed result
        let failed = ValidationResult::failed("Error message".to_string());
        assert_eq!(failed.status, ValidationStatus::Failed);
        assert_eq!(failed.completion_percentage, 0.0);
    }
    
    #[test]
    fn test_validation_config_is_complete() {
        let config = ValidationConfig {
            validation_type: ValidationType::TestCoverage,
            command: "cargo test".to_string(),
            expected_schema: None,
            threshold: 80.0,
            timeout: None,
            on_incomplete: None,
        };
        
        let passing_result = ValidationResult {
            completion_percentage: 85.0,
            status: ValidationStatus::Complete,
            implemented: vec![],
            missing: vec![],
            gaps: HashMap::new(),
            raw_output: None,
        };
        
        let failing_result = ValidationResult {
            completion_percentage: 75.0,
            status: ValidationStatus::Incomplete,
            implemented: vec![],
            missing: vec!["Some tests".to_string()],
            gaps: HashMap::new(),
            raw_output: None,
        };
        
        assert!(config.is_complete(&passing_result));
        assert!(!config.is_complete(&failing_result));
    }
    
    #[test]
    fn test_workflow_context_interpolation_with_validation() {
        let mut ctx = WorkflowContext::default();
        
        // Add some validation results
        let validation = ValidationResult {
            completion_percentage: 85.5,
            status: ValidationStatus::Incomplete,
            implemented: vec!["Feature A".to_string()],
            missing: vec!["Feature B".to_string(), "Feature C".to_string()],
            gaps: {
                let mut gaps = HashMap::new();
                gaps.insert(
                    "feature_b".to_string(),
                    GapDetail {
                        description: "Feature B not implemented".to_string(),
                        location: None,
                        severity: Severity::High,
                        suggested_fix: None,
                    },
                );
                gaps
            },
            raw_output: None,
        };
        
        ctx.validation_results.insert("spec".to_string(), validation);
        
        // Test interpolation
        let template = "Completion: ${spec.completion}%, Missing: ${spec.missing}, Gaps: ${spec.gaps}";
        let result = ctx.interpolate(template);
        
        assert!(result.contains("85.5"));
        assert!(result.contains("Feature B, Feature C"));
        assert!(result.contains("Feature B not implemented"));
    }
    
    #[test]
    fn test_on_incomplete_config_validation() {
        // Valid config with claude command
        let valid = OnIncompleteConfig {
            strategy: CompletionStrategy::PatchGaps,
            claude: Some("/mmm-fix".to_string()),
            shell: None,
            prompt: None,
            max_attempts: 3,
            fail_workflow: false,
        };
        assert!(valid.validate().is_ok());
        assert!(valid.has_command());
        
        // Invalid - no command for patch_gaps
        let invalid = OnIncompleteConfig {
            strategy: CompletionStrategy::PatchGaps,
            claude: None,
            shell: None,
            prompt: None,
            max_attempts: 2,
            fail_workflow: true,
        };
        assert!(invalid.validate().is_err());
        assert!(!invalid.has_command());
        
        // Valid interactive with prompt
        let interactive = OnIncompleteConfig {
            strategy: CompletionStrategy::Interactive,
            claude: None,
            shell: None,
            prompt: Some("Continue?".to_string()),
            max_attempts: 1,
            fail_workflow: false,
        };
        assert!(interactive.validate().is_ok());
        
        // Invalid - zero max_attempts
        let zero_attempts = OnIncompleteConfig {
            strategy: CompletionStrategy::RetryFull,
            claude: Some("/mmm-retry".to_string()),
            shell: None,
            prompt: None,
            max_attempts: 0,
            fail_workflow: true,
        };
        assert!(zero_attempts.validate().is_err());
    }
    
    #[test]
    fn test_validation_workflow_step() {
        let step = WorkflowStep {
            name: None,
            claude: Some("/mmm-implement-spec 01".to_string()),
            shell: None,
            test: None,
            command: None,
            handler: None,
            timeout: None,
            capture_output: CaptureOutput::Disabled,
            on_failure: None,
            on_success: None,
            on_exit_code: Default::default(),
            commit_required: true,
            working_dir: None,
            env: Default::default(),
            validate: Some(ValidationConfig {
                validation_type: ValidationType::SpecCoverage,
                command: "/mmm-validate-spec 01".to_string(),
                expected_schema: None,
                threshold: 100.0,
                timeout: None,
                on_incomplete: Some(OnIncompleteConfig {
                    strategy: CompletionStrategy::PatchGaps,
                    claude: Some("/mmm-complete-spec 01".to_string()),
                    shell: None,
                    prompt: None,
                    max_attempts: 2,
                    fail_workflow: true,
                }),
            }),
        };
        
        assert!(step.validate.is_some());
        let validation = step.validate.unwrap();
        assert_eq!(validation.validation_type, ValidationType::SpecCoverage);
        assert_eq!(validation.threshold, 100.0);
    }
    
    #[tokio::test]
    async fn test_validation_execution_flow() {
        // This would test the full validation flow if we had a real executor
        // For now, just test the structures work together
        
        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: temp_dir.path().to_path_buf(),
            project_dir: temp_dir.path().to_path_buf(),
            claude_exe: PathBuf::from("claude"),
            environment: Default::default(),
        };
        
        let step = WorkflowStep {
            name: None,
            claude: Some("/test-command".to_string()),
            shell: None,
            test: None,
            command: None,
            handler: None,
            timeout: None,
            capture_output: CaptureOutput::Disabled,
            on_failure: None,
            on_success: None,
            on_exit_code: Default::default(),
            commit_required: false,
            working_dir: None,
            env: Default::default(),
            validate: Some(ValidationConfig {
                validation_type: ValidationType::SelfAssessment,
                command: "echo '{\"completion_percentage\": 100, \"status\": \"complete\"}'".to_string(),
                expected_schema: None,
                threshold: 100.0,
                timeout: None,
                on_incomplete: None,
            }),
        };
        
        // Just verify the structures compile and can be used
        assert!(step.validate.is_some());
    }
    
    #[test]
    fn test_gaps_summary() {
        let mut gaps = HashMap::new();
        gaps.insert(
            "rbac".to_string(),
            GapDetail {
                description: "Role-based access control missing".to_string(),
                location: Some("src/auth/rbac.rs".to_string()),
                severity: Severity::Critical,
                suggested_fix: Some("Implement RBAC middleware".to_string()),
            },
        );
        gaps.insert(
            "logging".to_string(),
            GapDetail {
                description: "Audit logging not implemented".to_string(),
                location: None,
                severity: Severity::Medium,
                suggested_fix: None,
            },
        );
        
        let result = ValidationResult {
            completion_percentage: 60.0,
            status: ValidationStatus::Incomplete,
            implemented: vec![],
            missing: vec![],
            gaps,
            raw_output: None,
        };
        
        let summary = result.gaps_summary();
        assert!(summary.contains("Role-based access control missing"));
        assert!(summary.contains("Audit logging not implemented"));
        assert!(summary.contains("critical"));
        assert!(summary.contains("medium"));
    }
}