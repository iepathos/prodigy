//! Progress display with real-time updates and beautiful animations

use colored::*;
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
use std::time::{Duration, Instant};

/// Phases of the improvement process
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    Analyzing,
    Reviewing,
    Improving,
    Validating,
    Complete,
}

impl Phase {
    /// Get the emoji icon for this phase
    pub fn icon(&self) -> &'static str {
        match self {
            Phase::Analyzing => "📊",
            Phase::Reviewing => "🔍",
            Phase::Improving => "🔧",
            Phase::Validating => "✅",
            Phase::Complete => "✨",
        }
    }
    
    /// Get the display name for this phase
    pub fn name(&self) -> &'static str {
        match self {
            Phase::Analyzing => "Analyzing project",
            Phase::Reviewing => "Reviewing code quality",
            Phase::Improving => "Applying improvements",
            Phase::Validating => "Validating changes",
            Phase::Complete => "Complete",
        }
    }
}

/// Progress display manager
pub struct ProgressDisplay {
    multi: MultiProgress,
    main_bar: ProgressBar,
    start_time: Instant,
    current_phase: Phase,
}

impl ProgressDisplay {
    /// Create a new progress display
    pub fn new() -> Self {
        let multi = MultiProgress::new();
        let main_bar = multi.add(ProgressBar::new(100));
        
        // Set up the main progress bar style
        main_bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} {msg} {wide_bar:.cyan/blue} {percent}% ({eta_precise})")
                .unwrap()
                .progress_chars("━━─")
        );
        
        main_bar.set_message("Starting...");
        
        Self {
            multi,
            main_bar,
            start_time: Instant::now(),
            current_phase: Phase::Analyzing,
        }
    }
    
    /// Start a new phase
    pub fn start_phase(&mut self, phase: Phase) {
        self.current_phase = phase;
        self.main_bar.set_position(0);
        self.main_bar.set_message(format!("{} {}...", phase.icon(), phase.name()));
        
        if phase == Phase::Analyzing {
            println!("{} {} starting code improvement...", "🚀".bold(), "MMM".cyan().bold());
            println!();
        }
    }
    
    /// Update progress for the current phase
    pub fn update(&self, progress: u64, message: &str) {
        self.main_bar.set_position(progress);
        if !message.is_empty() {
            self.main_bar.set_message(
                format!("{} {} - {}", 
                    self.current_phase.icon(), 
                    self.current_phase.name(),
                    message
                )
            );
        }
    }
    
    /// Add a sub-item completion message
    pub fn complete_item(&self, message: &str) {
        self.multi.println(format!("  {} {}", "✓".green(), message)).ok();
    }
    
    /// Complete the current phase
    pub fn complete_phase(&mut self, summary: &str) {
        self.main_bar.set_position(100);
        let duration = format_duration(self.start_time.elapsed());
        
        self.main_bar.finish_with_message(
            format!("{} {} ({})", 
                self.current_phase.icon(),
                summary,
                duration.dimmed()
            )
        );
        
        // Add spacing between phases
        if self.current_phase != Phase::Complete {
            println!();
        }
    }
    
    /// Show an error message
    pub fn error(&self, message: &str) {
        self.multi.println(format!("{} {}", "❌".red(), message)).ok();
    }
    
    /// Show a warning message
    pub fn warning(&self, message: &str) {
        self.multi.println(format!("{} {}", "⚠️".yellow(), message)).ok();
    }
    
    /// Get the multi progress handle for advanced usage
    pub fn multi(&self) -> &MultiProgress {
        &self.multi
    }
}

/// Format a duration in a human-readable way
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs_f64();
    if secs < 1.0 {
        format!("{:.0}ms", duration.as_millis())
    } else if secs < 60.0 {
        format!("{:.1}s", secs)
    } else {
        let mins = secs / 60.0;
        format!("{:.1}m", mins)
    }
}

/// Create a simple spinner for quick operations
pub fn spinner(message: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
    );
    spinner.set_message(message.to_string());
    spinner.enable_steady_tick(Duration::from_millis(100));
    spinner
}

/// Show a progress bar for countable operations
pub fn progress_bar(total: u64, message: &str) -> ProgressBar {
    let bar = ProgressBar::new(total);
    bar.set_style(
        ProgressStyle::default_bar()
            .template("{msg} {bar:40.cyan/blue} {pos}/{len}")
            .unwrap()
            .progress_chars("█▓▒░")
    );
    bar.set_message(message.to_string());
    bar
}