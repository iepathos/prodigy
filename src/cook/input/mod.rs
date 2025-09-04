pub mod arguments;
pub mod config;
pub mod file_pattern;
pub mod legacy_adapter;
pub mod processor;
pub mod provider;
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
