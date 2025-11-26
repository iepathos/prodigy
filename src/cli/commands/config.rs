//! Configuration tracing CLI commands.
//!
//! This module implements the `prodigy config` subcommands for tracing
//! configuration value origins and diagnosing configuration issues.

use anyhow::Result;

use crate::cli::args::ConfigCommands;
use crate::config::diagnostics::{detect_issues, format_issues, format_issues_json};
use crate::config::tracing::{trace_config, TracedProdigyConfig};

/// Execute a config command.
pub async fn run_config_command(command: ConfigCommands) -> Result<()> {
    match command {
        ConfigCommands::Trace {
            path,
            all,
            overrides,
            diagnose,
            json,
        } => handle_trace(path, all, overrides, diagnose, json).await,
        ConfigCommands::Show { path, json } => handle_show(path, json).await,
    }
}

/// Handle the `prodigy config trace` command.
async fn handle_trace(
    path: Option<String>,
    all: bool,
    overrides: bool,
    diagnose: bool,
    json: bool,
) -> Result<()> {
    let traced = match trace_config() {
        Ok(t) => t,
        Err(errors) => {
            eprintln!("Failed to load configuration:");
            for error in errors.iter() {
                eprintln!("  - {}", error);
            }
            std::process::exit(1);
        }
    };

    // Handle diagnose mode
    if diagnose {
        let issues = detect_issues(&traced);
        if json {
            println!("{}", format_issues_json(&issues));
        } else {
            println!("{}", format_issues(&issues));
        }
        return Ok(());
    }

    // Handle specific path
    if let Some(path) = path {
        print_single_trace(&traced, &path, json);
        return Ok(());
    }

    // Handle overrides mode
    if overrides {
        print_overrides(&traced, json);
        return Ok(());
    }

    // Handle all mode (or default if nothing specified)
    if all || (!overrides && !diagnose) {
        print_all_traces(&traced, json);
    }

    Ok(())
}

/// Handle the `prodigy config show` command.
async fn handle_show(path: Option<String>, json: bool) -> Result<()> {
    let traced = match trace_config() {
        Ok(t) => t,
        Err(errors) => {
            eprintln!("Failed to load configuration:");
            for error in errors.iter() {
                eprintln!("  - {}", error);
            }
            std::process::exit(1);
        }
    };

    if json {
        // Output all config as JSON
        let config = traced.config();
        let json_str = serde_json::to_string_pretty(config)?;
        println!("{}", json_str);
    } else if let Some(path) = path {
        // Show specific path value
        if let Some(trace) = traced.trace(&path) {
            println!("{}: {}", path, format_json_value(&trace.final_value));
        } else {
            eprintln!("No value found at path: {}", path);
            std::process::exit(1);
        }
    } else {
        // Show all effective values
        let config = traced.config();
        println!("Effective configuration:");
        println!();
        println!("  log_level: {}", config.log_level);
        println!("  max_concurrent_specs: {}", config.max_concurrent_specs);
        println!("  auto_commit: {}", config.auto_commit);

        if let Some(ref api_key) = config.claude_api_key {
            println!("  claude_api_key: {}...", &api_key[..api_key.len().min(8)]);
        } else {
            println!("  claude_api_key: (not set)");
        }

        if let Some(ref editor) = config.default_editor {
            println!("  default_editor: {}", editor);
        }

        if let Some(ref home) = config.prodigy_home {
            println!("  prodigy_home: {}", home.display());
        }

        println!();
        println!("Storage:");
        println!("  backend: {:?}", config.storage.backend);
        if let Some(ref path) = config.storage.base_path {
            println!("  base_path: {}", path.display());
        }
        println!("  compression_level: {}", config.storage.compression_level);

        if let Some(ref project) = config.project {
            println!();
            println!("Project:");
            if let Some(ref name) = project.name {
                println!("  name: {}", name);
            }
            if let Some(ref desc) = project.description {
                println!("  description: {}", desc);
            }
            if let Some(ref spec_dir) = project.spec_dir {
                println!("  spec_dir: {}", spec_dir.display());
            }
        }

        println!();
        println!("Plugins:");
        println!("  enabled: {}", config.plugins.enabled);
        if let Some(ref dir) = config.plugins.directory {
            println!("  directory: {}", dir.display());
        }
        if !config.plugins.auto_load.is_empty() {
            println!("  auto_load: {}", config.plugins.auto_load.join(", "));
        }
    }

    Ok(())
}

/// Print trace for a single path.
fn print_single_trace(traced: &TracedProdigyConfig, path: &str, json: bool) {
    match traced.trace(path) {
        Some(trace) => {
            if json {
                if let Some(json_output) = traced.to_json(path) {
                    let json_str = serde_json::to_string_pretty(&json_output).unwrap_or_default();
                    println!("{}", json_str);
                }
            } else {
                println!("{}", trace.explain(path));
            }
        }
        None => {
            eprintln!("No value found at path: {}", path);
            std::process::exit(1);
        }
    }
}

/// Print only overridden values.
fn print_overrides(traced: &TracedProdigyConfig, json: bool) {
    let overridden = traced.overridden_paths();

    if overridden.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No configuration values were overridden.");
        }
        return;
    }

    if json {
        let outputs = traced.overrides_to_json();
        let json_str = serde_json::to_string_pretty(&outputs).unwrap_or_default();
        println!("{}", json_str);
    } else {
        println!("Overridden configuration values:\n");
        for path in overridden {
            if let Some(trace) = traced.trace(&path) {
                println!("{}", trace.explain(&path));
                println!();
            }
        }
    }
}

/// Print all traced values.
fn print_all_traces(traced: &TracedProdigyConfig, json: bool) {
    if json {
        let outputs = traced.all_to_json();
        let json_str = serde_json::to_string_pretty(&outputs).unwrap_or_default();
        println!("{}", json_str);
    } else {
        println!("Configuration values:\n");
        for (path, trace) in traced.all_traces() {
            println!("{}", trace.explain(&path));
            println!();
        }
    }
}

/// Format a JSON value for display.
fn format_json_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => format!("\"{}\"", s),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string(value).unwrap_or_else(|_| format!("{:?}", value))
        }
    }
}
