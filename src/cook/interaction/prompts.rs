//! User prompting implementation

use anyhow::Result;
use async_trait::async_trait;
use std::io::{self, Write};

/// Trait for user prompting
#[async_trait]
pub trait UserPrompter: Send + Sync {
    /// Prompt for yes/no confirmation
    async fn prompt_yes_no(&self, message: &str) -> Result<bool>;

    /// Prompt for text input
    async fn prompt_text(&self, message: &str, default: Option<&str>) -> Result<String>;

    /// Prompt for choice from list
    async fn prompt_choice(&self, message: &str, choices: &[String]) -> Result<usize>;
}

/// Real implementation of user prompter
pub struct UserPrompterImpl;

impl UserPrompterImpl {
    pub fn new() -> Self {
        Self
    }

    fn read_line() -> Result<String> {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        Ok(input.trim().to_string())
    }
}

#[async_trait]
impl UserPrompter for UserPrompterImpl {
    async fn prompt_yes_no(&self, message: &str) -> Result<bool> {
        print!("{} [Y/n]: ", message);
        io::stdout().flush()?;

        let input = Self::read_line()?;
        let input = input.to_lowercase();

        Ok(input.is_empty() || input == "y" || input == "yes")
    }

    async fn prompt_text(&self, message: &str, default: Option<&str>) -> Result<String> {
        if let Some(default_value) = default {
            print!("{} [{}]: ", message, default_value);
        } else {
            print!("{}: ", message);
        }
        io::stdout().flush()?;

        let input = Self::read_line()?;

        if input.is_empty() && default.is_some() {
            Ok(default.unwrap().to_string())
        } else {
            Ok(input)
        }
    }

    async fn prompt_choice(&self, message: &str, choices: &[String]) -> Result<usize> {
        println!("{}", message);
        for (i, choice) in choices.iter().enumerate() {
            println!("  {}. {}", i + 1, choice);
        }
        print!("Enter choice (1-{}): ", choices.len());
        io::stdout().flush()?;

        loop {
            let input = Self::read_line()?;
            if let Ok(num) = input.parse::<usize>() {
                if num > 0 && num <= choices.len() {
                    return Ok(num - 1);
                }
            }
            print!("Invalid choice. Please enter a number between 1 and {}: ", choices.len());
            io::stdout().flush()?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub struct MockPrompter {
        responses: std::sync::Mutex<Vec<String>>,
    }

    impl MockPrompter {
        pub fn new(responses: Vec<String>) -> Self {
            Self {
                responses: std::sync::Mutex::new(responses),
            }
        }
    }

    #[async_trait]
    impl UserPrompter for MockPrompter {
        async fn prompt_yes_no(&self, _message: &str) -> Result<bool> {
            let mut responses = self.responses.lock().unwrap();
            if let Some(response) = responses.pop() {
                Ok(response.to_lowercase() == "y" || response.to_lowercase() == "yes")
            } else {
                anyhow::bail!("No mock response available")
            }
        }

        async fn prompt_text(&self, _message: &str, default: Option<&str>) -> Result<String> {
            let mut responses = self.responses.lock().unwrap();
            if let Some(response) = responses.pop() {
                if response.is_empty() && default.is_some() {
                    Ok(default.unwrap().to_string())
                } else {
                    Ok(response)
                }
            } else {
                anyhow::bail!("No mock response available")
            }
        }

        async fn prompt_choice(&self, _message: &str, choices: &[String]) -> Result<usize> {
            let mut responses = self.responses.lock().unwrap();
            if let Some(response) = responses.pop() {
                let num: usize = response.parse()?;
                if num > 0 && num <= choices.len() {
                    Ok(num - 1)
                } else {
                    anyhow::bail!("Invalid choice")
                }
            } else {
                anyhow::bail!("No mock response available")
            }
        }
    }

    #[tokio::test]
    async fn test_mock_prompter_yes_no() {
        let prompter = MockPrompter::new(vec!["n".to_string(), "yes".to_string()]);

        // First response should be "yes"
        assert!(prompter.prompt_yes_no("Test?").await.unwrap());

        // Second response should be "n"
        assert!(!prompter.prompt_yes_no("Test?").await.unwrap());
    }

    #[tokio::test]
    async fn test_mock_prompter_text() {
        let prompter = MockPrompter::new(vec!["".to_string(), "custom".to_string()]);

        // First response should be "custom"
        assert_eq!(prompter.prompt_text("Enter text", None).await.unwrap(), "custom");

        // Second response should use default
        assert_eq!(
            prompter.prompt_text("Enter text", Some("default")).await.unwrap(),
            "default"
        );
    }

    #[tokio::test]
    async fn test_mock_prompter_choice() {
        let prompter = MockPrompter::new(vec!["2".to_string()]);
        let choices = vec!["Option A".to_string(), "Option B".to_string()];

        let choice = prompter.prompt_choice("Choose", &choices).await.unwrap();
        assert_eq!(choice, 1); // 0-based index
    }
}