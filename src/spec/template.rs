use crate::Result;
use std::collections::HashMap;

pub struct SpecTemplate {
    pub name: String,
    pub description: String,
    pub content: String,
    pub variables: Vec<TemplateVariable>,
}

pub struct TemplateVariable {
    pub name: String,
    pub description: String,
    pub default: Option<String>,
    pub required: bool,
}

impl SpecTemplate {
    pub fn feature() -> Self {
        Self {
            name: "feature".to_string(),
            description: "Template for new feature specifications".to_string(),
            content: r#"---
id: {{id}}
name: {{name}}
objective: {{objective}}
acceptance_criteria:
  - {{criteria1}}
dependencies: []
tags: [feature]
priority: {{priority}}
estimated_hours: {{hours}}
---

# Feature: {{name}}

## Objective
{{objective}}

## Acceptance Criteria
- [ ] {{criteria1}}
- [ ] Additional criteria here...

## Technical Details
Describe the technical approach and implementation details.

## User Stories
As a {{user_type}}, I want to {{action}} so that {{benefit}}.

## Implementation Notes
- Consider edge cases
- Performance implications
- Security considerations
"#
            .to_string(),
            variables: vec![
                TemplateVariable {
                    name: "id".to_string(),
                    description: "Unique identifier for the specification".to_string(),
                    default: None,
                    required: true,
                },
                TemplateVariable {
                    name: "name".to_string(),
                    description: "Human-readable name for the feature".to_string(),
                    default: None,
                    required: true,
                },
                TemplateVariable {
                    name: "objective".to_string(),
                    description: "Main objective of the feature".to_string(),
                    default: None,
                    required: true,
                },
                TemplateVariable {
                    name: "criteria1".to_string(),
                    description: "First acceptance criterion".to_string(),
                    default: None,
                    required: true,
                },
                TemplateVariable {
                    name: "priority".to_string(),
                    description: "Priority level (1-5)".to_string(),
                    default: Some("3".to_string()),
                    required: false,
                },
                TemplateVariable {
                    name: "hours".to_string(),
                    description: "Estimated hours for implementation".to_string(),
                    default: Some("8".to_string()),
                    required: false,
                },
                TemplateVariable {
                    name: "user_type".to_string(),
                    description: "Type of user for the user story".to_string(),
                    default: Some("user".to_string()),
                    required: false,
                },
                TemplateVariable {
                    name: "action".to_string(),
                    description: "Action the user wants to perform".to_string(),
                    default: None,
                    required: true,
                },
                TemplateVariable {
                    name: "benefit".to_string(),
                    description: "Benefit the user receives".to_string(),
                    default: None,
                    required: true,
                },
            ],
        }
    }

    pub fn render(&self, values: &HashMap<String, String>) -> Result<String> {
        let mut result = self.content.clone();

        for var in &self.variables {
            let value = values
                .get(&var.name)
                .or(var.default.as_ref())
                .ok_or_else(|| {
                    crate::Error::Specification(format!(
                        "Required variable '{}' not provided",
                        var.name
                    ))
                })?;

            result = result.replace(&format!("{{{{{}}}}}", var.name), value);
        }

        Ok(result)
    }
}
