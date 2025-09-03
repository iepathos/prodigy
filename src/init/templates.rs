use std::collections::HashMap;

pub struct CommandTemplate {
    pub name: &'static str,
    pub content: &'static str,
    pub description: &'static str,
}

// Include all prodigy-* commands from .claude/commands/ directory
// This macro is generated at compile time to include all matching files
include!(concat!(env!("OUT_DIR"), "/command_includes.rs"));

pub fn get_all_templates() -> Vec<CommandTemplate> {
    // This function is generated at compile time to include all prodigy-* commands
    get_all_command_templates()
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

        // Verify we have templates (dynamically discovered)
        assert!(templates.len() >= 6); // At least the original 6 core templates

        let template_names: Vec<&str> = templates.iter().map(|t| t.name).collect();
        // Verify core templates are still included
        assert!(template_names.contains(&"prodigy-code-review"));
        assert!(template_names.contains(&"prodigy-implement-spec"));
        assert!(template_names.contains(&"prodigy-lint"));
        assert!(template_names.contains(&"prodigy-cleanup-tech-debt"));
        assert!(template_names.contains(&"prodigy-merge-worktree"));
        assert!(template_names.contains(&"prodigy-product-enhance"));

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
        // Templates might be in different order after dynamic discovery
        let template_names: Vec<&str> = templates.iter().map(|t| t.name).collect();
        assert!(template_names.contains(&"prodigy-code-review"));
        assert!(template_names.contains(&"prodigy-lint"));

        // Test with non-existent template
        let names = vec!["non-existent".to_string()];
        let templates = get_templates_by_names(&names);
        assert_eq!(templates.len(), 0);
    }
}
