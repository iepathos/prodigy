//! Integration tests for conditional workflow execution

#[cfg(test)]
mod tests {
    use crate::config::command::WorkflowStepCommand;
    use crate::cook::expression::{ExpressionEvaluator, VariableContext};

    #[tokio::test]
    async fn test_when_clause_parsing() {
        // Test that when clause is properly parsed and stored
        let step = WorkflowStepCommand {
            claude: None,
            shell: Some("echo 'test'".to_string()),
            analyze: None,
            test: None,
            goal_seek: None,
            foreach: None,
            id: None,
            commit_required: false,
            analysis: None,
            outputs: None,
            capture_output: None,
            on_failure: None,
            retry: None,
            on_success: None,
            validate: None,
            timeout: None,
            when: Some("${condition} == true".to_string()),
            capture_format: None,
            capture_streams: None,
            output_file: None,
        };

        assert_eq!(step.when, Some("${condition} == true".to_string()));
    }

    #[tokio::test]
    async fn test_when_clause_with_complex_expression() {
        let step = WorkflowStepCommand {
            claude: Some("/prodigy-test".to_string()),
            shell: None,
            analyze: None,
            test: None,
            goal_seek: None,
            foreach: None,
            id: None,
            commit_required: false,
            analysis: None,
            outputs: None,
            capture_output: None,
            on_failure: None,
            retry: None,
            on_success: None,
            validate: None,
            timeout: None,
            when: Some("${build.success} && ${coverage} >= 80".to_string()),
            capture_format: None,
            capture_streams: None,
            output_file: None,
        };

        assert!(step.when.is_some());
        assert!(step.when.unwrap().contains("&&"));
    }

    #[test]
    fn test_expression_evaluation_with_context() {
        let evaluator = ExpressionEvaluator::new();
        let mut context = VariableContext::new();

        // Test simple boolean
        context.set_bool("flag".to_string(), true);
        assert!(evaluator.evaluate("${flag}", &context).unwrap());

        // Test comparison
        context.set_number("score".to_string(), 85.0);
        assert!(evaluator.evaluate("${score} >= 80", &context).unwrap());
        assert!(!evaluator.evaluate("${score} < 80", &context).unwrap());

        // Test string comparison
        context.set_string("env".to_string(), "production".to_string());
        assert!(evaluator
            .evaluate("${env} == 'production'", &context)
            .unwrap());
        assert!(!evaluator.evaluate("${env} == 'staging'", &context).unwrap());

        // Test logical operators
        context.set_bool("a".to_string(), true);
        context.set_bool("b".to_string(), false);
        assert!(!evaluator.evaluate("${a} && ${b}", &context).unwrap());
        assert!(evaluator.evaluate("${a} || ${b}", &context).unwrap());

        // Test step results
        context.set_step_result("build", true, 0, Some("Build successful".to_string()));
        assert!(evaluator.evaluate("${build.success}", &context).unwrap());
        assert!(evaluator
            .evaluate("${build.exit_code} == 0", &context)
            .unwrap());
    }

    #[test]
    fn test_complex_expressions() {
        let evaluator = ExpressionEvaluator::new();
        let mut context = VariableContext::new();

        context.set_number("coverage".to_string(), 75.0);
        context.set_bool("override".to_string(), true);
        context.set_string("branch".to_string(), "main".to_string());

        // Test complex condition
        assert!(evaluator
            .evaluate("${coverage} >= 70 || ${override} == true", &context)
            .unwrap());

        // Test with parentheses
        assert!(evaluator
            .evaluate(
                "(${coverage} >= 80 || ${override}) && ${branch} == 'main'",
                &context
            )
            .unwrap());

        // Test negation
        assert!(!evaluator.evaluate("!${override}", &context).unwrap());
    }

    #[test]
    fn test_undefined_variables() {
        let evaluator = ExpressionEvaluator::new();
        let context = VariableContext::new();

        // Undefined variables should evaluate to false
        assert!(!evaluator.evaluate("${undefined}", &context).unwrap());

        // But can check for existence
        assert!(!evaluator.evaluate("${undefined.exists}", &context).unwrap());
    }

    #[test]
    fn test_workflow_step_with_when_serialization() {
        let step = WorkflowStepCommand {
            claude: Some("/prodigy-test".to_string()),
            shell: None,
            analyze: None,
            test: None,
            goal_seek: None,
            foreach: None,
            id: None,
            commit_required: false,
            analysis: None,
            outputs: None,
            capture_output: None,
            on_failure: None,
            retry: None,
            on_success: None,
            validate: None,
            timeout: None,
            when: Some("${condition} == true".to_string()),
            capture_format: None,
            capture_streams: None,
            output_file: None,
        };

        let json = serde_json::to_string(&step).unwrap();
        assert!(json.contains("\"when\":"));
        assert!(json.contains("${condition} == true"));

        let deserialized: WorkflowStepCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.when, Some("${condition} == true".to_string()));
    }

    #[test]
    fn test_backward_compatibility() {
        // Step without when clause should work
        let json = r#"{
            "claude": "/prodigy-test",
            "commit_required": false
        }"#;

        let step: WorkflowStepCommand = serde_json::from_str(json).unwrap();
        assert_eq!(step.claude, Some("/prodigy-test".to_string()));
        assert_eq!(step.when, None);
    }
}
