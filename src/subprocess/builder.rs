use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use crate::subprocess::ProcessCommand;

pub struct ProcessCommandBuilder {
    command: ProcessCommand,
}

impl ProcessCommandBuilder {
    pub fn new(program: &str) -> Self {
        Self {
            command: ProcessCommand {
                program: program.to_string(),
                args: Vec::new(),
                env: HashMap::new(),
                working_dir: None,
                timeout: None,
                stdin: None,
            },
        }
    }

    pub fn arg(mut self, arg: &str) -> Self {
        self.command.args.push(arg.to_string());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.command
            .args
            .extend(args.into_iter().map(|s| s.as_ref().to_string()));
        self
    }

    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.command.env.insert(key.to_string(), value.to_string());
        self
    }

    pub fn envs<I, K, V>(mut self, vars: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        for (key, value) in vars {
            self.command
                .env
                .insert(key.as_ref().to_string(), value.as_ref().to_string());
        }
        self
    }

    pub fn current_dir(mut self, dir: &Path) -> Self {
        self.command.working_dir = Some(dir.to_path_buf());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.command.timeout = Some(timeout);
        self
    }

    pub fn stdin(mut self, input: String) -> Self {
        self.command.stdin = Some(input);
        self
    }

    pub fn build(self) -> ProcessCommand {
        self.command
    }
}
