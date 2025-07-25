use super::Command;
use std::collections::HashMap;
use std::sync::Arc;

pub struct CommandRegistry {
    commands: HashMap<String, Arc<dyn Command>>,
    aliases: HashMap<String, String>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    pub fn register(&mut self, command: Arc<dyn Command>) {
        let name = command.name().to_string();

        for alias in command.aliases() {
            self.aliases.insert(alias.to_string(), name.clone());
        }

        self.commands.insert(name, command);
    }

    pub fn get_command(&self, name: &str) -> Option<Arc<dyn Command>> {
        self.commands.get(name).cloned().or_else(|| {
            self.aliases
                .get(name)
                .and_then(|actual_name| self.commands.get(actual_name).cloned())
        })
    }

    pub fn list_commands(&self) -> Vec<(&str, &str)> {
        let mut commands: Vec<_> = self
            .commands
            .iter()
            .map(|(name, cmd)| (name.as_str(), cmd.description()))
            .collect();

        commands.sort_by_key(|(name, _)| *name);
        commands
    }

    pub fn autocomplete(&self, partial: &str) -> Vec<String> {
        let mut suggestions = Vec::new();

        for name in self.commands.keys() {
            if name.starts_with(partial) {
                suggestions.push(name.clone());
            }
        }

        for alias in self.aliases.keys() {
            if alias.starts_with(partial) && !suggestions.contains(alias) {
                suggestions.push(alias.clone());
            }
        }

        suggestions.sort();
        suggestions
    }

    pub fn has_command(&self, name: &str) -> bool {
        self.commands.contains_key(name) || self.aliases.contains_key(name)
    }
}
