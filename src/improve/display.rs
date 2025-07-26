use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

use super::session::{ReviewSummary, SessionResult};

pub struct ProgressSpinner {
    bar: ProgressBar,
}

impl ProgressSpinner {
    pub fn new(message: &str) -> Self {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ "),
        );
        bar.set_message(message.to_string());
        bar.enable_steady_tick(Duration::from_millis(100));

        Self { bar }
    }

    pub fn update(&self, message: &str) {
        self.bar.set_message(message.to_string());
    }

    pub fn success(self, message: &str) {
        self.bar.finish_with_message(format!("✓ {}", message));
    }

    pub fn error(self, message: &str) {
        self.bar.finish_with_message(format!("✗ {}", message));
    }
}

pub fn show_progress(action: &str, details: &str) {
    println!("  {} {}", action, details);
}

pub fn show_welcome() {
    println!("\n🚀 MMM starting code improvement...");
}

pub fn show_analysis_results(summary: &str, focus_areas: &[String]) {
    println!("\n📊 Analyzing project...");
    println!("  ✓ {}", summary);
    if !focus_areas.is_empty() {
        println!("  ✓ Focus areas: {}", focus_areas.join(", "));
    }
}

pub fn show_review_results(summary: &ReviewSummary) {
    println!("\n🔍 Reviewing code quality...");
    println!("  Current score: {:.1}/10", summary.current_score);
    if summary.issues_found > 0 {
        println!(
            "  Issues found: {} ({} high, {} medium, {} low)",
            summary.issues_found,
            summary.high_severity,
            summary.medium_severity,
            summary.low_severity
        );
    } else {
        println!("  No significant issues found!");
    }
}

pub fn show_improvement_progress(action: &str) {
    println!("\n🔧 Applying improvements...");
    println!("  ✓ {}", action);
}

pub fn show_results(result: &SessionResult) {
    println!("\n✅ Improvement complete!");
    println!(
        "  Score: {:.1} → {:.1} (+{:.1})",
        result.initial_score, result.final_score, result.improvement
    );
    println!("  Files changed: {}", result.files_changed);
    println!("  Iterations: {}", result.iterations);

    if result.improvement > 0.0 {
        println!("\n💡 Run 'mmm improve' again for further improvements");
    } else {
        println!("\n🎉 Your code is looking great!");
    }
}

pub fn show_dry_run_notice() {
    println!("\n🔍 DRY RUN MODE - No changes will be made");
}

pub fn show_error(error: &str) {
    eprintln!("\n❌ Error: {}", error);
}

pub fn format_file_list(files: &[String], max_items: usize) -> String {
    if files.len() <= max_items {
        files.join(", ")
    } else {
        format!(
            "{}, and {} more",
            files[..max_items].join(", "),
            files.len() - max_items
        )
    }
}

pub fn format_percentage(value: f32) -> String {
    format!("{:.0}%", value * 100.0)
}

pub fn format_score(score: f32) -> String {
    format!("{:.1}/10", score)
}
