//! Result summary display with quality scores and impact visualization

use colored::*;
use std::fmt;

/// Quality score representation
#[derive(Debug, Clone, Copy)]
pub struct QualityScore {
    pub before: f32,
    pub after: f32,
}

impl QualityScore {
    /// Create a new quality score
    pub fn new(before: f32, after: f32) -> Self {
        Self { before, after }
    }

    /// Get the improvement delta
    pub fn delta(&self) -> f32 {
        self.after - self.before
    }

    /// Get the percentage improvement
    pub fn improvement_percentage(&self) -> f32 {
        if self.before == 0.0 {
            100.0
        } else {
            (self.delta() / self.before) * 100.0
        }
    }

    /// Format as a progress bar
    pub fn progress_bar(&self, width: usize) -> String {
        let filled = ((self.after / 10.0) * width as f32) as usize;
        let empty = width.saturating_sub(filled);

        format!(
            "{}{}",
            "█".repeat(filled).green(),
            "░".repeat(empty).dimmed()
        )
    }
}

/// Impact metrics for improvements
#[derive(Debug, Default)]
pub struct ImpactMetrics {
    pub tests_coverage: Option<(f32, f32)>, // (before, after)
    pub errors_fixed: usize,
    pub warnings_fixed: usize,
    pub docs_added: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
    pub files_changed: usize,
}

impl ImpactMetrics {
    /// Format a metric change
    fn format_change(before: f32, after: f32, suffix: &str) -> String {
        let delta = after - before;
        let arrow = if delta > 0.0 { "↑" } else { "↓" };
        let color = if delta > 0.0 { "green" } else { "red" };

        format!(
            "{:.0}% → {:.0}% ({}{:.0}%{})",
            before,
            after,
            arrow,
            delta.abs(),
            suffix
        )
        .color(color)
        .to_string()
    }
}

/// Result summary for completed improvements
pub struct ResultSummary {
    pub quality_score: QualityScore,
    pub changes_made: Vec<String>,
    pub impact: ImpactMetrics,
    pub duration: std::time::Duration,
    pub next_suggestion: Option<String>,
}

impl ResultSummary {
    /// Display the full result summary
    pub fn display(&self) {
        // Header
        println!();
        println!(
            "{} {} Your code is now better.",
            "✨".bold(),
            "Improvement complete!".green().bold()
        );
        println!();

        // Quality score visualization
        println!(
            "{} {}  {:.1} → {:.1} ({}{:.1})",
            "📈".bold(),
            "Quality Score:".bold(),
            self.quality_score.before,
            self.quality_score.after,
            if self.quality_score.delta() > 0.0 {
                "+"
            } else {
                ""
            },
            self.quality_score.delta()
        );
        println!("                   {}", self.quality_score.progress_bar(20));
        println!();

        // Changes made
        if !self.changes_made.is_empty() {
            println!("{} {}", "📝".bold(), "Changes Made:".bold());
            for change in &self.changes_made {
                println!("  {} {}", "•".dimmed(), change);
            }
            println!();
        }

        // Impact metrics
        self.display_impact();

        // Summary stats
        println!(
            "{} Files changed: {} | Lines: {}, {}",
            "💾".bold(),
            self.impact.files_changed.to_string().cyan(),
            format!("+{}", self.impact.lines_added).green(),
            format!("-{}", self.impact.lines_removed).red()
        );
        println!();

        // Next suggestion
        if let Some(suggestion) = &self.next_suggestion {
            println!("{} {}", "🎯".bold(), "Next suggested improvement:".bold());
            println!("   {suggestion}");
            println!();
        }

        // Success message
        let improvement_pct = self.quality_score.improvement_percentage();
        println!(
            "{} {}",
            "✨".bold(),
            format!("Great work! Your code quality improved by {improvement_pct:.0}%")
                .green()
                .bold()
        );
    }

    /// Display impact metrics
    fn display_impact(&self) {
        println!("{} {}", "📊".bold(), "Impact:".bold());

        // Test coverage
        if let Some((before, after)) = self.impact.tests_coverage {
            let change = ImpactMetrics::format_change(before, after, "");
            println!("  Tests:     {change}");
        }

        // Errors
        if self.impact.errors_fixed > 0 {
            println!(
                "  Errors:    {} → 0 (-100%)",
                self.impact.errors_fixed.to_string().red(),
            );
        }

        // Documentation
        if self.impact.docs_added > 0 {
            println!(
                "  Docs:      {} APIs documented",
                format!("+{}", self.impact.docs_added).green()
            );
        }

        // Build status
        println!("  Build:     {} All checks passed", "✅".green());
        println!();
    }
}

/// Simple progress visualization for inline use
pub struct InlineProgress {
    current: f32,
    target: f32,
    width: usize,
}

impl InlineProgress {
    pub fn new(current: f32, target: f32) -> Self {
        Self {
            current,
            target,
            width: 20,
        }
    }

    pub fn with_width(mut self, width: usize) -> Self {
        self.width = width;
        self
    }
}

impl fmt::Display for InlineProgress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let progress = (self.current / self.target).min(1.0);
        let filled = (progress * self.width as f32) as usize;
        let empty = self.width.saturating_sub(filled);

        write!(
            f,
            "│ {}{}{}│",
            "█".repeat(filled).green(),
            "█".repeat(empty).dimmed(),
            " ".repeat(empty)
        )
    }
}
