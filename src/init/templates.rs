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
