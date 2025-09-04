#[cfg(test)]
mod tests {
    use super::super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_arguments_provider() {
        let provider = arguments::ArgumentsInputProvider;
        let mut config = provider::InputConfig::new();
        config.set("args".to_string(), json!("arg1,arg2,key=value"));

        let inputs = provider.generate_inputs(&config).await.unwrap();
        assert_eq!(inputs.len(), 3);

        // Test first argument
        assert_eq!(inputs[0].id, "arg_0");
        assert_eq!(inputs[0].variables.get("arg").unwrap().to_string(), "arg1");

        // Test key=value parsing
        assert_eq!(inputs[2].id, "arg_2");
        assert_eq!(
            inputs[2].variables.get("arg_key").unwrap().to_string(),
            "key"
        );
        assert_eq!(
            inputs[2].variables.get("arg_value").unwrap().to_string(),
            "value"
        );
    }

    #[tokio::test]
    async fn test_empty_input_source() {
        let processor = processor::InputProcessor::new();
        let config = config::InputConfig::default();

        let inputs = processor.process_inputs(&config).await.unwrap();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].id, "empty");
    }

    #[test]
    fn test_variable_value_conversions() {
        use types::VariableValue;

        let str_val = VariableValue::String("test".to_string());
        assert_eq!(str_val.to_string(), "test");

        let num_val = VariableValue::Number(42);
        assert_eq!(num_val.as_number().unwrap(), 42);
        assert_eq!(num_val.to_string(), "42");

        let path_val = VariableValue::Path(std::path::PathBuf::from("/tmp/test"));
        assert_eq!(
            path_val.as_path().unwrap(),
            std::path::PathBuf::from("/tmp/test")
        );
    }

    #[test]
    fn test_execution_input_variable_substitution() {
        use types::{ExecutionInput, InputType, VariableValue};

        let mut input = ExecutionInput::new(
            "test_input".to_string(),
            InputType::Arguments {
                separator: Some(",".to_string()),
            },
        );
        input.add_variable(
            "name".to_string(),
            VariableValue::String("test".to_string()),
        );
        input.add_variable("count".to_string(), VariableValue::Number(5));

        let template = "Processing {name} with count {count} (ID: {input_id})";
        let result = input.substitute_in_template(template).unwrap();
        assert_eq!(result, "Processing test with count 5 (ID: test_input)");
    }

    #[test]
    fn test_execution_input_helper_functions() {
        use types::{ExecutionInput, InputType, VariableValue};

        let mut input = ExecutionInput::new("test".to_string(), InputType::Empty);
        input.add_variable(
            "path".to_string(),
            VariableValue::String("/path/to/file.txt".to_string()),
        );
        input.add_variable(
            "name".to_string(),
            VariableValue::String("Hello World".to_string()),
        );

        let template = "{path|basename} - {name|lowercase}";
        let result = input.substitute_in_template(template).unwrap();
        assert_eq!(result, "file.txt - hello world");
    }

    #[test]
    fn test_legacy_adapter() {
        use crate::cook::command::CookCommand;
        use legacy_adapter::LegacyInputAdapter;

        let mut cmd = CookCommand {
            playbook: std::path::PathBuf::from("test.yml"),
            path: None,
            max_iterations: 1,
            worktree: false,
            map: vec!["*.rs".to_string()],
            args: vec!["arg1".to_string(), "arg2".to_string()],
            fail_fast: false,
            auto_accept: false,
            metrics: false,
            resume: None,
            verbosity: 0,
            quiet: false,
        };

        let config = LegacyInputAdapter::from_cook_command(&cmd).unwrap();

        // Should create a composite source with both args and file pattern
        match &config.sources[0] {
            config::InputSource::Composite { sources, .. } => {
                assert_eq!(sources.len(), 2);
            }
            _ => panic!("Expected composite source"),
        }
    }

    #[tokio::test]
    async fn test_input_processor_with_transformations() {
        use config::{InputConfig, InputSource, TransformationConfig};

        let processor = processor::InputProcessor::new();

        let mut transformation = TransformationConfig::default();
        transformation
            .variable_transformations
            .insert("arg".to_string(), "uppercase".to_string());

        let config = InputConfig {
            sources: vec![InputSource::Arguments {
                value: "hello".to_string(),
                separator: Some(",".to_string()),
                validation: None,
            }],
            validation: Default::default(),
            transformation,
            caching: Default::default(),
        };

        let inputs = processor.process_inputs(&config).await.unwrap();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].variables.get("arg").unwrap().to_string(), "HELLO");
    }

    #[tokio::test]
    async fn test_composite_input_source() {
        use config::{InputConfig, InputSource, MergeStrategy};

        let processor = processor::InputProcessor::new();

        let config = InputConfig {
            sources: vec![InputSource::Composite {
                sources: vec![
                    InputSource::Arguments {
                        value: "a,b".to_string(),
                        separator: Some(",".to_string()),
                        validation: None,
                    },
                    InputSource::Arguments {
                        value: "c,d".to_string(),
                        separator: Some(",".to_string()),
                        validation: None,
                    },
                ],
                merge_strategy: MergeStrategy::Sequential,
            }],
            validation: Default::default(),
            transformation: Default::default(),
            caching: Default::default(),
        };

        let inputs = processor.process_inputs(&config).await.unwrap();
        assert_eq!(inputs.len(), 4);
    }

    #[tokio::test]
    async fn test_interleaved_merge_strategy() {
        use config::{InputConfig, InputSource, MergeStrategy};

        let processor = processor::InputProcessor::new();

        let config = InputConfig {
            sources: vec![InputSource::Composite {
                sources: vec![
                    InputSource::Arguments {
                        value: "a,b".to_string(),
                        separator: Some(",".to_string()),
                        validation: None,
                    },
                    InputSource::Arguments {
                        value: "1,2".to_string(),
                        separator: Some(",".to_string()),
                        validation: None,
                    },
                ],
                merge_strategy: MergeStrategy::Interleaved,
            }],
            validation: Default::default(),
            transformation: Default::default(),
            caching: Default::default(),
        };

        let inputs = processor.process_inputs(&config).await.unwrap();
        assert_eq!(inputs.len(), 4);

        // Check interleaved order: a, 1, b, 2
        assert_eq!(inputs[0].variables.get("arg").unwrap().to_string(), "a");
        assert_eq!(inputs[1].variables.get("arg").unwrap().to_string(), "1");
        assert_eq!(inputs[2].variables.get("arg").unwrap().to_string(), "b");
        assert_eq!(inputs[3].variables.get("arg").unwrap().to_string(), "2");
    }
}
