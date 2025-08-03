use std::collections::HashMap;

pub struct CommandTemplate {
    pub name: &'static str,
    pub content: &'static str,
    pub description: &'static str,
}

pub const MMM_CODE_REVIEW: &str = include_str!("../../.claude/commands/mmm-code-review.md");
pub const MMM_IMPLEMENT_SPEC: &str = include_str!("../../.claude/commands/mmm-implement-spec.md");
pub const MMM_LINT: &str = include_str!("../../.claude/commands/mmm-lint.md");
pub const MMM_PRODUCT_ENHANCE: &str = include_str!("../../.claude/commands/mmm-product-enhance.md");
pub const MMM_MERGE_WORKTREE: &str = include_str!("../../.claude/commands/mmm-merge-worktree.md");
pub const MMM_CLEANUP_TECH_DEBT: &str =
    include_str!("../../.claude/commands/mmm-cleanup-tech-debt.md");

pub fn get_all_templates() -> Vec<CommandTemplate> {
    vec![
        CommandTemplate {
            name: "mmm-code-review",
            description: "Analyzes code quality and creates improvement specs",
            content: MMM_CODE_REVIEW,
        },
        CommandTemplate {
            name: "mmm-implement-spec",
            description: "Implements Git Good specifications",
            content: MMM_IMPLEMENT_SPEC,
        },
        CommandTemplate {
            name: "mmm-lint",
            description: "Runs formatters, linters, and tests",
            content: MMM_LINT,
        },
        CommandTemplate {
            name: "mmm-product-enhance",
            description: "Product-focused improvements for user value",
            content: MMM_PRODUCT_ENHANCE,
        },
        CommandTemplate {
            name: "mmm-merge-worktree",
            description: "Claude-assisted worktree merging with conflict resolution",
            content: MMM_MERGE_WORKTREE,
        },
        CommandTemplate {
            name: "mmm-cleanup-tech-debt",
            description: "Analyzes technical debt and generates cleanup specifications",
            content: MMM_CLEANUP_TECH_DEBT,
        },
    ]
}

pub fn get_templates_by_names(names: &[String]) -> Vec<CommandTemplate> {
    let all_templates = get_all_templates();
    let name_set: HashMap<&str, bool> = names.iter().map(|n| (n.as_str(), true)).collect();

    all_templates
        .into_iter()
        .filter(|t| name_set.contains_key(t.name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_all_templates_success() {
        // Test normal operation
        let templates = get_all_templates();

        // Verify expected templates are present
        assert!(templates.len() >= 4); // At least the core templates

        let template_names: Vec<&str> = templates.iter().map(|t| t.name).collect();
        assert!(template_names.contains(&"mmm-code-review"));
        assert!(template_names.contains(&"mmm-implement-spec"));
        assert!(template_names.contains(&"mmm-lint"));
        assert!(template_names.contains(&"mmm-cleanup-tech-debt"));

        // Verify each template has required fields
        for template in &templates {
            assert!(!template.name.is_empty());
            assert!(!template.description.is_empty());
            assert!(!template.content.is_empty());
        }
    }

    #[test]
    fn test_get_templates_by_names() {
        // Test filtering templates by name
        let names = vec!["mmm-code-review".to_string(), "mmm-lint".to_string()];
        let templates = get_templates_by_names(&names);

        assert_eq!(templates.len(), 2);
        assert_eq!(templates[0].name, "mmm-code-review");
        assert_eq!(templates[1].name, "mmm-lint");

        // Test with non-existent template
        let names = vec!["non-existent".to_string()];
        let templates = get_templates_by_names(&names);
        assert_eq!(templates.len(), 0);
    }
}
