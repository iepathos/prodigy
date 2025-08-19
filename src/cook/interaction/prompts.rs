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

impl Default for UserPrompterImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl UserPrompterImpl {
    pub fn new() -> Self {
        Self
    }

    fn read_line() -> Result<String> {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        Ok(input.trim().to_string())
    }

    /// Validate and parse a choice input
    /// Returns Some(index) if valid, None if invalid
    pub fn validate_choice_input(input: &str, num_choices: usize) -> Option<usize> {
        if num_choices == 0 {
            return None;
        }

        input.parse::<usize>().ok().and_then(|num| {
            if num > 0 && num <= num_choices {
                Some(num - 1)
            } else {
                None
            }
        })
    }

    /// Format choice prompt message
    pub fn format_choice_prompt(message: &str, choices: &[String]) -> String {
        let mut output = String::new();
        output.push_str(message);
        output.push('\n');
        for (i, choice) in choices.iter().enumerate() {
            output.push_str(&format!("  {}. {}\n", i + 1, choice));
        }
        output
    }

    /// Format choice input prompt
    pub fn format_choice_input_prompt(num_choices: usize) -> String {
        format!("Enter choice (1-{num_choices}): ")
    }

    /// Format invalid choice message  
    pub fn format_invalid_choice_message(num_choices: usize) -> String {
        format!("Invalid choice. Please enter a number between 1 and {num_choices}: ")
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
            print!("{message} [{default_value}]: ");
        } else {
            print!("{message}: ");
        }
        io::stdout().flush()?;

        let input = Self::read_line()?;

        if input.is_empty() {
            if let Some(def) = default {
                Ok(def.to_string())
            } else {
                Ok(input)
            }
        } else {
            Ok(input)
        }
    }

    async fn prompt_choice(&self, message: &str, choices: &[String]) -> Result<usize> {
        if choices.is_empty() {
            anyhow::bail!("No choices provided");
        }

        print!("{}", Self::format_choice_prompt(message, choices));
        print!("{}", Self::format_choice_input_prompt(choices.len()));
        io::stdout().flush()?;

        loop {
            let input = Self::read_line()?;
            if let Some(index) = Self::validate_choice_input(&input, choices.len()) {
                return Ok(index);
            }
            print!("{}", Self::format_invalid_choice_message(choices.len()));
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
                if response.is_empty() {
                    if let Some(def) = default {
                        Ok(def.to_string())
                    } else {
                        Ok(response)
                    }
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
        assert_eq!(
            prompter.prompt_text("Enter text", None).await.unwrap(),
            "custom"
        );

        // Second response should use default
        assert_eq!(
            prompter
                .prompt_text("Enter text", Some("default"))
                .await
                .unwrap(),
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

    #[test]
    fn test_validate_choice_input_valid_single_digit() {
        assert_eq!(UserPrompterImpl::validate_choice_input("1", 3), Some(0));
        assert_eq!(UserPrompterImpl::validate_choice_input("2", 3), Some(1));
        assert_eq!(UserPrompterImpl::validate_choice_input("3", 3), Some(2));
    }

    #[test]
    fn test_validate_choice_input_valid_multi_digit() {
        assert_eq!(UserPrompterImpl::validate_choice_input("10", 10), Some(9));
        assert_eq!(UserPrompterImpl::validate_choice_input("99", 100), Some(98));
    }

    #[test]
    fn test_validate_choice_input_invalid_zero() {
        assert_eq!(UserPrompterImpl::validate_choice_input("0", 3), None);
    }

    #[test]
    fn test_validate_choice_input_invalid_out_of_bounds() {
        assert_eq!(UserPrompterImpl::validate_choice_input("4", 3), None);
        assert_eq!(UserPrompterImpl::validate_choice_input("100", 10), None);
    }

    #[test]
    fn test_validate_choice_input_invalid_negative() {
        assert_eq!(UserPrompterImpl::validate_choice_input("-1", 3), None);
    }

    #[test]
    fn test_validate_choice_input_invalid_non_numeric() {
        assert_eq!(UserPrompterImpl::validate_choice_input("abc", 3), None);
        assert_eq!(UserPrompterImpl::validate_choice_input("1.5", 3), None);
        assert_eq!(UserPrompterImpl::validate_choice_input("", 3), None);
        assert_eq!(UserPrompterImpl::validate_choice_input(" ", 3), None);
    }

    #[test]
    fn test_validate_choice_input_empty_choices() {
        assert_eq!(UserPrompterImpl::validate_choice_input("1", 0), None);
    }

    #[test]
    fn test_validate_choice_input_single_choice() {
        assert_eq!(UserPrompterImpl::validate_choice_input("1", 1), Some(0));
        assert_eq!(UserPrompterImpl::validate_choice_input("2", 1), None);
    }

    #[test]
    fn test_format_choice_prompt() {
        let choices = vec!["Option A".to_string(), "Option B".to_string()];
        let formatted = UserPrompterImpl::format_choice_prompt("Choose an option:", &choices);
        assert_eq!(
            formatted,
            "Choose an option:\n  1. Option A\n  2. Option B\n"
        );
    }

    #[test]
    fn test_format_choice_prompt_empty() {
        let choices: Vec<String> = vec![];
        let formatted = UserPrompterImpl::format_choice_prompt("Choose:", &choices);
        assert_eq!(formatted, "Choose:\n");
    }

    #[test]
    fn test_format_choice_prompt_single() {
        let choices = vec!["Only Option".to_string()];
        let formatted = UserPrompterImpl::format_choice_prompt("Pick:", &choices);
        assert_eq!(formatted, "Pick:\n  1. Only Option\n");
    }

    #[test]
    fn test_format_choice_input_prompt() {
        assert_eq!(
            UserPrompterImpl::format_choice_input_prompt(3),
            "Enter choice (1-3): "
        );
        assert_eq!(
            UserPrompterImpl::format_choice_input_prompt(1),
            "Enter choice (1-1): "
        );
        assert_eq!(
            UserPrompterImpl::format_choice_input_prompt(10),
            "Enter choice (1-10): "
        );
    }

    #[test]
    fn test_format_invalid_choice_message() {
        assert_eq!(
            UserPrompterImpl::format_invalid_choice_message(3),
            "Invalid choice. Please enter a number between 1 and 3: "
        );
        assert_eq!(
            UserPrompterImpl::format_invalid_choice_message(1),
            "Invalid choice. Please enter a number between 1 and 1: "
        );
    }

    #[tokio::test]
    async fn test_prompt_choice_empty_choices() {
        let prompter = UserPrompterImpl::new();
        let choices: Vec<String> = vec![];
        let result = prompter.prompt_choice("Choose", &choices).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "No choices provided");
    }
}
