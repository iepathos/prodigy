//! Command type determination and validation
//!
//! This module handles the logic for determining and validating
//! command types from workflow steps.

use crate::commands::AttributeValue;
use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use crate::cook::workflow::{CommandType, WorkflowStep};
use std::collections::HashMap;

/// Determine command type from a workflow step
pub fn determine_command_type(step: &WorkflowStep) -> MapReduceResult<CommandType> {
    // Collect all specified command types
    let commands = collect_command_types(step);

    // Validate exactly one command is specified
    validate_command_count(&commands)?;

    // Extract and return the single command type
    commands
        .into_iter()
        .next()
        .ok_or_else(|| MapReduceError::InvalidConfiguration {
            reason: "No valid command found in step".to_string(),
            field: "command".to_string(),
            value: "<none>".to_string(),
        })
}

/// Collect all command types from a workflow step
pub fn collect_command_types(step: &WorkflowStep) -> Vec<CommandType> {
    let mut commands = Vec::new();

    if let Some(claude) = &step.claude {
        commands.push(CommandType::Claude(claude.clone()));
    }
    if let Some(shell) = &step.shell {
        commands.push(CommandType::Shell(shell.clone()));
    }
    if let Some(handler) = &step.handler {
        commands.push(build_handler_command(handler));
    }
    if let Some(test) = &step.test {
        commands.push(CommandType::Test(test.clone()));
    }
    if let Some(goal_seek) = &step.goal_seek {
        commands.push(CommandType::GoalSeek(goal_seek.clone()));
    }
    if let Some(foreach) = &step.foreach {
        commands.push(CommandType::Foreach(foreach.clone()));
    }

    commands
}

/// Build handler command type from handler step configuration
fn build_handler_command(handler: &crate::cook::workflow::HandlerStep) -> CommandType {
    let mut converted_attributes = HashMap::new();

    for (key, value) in &handler.attributes {
        let attr_value = convert_json_to_attribute_value(value);
        converted_attributes.insert(key.clone(), attr_value);
    }

    CommandType::Handler {
        handler_name: handler.name.clone(),
        attributes: converted_attributes,
    }
}

/// Convert JSON value to AttributeValue
fn convert_json_to_attribute_value(value: &serde_json::Value) -> AttributeValue {
    match value {
        serde_json::Value::String(s) => AttributeValue::from(s.clone()),
        serde_json::Value::Bool(b) => AttributeValue::from(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                AttributeValue::from(i as i32)
            } else if let Some(f) = n.as_f64() {
                AttributeValue::from(f)
            } else {
                AttributeValue::from(n.to_string())
            }
        }
        _ => AttributeValue::from(value.to_string()),
    }
}

/// Validate that exactly one command is specified
pub fn validate_command_count(commands: &[CommandType]) -> MapReduceResult<()> {
    match commands.len() {
        0 => Err(MapReduceError::InvalidConfiguration {
            reason: "No command type specified in step".to_string(),
            field: "command".to_string(),
            value: "<none>".to_string(),
        }),
        1 => Ok(()),
        n => {
            let types: Vec<String> = commands
                .iter()
                .map(|c| format!("{:?}", c))
                .collect();
            Err(MapReduceError::InvalidConfiguration {
                reason: format!("Multiple commands specified in single step: {}", n),
                field: "commands".to_string(),
                value: types.join(", "),
            })
        }
    }
}