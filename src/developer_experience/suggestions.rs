//! Smart suggestions and context-aware help

use crate::improve::session::ImprovementType;
use colored::*;
use std::collections::HashMap;

/// Smart helper for context-aware suggestions
pub struct SmartHelper {
    usage_history: Vec<ImprovementType>,
    project_context: ProjectContext,
}

#[derive(Debug, Default)]
struct ProjectContext {
    language: String,
    framework: Option<String>,
    test_coverage: f32,
    #[allow(dead_code)]
    has_ci: bool,
    last_improvement: Option<ImprovementType>,
}

/// Next action suggestion
#[derive(Debug, Clone)]
pub struct NextAction {
    pub command: String,
    pub description: String,
    pub priority: Priority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Medium,
    High,
}

impl Default for SmartHelper {
    fn default() -> Self {
        Self::new()
    }
}

impl SmartHelper {
    /// Create a new smart helper
    pub fn new() -> Self {
        Self {
            usage_history: Vec::new(),
            project_context: ProjectContext::default(),
        }
    }

    /// Update project context from analysis
    pub fn update_context(
        &mut self,
        language: String,
        framework: Option<String>,
        test_coverage: f32,
    ) {
        self.project_context.language = language;
        self.project_context.framework = framework;
        self.project_context.test_coverage = test_coverage;
    }

    /// Record an improvement for learning
    pub fn record_improvement(&mut self, improvement_type: ImprovementType) {
        self.usage_history.push(improvement_type);
        self.project_context.last_improvement = Some(improvement_type);
    }

    /// Suggest the next action based on context
    pub fn suggest_next_action(&self) -> NextAction {
        match self.project_context.last_improvement {
            Some(ImprovementType::ErrorHandling) => NextAction {
                command: "mmm improve --focus tests".to_string(),
                description: "Error handling improved. Consider adding tests next".to_string(),
                priority: Priority::High,
            },
            Some(ImprovementType::Testing) => NextAction {
                command: "mmm improve --focus docs".to_string(),
                description: "Nice! Tests added. How about improving documentation?".to_string(),
                priority: Priority::Medium,
            },
            Some(ImprovementType::Documentation) => NextAction {
                command: "mmm improve --focus performance".to_string(),
                description: "Documentation updated. Ready for performance optimization?"
                    .to_string(),
                priority: Priority::Low,
            },
            _ => NextAction {
                command: "mmm improve".to_string(),
                description: "Run again for more improvements".to_string(),
                priority: Priority::Medium,
            },
        }
    }

    /// Get multiple suggestions based on project analysis
    pub fn get_suggestions(&self) -> Vec<NextAction> {
        let mut suggestions = Vec::new();

        // Test coverage suggestions
        if self.project_context.test_coverage < 60.0 {
            suggestions.push(NextAction {
                command: "mmm improve --focus tests".to_string(),
                description: format!(
                    "Test coverage is only {:.0}%. Let's improve that!",
                    self.project_context.test_coverage
                ),
                priority: Priority::High,
            });
        }

        // Error handling suggestions (simplified without health score)
        suggestions.push(NextAction {
            command: "mmm improve --focus errors".to_string(),
            description: "Several functions could use better error handling".to_string(),
            priority: Priority::High,
        });

        // Documentation suggestions
        suggestions.push(NextAction {
            command: "mmm improve --focus docs".to_string(),
            description: "Some public APIs are missing documentation".to_string(),
            priority: Priority::Medium,
        });

        // Performance suggestions
        if self
            .usage_history
            .iter()
            .filter(|&&t| t == ImprovementType::Performance)
            .count()
            == 0
        {
            suggestions.push(NextAction {
                command: "mmm improve --focus performance".to_string(),
                description: "Ready to optimize performance?".to_string(),
                priority: Priority::Low,
            });
        }

        // Sort by priority
        suggestions.sort_by_key(|s| std::cmp::Reverse(s.priority));
        suggestions.truncate(3); // Show top 3 suggestions

        suggestions
    }

