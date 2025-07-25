use crate::Result;
use std::path::Path;
use tokio::fs;

pub struct ProjectTemplate {
    pub name: String,
    pub description: String,
    pub files: Vec<TemplateFile>,
}

pub struct TemplateFile {
    pub path: String,
    pub content: String,
}

impl ProjectTemplate {
    pub fn basic() -> Self {
        Self {
            name: "basic".to_string(),
            description: "Basic MMM project template".to_string(),
            files: vec![
                TemplateFile {
                    path: "specs/README.md".to_string(),
                    content: r#"# Project Specifications

This directory contains all project specifications that will be implemented by MMM.

## Specification Format

Each specification should be a markdown file with:
- Clear objectives
- Acceptance criteria
- Technical details
- Implementation notes

## Naming Convention

Use descriptive names with number prefixes for ordering:
- `01-initial-setup.md`
- `02-core-features.md`
- `03-advanced-features.md`
"#.to_string(),
                },
                TemplateFile {
                    path: ".gitignore".to_string(),
                    content: r#"# MMM
.mmm/state.db
.mmm/logs/
.mmm/cache/

# IDE
.idea/
.vscode/
*.swp
*.swo

# OS
.DS_Store
Thumbs.db
"#.to_string(),
                },
            ],
        }
    }
    
    pub async fn apply(&self, project_path: &Path) -> Result<()> {
        for file in &self.files {
            let file_path = project_path.join(&file.path);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            fs::write(&file_path, &file.content).await?;
        }
        Ok(())
    }
}