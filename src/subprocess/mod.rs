pub mod builder;
pub mod claude;
pub mod error;
pub mod git;
pub mod mock;
pub mod runner;

#[cfg(test)]
mod tests;

pub use builder::ProcessCommandBuilder;
pub use claude::ClaudeRunner;
pub use error::ProcessError;
pub use git::GitRunner;
pub use mock::{MockCommandConfig, MockProcessRunner};
pub use runner::ProcessCommand;
pub use runner::{ExitStatusHelper, ProcessOutput, ProcessRunner, ProcessStream};

use std::sync::Arc;

#[derive(Clone)]
pub struct SubprocessManager {
    runner: Arc<dyn ProcessRunner>,
}

impl SubprocessManager {
    pub fn new(runner: Arc<dyn ProcessRunner>) -> Self {
        Self { runner }
    }

    pub fn production() -> Self {
        Self::new(Arc::new(runner::TokioProcessRunner))
    }

    #[cfg(test)]
    pub fn mock() -> (Self, MockProcessRunner) {
        let mock = MockProcessRunner::new();
        let runner = Arc::new(mock.clone()) as Arc<dyn ProcessRunner>;
        (Self::new(runner), mock)
    }

    pub fn runner(&self) -> Arc<dyn ProcessRunner> {
        Arc::clone(&self.runner)
    }

    pub fn git(&self) -> git::GitRunnerImpl {
        git::GitRunnerImpl::new(Arc::clone(&self.runner))
    }

    pub fn claude(&self) -> claude::ClaudeRunnerImpl {
        claude::ClaudeRunnerImpl::new(Arc::clone(&self.runner))
    }
}