    /// Display suggestions to the user
    pub fn display_suggestions(&self) {
        let suggestions = self.get_suggestions();

        if suggestions.is_empty() {
            return;
        }

        println!(
            "{} Based on your project, consider these improvements:",
            "🎯".bold()
        );
        println!();

        for (i, suggestion) in suggestions.iter().enumerate() {
            let priority_icon = match suggestion.priority {
                Priority::High => "🔴",
                Priority::Medium => "🟡",
                Priority::Low => "🟢",
            };

            println!(
                "{}. {} {}",
                (i + 1).to_string().cyan(),
                priority_icon,
                suggestion.description.bold()
            );
            println!("   {}", suggestion.command.cyan());
            println!();
        }
    }
}

/// Contextual help based on current operation
pub struct ContextualHelp {
    help_items: HashMap<String, HelpItem>,
}

pub struct HelpItem {
    title: String,
    content: String,
    related_commands: Vec<String>,
}

impl Default for ContextualHelp {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextualHelp {
    /// Create a new contextual help system
    pub fn new() -> Self {
        let mut help_items = HashMap::new();

        // First run help
        help_items.insert(
            "first_run".to_string(),
            HelpItem {
                title: "Getting Started with MMM".to_string(),
                content: "MMM analyzes your project and improves code quality automatically. \
                     It's completely safe - all changes are validated before applying."
                    .to_string(),
                related_commands: vec![
                    "mmm improve".to_string(),
                    "mmm improve --preview".to_string(),
                    "mmm config".to_string(),
                ],
            },
        );

        // Error handling help
        help_items.insert(
            "error_handling".to_string(),
            HelpItem {
                title: "Improving Error Handling".to_string(),
                content: "MMM will replace unwrap() calls with proper error propagation, \
                     add context to errors, and ensure all errors are handled gracefully."
                    .to_string(),
                related_commands: vec![
                    "mmm improve --focus errors".to_string(),
                    "mmm improve --conservative".to_string(),
                ],
            },
        );

        // Testing help
        help_items.insert(
            "testing".to_string(),
            HelpItem {
                title: "Adding Tests".to_string(),
                content: "MMM can generate unit tests for your functions, improve existing \
                     tests, and increase overall test coverage."
                    .to_string(),
                related_commands: vec![
                    "mmm improve --focus tests".to_string(),
                    "mmm test".to_string(),
                ],
            },
        );

        Self { help_items }
    }

    /// Get help for a specific context
    pub fn get_help(&self, context: &str) -> Option<&HelpItem> {
        self.help_items.get(context)
    }

    /// Display help for a context
    pub fn display_help(&self, context: &str) {
        if let Some(help) = self.get_help(context) {
            println!("{} {}", "ℹ️".blue(), help.title.blue().bold());
            println!();
            println!("{}", help.content);

            if !help.related_commands.is_empty() {
                println!();
                println!("{}", "Related commands:".dimmed());
                for cmd in &help.related_commands {
                    println!("  {} {}", "•".dimmed(), cmd.cyan());
                }
            }
            println!();
        }
    }
}

/// Learning system that improves suggestions over time
pub struct LearningSystem {
    success_patterns: HashMap<String, f32>,
    failure_patterns: HashMap<String, f32>,
}

impl Default for LearningSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl LearningSystem {
    pub fn new() -> Self {
        Self {
            success_patterns: HashMap::new(),
            failure_patterns: HashMap::new(),
        }
    }

    /// Record a successful improvement
    pub fn record_success(&mut self, pattern: String, impact: f32) {
        *self.success_patterns.entry(pattern).or_insert(0.0) += impact;
    }

    /// Record a failed improvement
    pub fn record_failure(&mut self, pattern: String) {
        *self.failure_patterns.entry(pattern).or_insert(0.0) += 1.0;
    }

    /// Get confidence score for a pattern
    pub fn get_confidence(&self, pattern: &str) -> f32 {
        let successes = self.success_patterns.get(pattern).copied().unwrap_or(0.0);
        let failures = self.failure_patterns.get(pattern).copied().unwrap_or(0.0);

        if successes + failures == 0.0 {
            0.5 // Neutral confidence for unknown patterns
        } else {
            successes / (successes + failures)
        }
    }

    /// Get top successful patterns
    pub fn top_patterns(&self, limit: usize) -> Vec<(String, f32)> {
        let mut patterns: Vec<_> = self
            .success_patterns
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        patterns.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        patterns.truncate(limit);
        patterns
    }
}
