//! Changelog management commands
//!
//! Provides tools for generating, validating, and managing changelogs
//! following the Keep a Changelog format and Semantic Versioning.

use crate::cli::args::ChangelogCommands;
use anyhow::{Context, Result};
use chrono::Local;
use git2::Repository;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Execute a changelog command
pub async fn run_changelog_command(command: ChangelogCommands) -> Result<()> {
    match command {
        ChangelogCommands::Generate {
            output,
            from,
            to,
            filter,
            dry_run,
        } => generate_changelog(&output, from, to, filter, dry_run).await,
        ChangelogCommands::Validate { file, strict, json } => {
            validate_changelog(&file, strict, json).await
        }
        ChangelogCommands::Release {
            version,
            date,
            from_commits,
            dry_run,
        } => prepare_release(&version, date, from_commits, dry_run).await,
        ChangelogCommands::Export {
            input,
            output,
            format,
            version,
        } => export_changelog(&input, &output, &format, version).await,
        ChangelogCommands::Add {
            entry_type,
            description,
            unreleased,
            version,
        } => add_changelog_entry(&entry_type, &description, unreleased, version).await,
        ChangelogCommands::Stats { file, json } => show_changelog_stats(&file, json).await,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogEntry {
    pub category: String,
    pub description: String,
    pub commit: Option<String>,
    pub author: Option<String>,
    pub pr_number: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogRelease {
    pub version: String,
    pub date: String,
    pub entries: HashMap<String, Vec<ChangelogEntry>>,
    pub breaking_changes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Changelog {
    pub title: String,
    pub description: String,
    pub unreleased: HashMap<String, Vec<ChangelogEntry>>,
    pub releases: Vec<ChangelogRelease>,
}

/// Generate changelog from git commits
async fn generate_changelog(
    output: &Path,
    from: Option<String>,
    to: Option<String>,
    filter: Option<String>,
    dry_run: bool,
) -> Result<()> {
    let repo = Repository::open(".")
        .context("Failed to open git repository. Is this a git repository?")?;

    let commits = parse_commits(&repo, from.as_deref(), to.as_deref(), filter.as_deref())?;
    let changelog = build_changelog_from_commits(commits)?;
    let markdown = format_changelog_markdown(&changelog)?;

    if dry_run {
        println!("{}", markdown);
    } else {
        fs::write(output, markdown).context("Failed to write changelog file")?;
        println!("✓ Changelog generated: {}", output.display());
    }

    Ok(())
}

/// Parse git commits into changelog entries
fn parse_commits(
    repo: &Repository,
    from: Option<&str>,
    to: Option<&str>,
    filter: Option<&str>,
) -> Result<Vec<ChangelogEntry>> {
    let mut revwalk = repo.revwalk()?;

    // Set up revision range
    if let Some(to_ref) = to {
        let to_oid = repo.revparse_single(to_ref)?.id();
        revwalk.push(to_oid)?;
    } else {
        revwalk.push_head()?;
    }

    if let Some(from_ref) = from {
        let from_oid = repo.revparse_single(from_ref)?.id();
        revwalk.hide(from_oid)?;
    }

    let filter_regex = filter.map(|f| Regex::new(f)).transpose()?;

    let mut entries = Vec::new();
    for oid in revwalk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let message = commit.message().unwrap_or("").trim();

        if let Some(ref regex) = filter_regex {
            if !regex.is_match(message) {
                continue;
            }
        }

        if let Some(entry) = parse_conventional_commit(message, &commit)? {
            entries.push(entry);
        }
    }

    Ok(entries)
}

/// Parse a conventional commit message into a changelog entry
fn parse_conventional_commit(
    message: &str,
    commit: &git2::Commit,
) -> Result<Option<ChangelogEntry>> {
    // Conventional commit format: type(scope): description
    let re = Regex::new(r"^(feat|fix|docs|style|refactor|perf|test|chore|build|ci)(?:\(([^)]+)\))?: (.+)$")?;

    if let Some(captures) = re.captures(message.lines().next().unwrap_or("")) {
        let commit_type = captures.get(1).unwrap().as_str();
        let description = captures.get(3).unwrap().as_str();

        let category = match commit_type {
            "feat" => "Added",
            "fix" => "Fixed",
            "docs" => "Documentation",
            "refactor" => "Changed",
            "perf" => "Changed",
            "chore" => "Changed",
            _ => return Ok(None), // Skip other types
        };

        let author = commit
            .author()
            .name()
            .map(|s| s.to_string());

        let pr_number = extract_pr_number(message);

        Ok(Some(ChangelogEntry {
            category: category.to_string(),
            description: description.to_string(),
            commit: Some(commit.id().to_string()[..8].to_string()),
            author,
            pr_number,
        }))
    } else {
        Ok(None)
    }
}

/// Extract PR number from commit message
fn extract_pr_number(message: &str) -> Option<u32> {
    let re = Regex::new(r"#(\d+)").ok()?;
    re.captures(message)?
        .get(1)?
        .as_str()
        .parse()
        .ok()
}

/// Build changelog structure from entries
fn build_changelog_from_commits(entries: Vec<ChangelogEntry>) -> Result<Changelog> {
    let mut grouped: HashMap<String, Vec<ChangelogEntry>> = HashMap::new();

    for entry in entries {
        grouped
            .entry(entry.category.clone())
            .or_insert_with(Vec::new)
            .push(entry);
    }

    Ok(Changelog {
        title: "Changelog".to_string(),
        description: "All notable changes to this project will be documented in this file."
            .to_string(),
        unreleased: grouped,
        releases: Vec::new(),
    })
}

/// Format changelog as markdown
fn format_changelog_markdown(changelog: &Changelog) -> Result<String> {
    let mut output = String::new();

    output.push_str("# Changelog\n\n");
    output.push_str(&changelog.description);
    output.push_str("\n\n");
    output.push_str(
        "The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),\n",
    );
    output.push_str(
        "and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).\n\n",
    );

    // Unreleased section
    if !changelog.unreleased.is_empty() {
        output.push_str("## [Unreleased]\n\n");
        format_entries(&mut output, &changelog.unreleased)?;
    }

    // Release sections
    for release in &changelog.releases {
        output.push_str(&format!("## [{}] - {}\n\n", release.version, release.date));

        if !release.breaking_changes.is_empty() {
            output.push_str("### Breaking Changes\n\n");
            for change in &release.breaking_changes {
                output.push_str(&format!("- {}\n", change));
            }
            output.push('\n');
        }

        format_entries(&mut output, &release.entries)?;
    }

    Ok(output)
}

/// Format entries by category
fn format_entries(output: &mut String, entries: &HashMap<String, Vec<ChangelogEntry>>) -> Result<()> {
    let order = ["Added", "Changed", "Deprecated", "Removed", "Fixed", "Security"];

    for category in &order {
        if let Some(items) = entries.get(*category) {
            output.push_str(&format!("### {}\n", category));
            for item in items {
                output.push_str("- ");
                output.push_str(&item.description);

                // Add commit hash
                if let Some(ref commit) = item.commit {
                    output.push_str(&format!(" ({})", commit));
                }

                // Add PR link
                if let Some(pr) = item.pr_number {
                    output.push_str(&format!(" [#{}]", pr));
                }

                // Add author
                if let Some(ref author) = item.author {
                    output.push_str(&format!(" @{}", author));
                }

                output.push('\n');
            }
            output.push('\n');
        }
    }

    Ok(())
}

/// Validate changelog format
async fn validate_changelog(file: &Path, strict: bool, json: bool) -> Result<()> {
    let content = fs::read_to_string(file)
        .with_context(|| format!("Failed to read changelog file: {}", file.display()))?;

    let mut issues = Vec::new();

    // Check for title
    if !content.starts_with("# Changelog") && !content.starts_with("# CHANGELOG") {
        issues.push("Missing '# Changelog' title");
    }

    // Check for Keep a Changelog reference
    if !content.contains("keepachangelog.com") {
        issues.push("Missing Keep a Changelog reference");
    }

    // Check for Semantic Versioning reference
    if !content.contains("semver.org") {
        issues.push("Missing Semantic Versioning reference");
    }

    // Check for Unreleased section
    if !content.contains("## [Unreleased]") && strict {
        issues.push("Missing [Unreleased] section");
    }

    // Check for valid version format
    let version_re = Regex::new(r"## \[(\d+\.\d+\.\d+)\]")?;
    let versions: Vec<_> = version_re
        .captures_iter(&content)
        .filter_map(|c| c.get(1).map(|m| m.as_str()))
        .collect();

    if versions.is_empty() && strict {
        issues.push("No versioned releases found");
    }

    // Check for valid categories
    let valid_categories = [
        "Added",
        "Changed",
        "Deprecated",
        "Removed",
        "Fixed",
        "Security",
    ];
    let category_re = Regex::new(r"### (.+)")?;
    for cap in category_re.captures_iter(&content) {
        let category = cap.get(1).unwrap().as_str().trim();
        if !valid_categories.contains(&category) && category != "Breaking Changes" {
            issues.push("Invalid category found");
        }
    }

    if json {
        let result = serde_json::json!({
            "valid": issues.is_empty(),
            "issues": issues,
            "versions": versions,
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        if issues.is_empty() {
            println!("✓ Changelog is valid");
        } else {
            println!("✗ Changelog validation failed:");
            for issue in &issues {
                println!("  - {}", issue);
            }
            if strict {
                anyhow::bail!("Changelog validation failed");
            }
        }
    }

    Ok(())
}

/// Prepare a new release section
async fn prepare_release(
    version: &str,
    date: Option<String>,
    _from_commits: bool,
    dry_run: bool,
) -> Result<()> {
    let date = date.unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string());

    let mut content = fs::read_to_string("CHANGELOG.md")
        .context("Failed to read CHANGELOG.md")?;

    // Find the Unreleased section
    let unreleased_re = Regex::new(r"## \[Unreleased\]\n\n((?:###.+?\n(?:- .+?\n)+\n?)+)")?;

    if let Some(captures) = unreleased_re.captures(&content) {
        let unreleased_content = captures.get(1).unwrap().as_str();

        // Create new release section
        let release_section = format!("## [{}] - {}\n\n{}", version, date, unreleased_content);

        // Replace Unreleased with new release and empty Unreleased
        content = unreleased_re.replace(
            &content,
            format!("## [Unreleased]\n\n{}", release_section).as_str(),
        ).to_string();

        if dry_run {
            println!("{}", content);
        } else {
            fs::write("CHANGELOG.md", content)?;
            println!("✓ Release {} prepared in CHANGELOG.md", version);
        }
    } else {
        anyhow::bail!("Could not find Unreleased section in CHANGELOG.md");
    }

    Ok(())
}

/// Export changelog to different formats
async fn export_changelog(
    input: &Path,
    output: &Path,
    format: &str,
    version: Option<String>,
) -> Result<()> {
    let content = fs::read_to_string(input)?;
    let changelog = parse_changelog_markdown(&content)?;

    match format {
        "json" => {
            let json = if let Some(ver) = version {
                let release = changelog
                    .releases
                    .iter()
                    .find(|r| r.version == ver)
                    .context("Version not found")?;
                serde_json::to_string_pretty(release)?
            } else {
                serde_json::to_string_pretty(&changelog)?
            };
            fs::write(output, json)?;
        }
        "html" => {
            let html = format_changelog_html(&changelog, version.as_deref())?;
            fs::write(output, html)?;
        }
        _ => anyhow::bail!("Unsupported format: {}", format),
    }

    println!("✓ Exported to {}", output.display());
    Ok(())
}

/// Parse markdown changelog
fn parse_changelog_markdown(content: &str) -> Result<Changelog> {
    let mut changelog = Changelog {
        title: "Changelog".to_string(),
        description: String::new(),
        unreleased: HashMap::new(),
        releases: Vec::new(),
    };

    let lines: Vec<&str> = content.lines().collect();
    let mut current_version: Option<String> = None;
    let mut current_date: Option<String> = None;
    let mut current_category: Option<String> = None;
    let mut current_entries: HashMap<String, Vec<ChangelogEntry>> = HashMap::new();
    let mut breaking_changes: Vec<String> = Vec::new();

    for line in lines {
        // Version header: ## [1.0.0] - 2024-01-01
        if line.starts_with("## [") {
            // Save previous section if any
            if let Some(ver) = current_version.take() {
                changelog.releases.push(ChangelogRelease {
                    version: ver,
                    date: current_date.take().unwrap_or_default(),
                    entries: current_entries.clone(),
                    breaking_changes: breaking_changes.clone(),
                });
                current_entries.clear();
                breaking_changes.clear();
            }

            // Parse new version
            let version_re = Regex::new(r"## \[([^\]]+)\](?: - (.+))?")?;
            if let Some(cap) = version_re.captures(line) {
                let version = cap.get(1).unwrap().as_str();
                if version == "Unreleased" {
                    current_version = None;
                } else {
                    current_version = Some(version.to_string());
                    current_date = cap.get(2).map(|m| m.as_str().to_string());
                }
            }
        }
        // Category header: ### Added
        else if line.starts_with("### ") {
            current_category = Some(line[4..].trim().to_string());
        }
        // Entry: - Description
        else if line.starts_with("- ") {
            if let Some(ref category) = current_category {
                let description = line[2..].trim().to_string();
                let entry = ChangelogEntry {
                    category: category.clone(),
                    description,
                    commit: None,
                    author: None,
                    pr_number: None,
                };

                if category == "Breaking Changes" {
                    breaking_changes.push(entry.description.clone());
                } else {
                    current_entries
                        .entry(category.clone())
                        .or_insert_with(Vec::new)
                        .push(entry);
                }
            }
        }
    }

    // Save last section
    if let Some(ver) = current_version {
        changelog.releases.push(ChangelogRelease {
            version: ver,
            date: current_date.unwrap_or_default(),
            entries: current_entries,
            breaking_changes,
        });
    } else if !current_entries.is_empty() {
        changelog.unreleased = current_entries;
    }

    Ok(changelog)
}

/// Format changelog as HTML
fn format_changelog_html(changelog: &Changelog, version: Option<&str>) -> Result<String> {
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
    html.push_str("<title>Changelog</title>\n");
    html.push_str("<style>\n");
    html.push_str("body { font-family: sans-serif; max-width: 800px; margin: 40px auto; }\n");
    html.push_str("h1 { border-bottom: 2px solid #333; }\n");
    html.push_str("h2 { margin-top: 30px; color: #2c3e50; }\n");
    html.push_str("h3 { color: #34495e; }\n");
    html.push_str("</style>\n</head>\n<body>\n");

    html.push_str(&format!("<h1>{}</h1>\n", changelog.title));
    html.push_str(&format!("<p>{}</p>\n", changelog.description));

    if version.is_none() && !changelog.unreleased.is_empty() {
        html.push_str("<h2>Unreleased</h2>\n");
        format_entries_html(&mut html, &changelog.unreleased)?;
    }

    for release in &changelog.releases {
        if version.map_or(true, |v| v == release.version) {
            html.push_str(&format!("<h2>{} - {}</h2>\n", release.version, release.date));

            if !release.breaking_changes.is_empty() {
                html.push_str("<h3>Breaking Changes</h3>\n<ul>\n");
                for change in &release.breaking_changes {
                    html.push_str(&format!("<li>{}</li>\n", change));
                }
                html.push_str("</ul>\n");
            }

            format_entries_html(&mut html, &release.entries)?;
        }
    }

    html.push_str("</body>\n</html>");
    Ok(html)
}

/// Format entries as HTML
fn format_entries_html(
    html: &mut String,
    entries: &HashMap<String, Vec<ChangelogEntry>>,
) -> Result<()> {
    let order = ["Added", "Changed", "Deprecated", "Removed", "Fixed", "Security"];

    for category in &order {
        if let Some(items) = entries.get(*category) {
            html.push_str(&format!("<h3>{}</h3>\n<ul>\n", category));
            for item in items {
                html.push_str(&format!("<li>{}</li>\n", item.description));
            }
            html.push_str("</ul>\n");
        }
    }

    Ok(())
}

/// Add a changelog entry
async fn add_changelog_entry(
    entry_type: &str,
    description: &str,
    unreleased: bool,
    version: Option<String>,
) -> Result<()> {
    let category = match entry_type.to_lowercase().as_str() {
        "added" | "add" => "Added",
        "changed" | "change" => "Changed",
        "deprecated" | "deprecate" => "Deprecated",
        "removed" | "remove" => "Removed",
        "fixed" | "fix" => "Fixed",
        "security" => "Security",
        _ => anyhow::bail!("Invalid entry type: {}", entry_type),
    };

    let mut content = fs::read_to_string("CHANGELOG.md")?;

    // Find target section
    let section = if unreleased {
        "Unreleased"
    } else {
        version
            .as_deref()
            .context("Version required when not adding to unreleased")?
    };

    // Find or create category
    let section_re = Regex::new(&format!(r"## \[{}\](?:\n|\s)", regex::escape(section)))?;
    let category_re = Regex::new(&format!(r"(### {}(?:\n|\r\n))", category))?;

    if let Some(_section_match) = section_re.find(&content) {
        if let Some(cat_match) = category_re.find(&content) {
            // Category exists, insert after it
            let insert_pos = cat_match.end();
            content.insert_str(insert_pos, &format!("- {}\n", description));
        } else {
            // Category doesn't exist, create it
            anyhow::bail!(
                "Category {} not found in section {}. Please add it manually.",
                category,
                section
            );
        }
    } else {
        anyhow::bail!("Section {} not found in CHANGELOG.md", section);
    }

    fs::write("CHANGELOG.md", content)?;
    println!("✓ Added entry to {} section", section);

    Ok(())
}

/// Show changelog statistics
async fn show_changelog_stats(file: &Path, json: bool) -> Result<()> {
    let content = fs::read_to_string(file)?;
    let changelog = parse_changelog_markdown(&content)?;

    let total_releases = changelog.releases.len();
    let total_entries: usize = changelog
        .releases
        .iter()
        .map(|r| r.entries.values().map(|v| v.len()).sum::<usize>())
        .sum();

    let mut category_counts: HashMap<String, usize> = HashMap::new();
    for release in &changelog.releases {
        for (category, entries) in &release.entries {
            *category_counts.entry(category.clone()).or_insert(0) += entries.len();
        }
    }

    if json {
        let stats = serde_json::json!({
            "total_releases": total_releases,
            "total_entries": total_entries,
            "category_counts": category_counts,
        });
        println!("{}", serde_json::to_string_pretty(&stats)?);
    } else {
        println!("Changelog Statistics:");
        println!("  Total releases: {}", total_releases);
        println!("  Total entries: {}", total_entries);
        println!("\nEntries by category:");
        for (category, count) in category_counts {
            println!("  {}: {}", category, count);
        }
    }

    Ok(())
}
