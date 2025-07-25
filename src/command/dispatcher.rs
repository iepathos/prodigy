use super::{
    BatchCommand, Command, CommandContext, CommandHistory, CommandOutput, CommandRegistry,
};
use crate::{Error, Result};
use std::sync::Arc;

pub struct CommandDispatcher {
    registry: CommandRegistry,
    history: CommandHistory,
}

impl Default for CommandDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandDispatcher {
    pub fn new() -> Self {
        Self {
            registry: CommandRegistry::new(),
            history: CommandHistory::new(),
        }
    }

    pub fn register_command(&mut self, command: Arc<dyn Command>) {
        self.registry.register(command);
    }

    pub async fn dispatch(
        &mut self,
        command_line: &str,
        context: CommandContext,
    ) -> Result<CommandOutput> {
        let parts: Vec<String> = shell_words::split(command_line)
            .map_err(|e| Error::Command(format!("Failed to parse command: {e}")))?;

        if parts.is_empty() {
            return Err(Error::Command("Empty command".to_string()));
        }

        let command_name = &parts[0];
        let args = parts[1..].to_vec();

        self.history.add_command(command_line.to_string()).await?;

        let command = self
            .registry
            .get_command(command_name)
            .ok_or_else(|| Error::Command(format!("Unknown command: {command_name}")))?;

        command.validate_args(&args)?;

        let mut cmd_context = context;
        cmd_context.args = args;

        let output = command.execute(cmd_context).await?;

        self.history
            .add_result(command_line.to_string(), &output)
            .await?;

        Ok(output)
    }

    pub async fn dispatch_batch(
        &mut self,
        batch: BatchCommand,
        context: CommandContext,
    ) -> Result<Vec<CommandOutput>> {
        let mut results = Vec::new();

        for command in batch.commands {
            match self.dispatch(&command, context.clone()).await {
                Ok(output) => {
                    let success = output.success;
                    results.push(output);

                    if !success && batch.stop_on_error {
                        break;
                    }
                }
                Err(e) => {
                    if batch.stop_on_error {
                        return Err(e);
                    } else {
                        results.push(CommandOutput::failure(e.to_string()));
                    }
                }
            }
        }

        Ok(results)
    }

    pub fn list_commands(&self) -> Vec<(&str, &str)> {
        self.registry.list_commands()
    }

    pub async fn replay_command(
        &mut self,
        index: usize,
        context: CommandContext,
    ) -> Result<CommandOutput> {
        let command = self
            .history
            .get_command(index)
            .await?
            .ok_or_else(|| Error::Command(format!("No command at index {index}")))?;

        self.dispatch(&command, context).await
    }

    pub async fn get_history(&self, limit: Option<usize>) -> Result<Vec<String>> {
        self.history.get_recent(limit.unwrap_or(50)).await
    }

    pub fn autocomplete(&self, partial: &str) -> Vec<String> {
        self.registry.autocomplete(partial)
    }
}
