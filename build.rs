use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=.claude/commands/");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("command_includes.rs");

    // Get the manifest directory (where Cargo.toml is)
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_path = Path::new(&manifest_dir);
    let commands_dir = manifest_path.join(".claude").join("commands");

    let mut includes = String::new();
    let mut templates = String::new();

    // Scan for all prodigy-*.md files
    if commands_dir.exists() {
        let mut command_files: Vec<_> = fs::read_dir(&commands_dir)
            .unwrap()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                let file_name = path.file_name()?.to_str()?;

                // Only include prodigy-*.md files
                if file_name.starts_with("prodigy-") && file_name.ends_with(".md") {
                    // Use absolute path for include_str!
                    let absolute_path = path.canonicalize().ok()?;

                    Some((
                        file_name.trim_end_matches(".md").to_string(),
                        absolute_path.to_str()?.replace('\\', "/"),
                    ))
                } else {
                    None
                }
            })
            .collect();

        // Sort for consistent ordering
        command_files.sort_by(|a, b| a.0.cmp(&b.0));

        // Generate const declarations for each command
        for (name, path) in &command_files {
            let const_name = name.to_uppercase().replace('-', "_");
            includes.push_str(&format!(
                "pub const {}: &str = include_str!(\"{}\");\n",
                const_name, path
            ));
        }

        // Generate the get_all_command_templates function
        templates.push_str("pub fn get_all_command_templates() -> Vec<CommandTemplate> {\n");
        templates.push_str("    vec![\n");

        for (name, _) in &command_files {
            let const_name = name.to_uppercase().replace('-', "_");
            let description = extract_description(name);

            templates.push_str(&format!(
                "        CommandTemplate {{\n            name: \"{}\",\n            description: \"{}\",\n            content: {},\n        }},\n",
                name, description, const_name
            ));
        }

        templates.push_str("    ]\n");
        templates.push_str("}\n");
    } else {
        // If no commands directory, return empty
        templates.push_str("pub fn get_all_command_templates() -> Vec<CommandTemplate> {\n");
        templates.push_str("    vec![]\n");
        templates.push_str("}\n");
    }

    // Write the generated code
    let generated_code = format!("{}\n{}", includes, templates);
    fs::write(&dest_path, generated_code).unwrap();
}

fn extract_description(name: &str) -> &'static str {
    // Extract a description from the command name
    match name {
        "prodigy-code-review" => "Analyzes code quality and creates improvement specs",
        "prodigy-implement-spec" => "Implements Git Good specifications",
        "prodigy-lint" => "Runs formatters, linters, and tests",
        "prodigy-product-enhance" => "Product-focused improvements for user value",
        "prodigy-merge-worktree" => "Claude-assisted worktree merging with conflict resolution",
        "prodigy-cleanup-tech-debt" => {
            "Analyzes technical debt and generates cleanup specifications"
        }
        "prodigy-coverage" => "Analyzes and improves test coverage",
        "prodigy-commit-changes" => "Commits changes with detailed commit messages",
        "prodigy-debug-test-failure" => "Debugs and fixes test failures",
        "prodigy-docs-generate" => "Generates documentation for the codebase",
        "prodigy-performance" => "Analyzes and optimizes performance",
        "prodigy-security-audit" => "Performs security analysis and auditing",
        "prodigy-test-generate" => "Generates test cases for code",
        "prodigy-add-spec" => "Adds new specifications to the project",
        "prodigy-compare-debt-results" => "Compares technical debt analysis results",
        _ => "Claude command for Prodigy automation",
    }
}
