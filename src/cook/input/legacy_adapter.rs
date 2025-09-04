use super::config::InputConfig;
use crate::cook::command::CookCommand;
use anyhow::Result;

/// Maintains compatibility with existing command line arguments
pub struct LegacyInputAdapter;

impl LegacyInputAdapter {
    /// Convert existing cook command to new input configuration
    pub fn from_cook_command(command: &CookCommand) -> Result<InputConfig> {
        let mut config = InputConfig::default();

        // Handle --args flag
        if !command.args.is_empty() {
            // Join all args with commas for backward compatibility
            let args_str = command.args.join(",");
            config = InputConfig::from_command_args(&args_str);
        }

        // Handle --map flag
        if !command.map.is_empty() {
            // If we have both --args and --map, create a composite source
            if !command.args.is_empty() {
                let args_source = super::config::InputSource::Arguments {
                    value: command.args.join(","),
                    separator: Some(",".to_string()),
                    validation: None,
                };

                let file_source = super::config::InputSource::FilePattern {
                    patterns: command.map.clone(),
                    recursive: false,
                    filters: None,
                };

                config.sources = vec![super::config::InputSource::Composite {
                    sources: vec![args_source, file_source],
                    merge_strategy: super::config::MergeStrategy::Sequential,
                }];
            } else {
                config = InputConfig::from_file_patterns(command.map.clone());
            }
        }

        // If no inputs specified, use empty source
        if command.args.is_empty() && command.map.is_empty() {
            config.sources = vec![super::config::InputSource::Empty];
        }

        Ok(config)
    }

    /// Check if the configuration supports MapReduce
    pub fn supports_mapreduce(config: &InputConfig) -> bool {
        config.sources.iter().any(|source| match source {
            super::config::InputSource::FilePattern { .. } => true,
            super::config::InputSource::StructuredData { .. } => true,
            super::config::InputSource::Composite { .. } => true,
            _ => false,
        })
    }

    /// Convert execution inputs to legacy format for existing code
    pub fn to_legacy_format(inputs: &[super::ExecutionInput]) -> Vec<String> {
        inputs
            .iter()
            .map(|input| {
                // Try to get the most appropriate value for legacy compatibility
                if let Some(file_path) = input.variables.get("file_path") {
                    file_path.to_string()
                } else if let Some(arg) = input.variables.get("arg") {
                    arg.to_string()
                } else if let Some(item) = input.variables.get("item") {
                    item.to_string()
                } else {
                    input.id.clone()
                }
            })
            .collect()
    }
}
