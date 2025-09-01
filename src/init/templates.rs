use std::collections::HashMap;

pub struct CommandTemplate {
    pub name: &'static str,
    pub content: &'static str,
    pub description: &'static str,
}

pub const PRODIGY_CODE_REVIEW: &str = include_str!("../../.claude/commands/prodigy-code-review.md");
pub const PRODIGY_IMPLEMENT_SPEC: &str =
    include_str!("../../.claude/commands/prodigy-implement-spec.md");
pub const PRODIGY_LINT: &str = include_str!("../../.claude/commands/prodigy-lint.md");
pub const PRODIGY_PRODUCT_ENHANCE: &str =
    include_str!("../../.claude/commands/prodigy-product-enhance.md");
pub const PRODIGY_MERGE_WORKTREE: &str =
    include_str!("../../.claude/commands/prodigy-merge-worktree.md");
pub const PRODIGY_CLEANUP_TECH_DEBT: &str =
    include_str!("../../.claude/commands/prodigy-cleanup-tech-debt.md");

pub fn get_all_templates() -> Vec<CommandTemplate> {
    vec![
        CommandTemplate {
            name: "prodigy-code-review",
            description: "Analyzes code quality and creates improvement specs",
            content: PRODIGY_CODE_REVIEW,
        },
        CommandTemplate {
            name: "prodigy-implement-spec",
            description: "Implements Git Good specifications",
            content: PRODIGY_IMPLEMENT_SPEC,
        },
        CommandTemplate {
            name: "prodigy-lint",
            description: "Runs formatters, linters, and tests",
            content: PRODIGY_LINT,
        },
        CommandTemplate {
            name: "prodigy-product-enhance",
            description: "Product-focused improvements for user value",
            content: PRODIGY_PRODUCT_ENHANCE,
        },
        CommandTemplate {
            name: "prodigy-merge-worktree",
            description: "Claude-assisted worktree merging with conflict resolution",
            content: PRODIGY_MERGE_WORKTREE,
        },
        CommandTemplate {
            name: "prodigy-cleanup-tech-debt",
            description: "Analyzes technical debt and generates cleanup specifications",
            content: PRODIGY_CLEANUP_TECH_DEBT,
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
        assert!(template_names.contains(&"prodigy-code-review"));
        assert!(template_names.contains(&"prodigy-implement-spec"));
        assert!(template_names.contains(&"prodigy-lint"));
        assert!(template_names.contains(&"prodigy-cleanup-tech-debt"));

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
        let names = vec![
            "prodigy-code-review".to_string(),
            "prodigy-lint".to_string(),
        ];
        let templates = get_templates_by_names(&names);

        assert_eq!(templates.len(), 2);
        assert_eq!(templates[0].name, "prodigy-code-review");
        assert_eq!(templates[1].name, "prodigy-lint");

        // Test with non-existent template
        let names = vec!["non-existent".to_string()];
        let templates = get_templates_by_names(&names);
        assert_eq!(templates.len(), 0);
    }
}
