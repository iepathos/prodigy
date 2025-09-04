pub mod arguments;
pub mod config;
pub mod environment;
pub mod file_pattern;
pub mod generated;
pub mod legacy_adapter;
pub mod processor;
pub mod provider;
pub mod standard_input;
pub mod structured_data;
pub mod types;

#[cfg(test)]
mod tests;

pub use config::{InputConfig, InputSource};
pub use legacy_adapter::LegacyInputAdapter;
pub use processor::InputProcessor;
pub use provider::InputProvider;
pub use types::{
    ExecutionInput, InputMetadata, InputType, ValidationRule, VariableDefinition, VariableType,
    VariableValue,
};
