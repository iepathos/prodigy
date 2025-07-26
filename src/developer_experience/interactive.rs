//! Interactive features for live preview and graceful interruption

use colored::*;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Live preview mode for interactive improvements
pub struct LivePreview {
    auto_accept: bool,
    skip_all: bool,
}

impl LivePreview {
    /// Create a new live preview instance
    pub fn new() -> Self {
        Self {
            auto_accept: false,
            skip_all: false,
        }
    }
    
    /// Show a change preview and get user decision
    pub async fn preview_change(
        &mut self,
        file_path: &str,
        line_number: usize,
        old_content: &str,
        new_content: &str,
    ) -> anyhow::Result<ChangeDecision> {
        if self.auto_accept {
            return Ok(ChangeDecision::Accept);
        }
        
        if self.skip_all {
            return Ok(ChangeDecision::Skip);
        }
        
        // Display the change
        println!();
        println!("{} Improving {}...", "🔧".bold(), "error handling".cyan());
        println!();
        println!("{}:{}", file_path.dimmed(), line_number);
        
        // Show diff
        for line in old_content.lines() {
            println!("{} {}", "-".red(), line.red());
        }
        for line in new_content.lines() {
            println!("{} {}", "+".green(), line.green());
        }
        println!();
        
        // Get user input
        print!("Accept change? {} ", "[Y/n/skip all/accept all]:".dimmed());
        io::stdout().flush()?;
        
        let input = self.read_user_input().await?;
        
        match input.trim().to_lowercase().as_str() {
            "" | "y" | "yes" => Ok(ChangeDecision::Accept),
            "n" | "no" => Ok(ChangeDecision::Skip),
            "s" | "skip all" => {
                self.skip_all = true;
                Ok(ChangeDecision::Skip)
            }
            "a" | "accept all" => {
                self.auto_accept = true;
                Ok(ChangeDecision::Accept)
            }
            _ => {
                println!("{} Invalid input, skipping change", "⚠️".yellow());
                Ok(ChangeDecision::Skip)
            }
        }
    }
    
    /// Read user input asynchronously
    async fn read_user_input(&self) -> anyhow::Result<String> {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        Ok(input)
    }
}

/// Decision for a change preview
#[derive(Debug, Clone, Copy)]
pub enum ChangeDecision {
    Accept,
    Skip,
}

/// Interrupt handler for graceful pause/resume
pub struct InterruptHandler {
    interrupted: Arc<AtomicBool>,
    can_resume: bool,
}

impl InterruptHandler {
    /// Create a new interrupt handler
    pub fn new() -> Self {
        let interrupted = Arc::new(AtomicBool::new(false));
        
        // Set up Ctrl+C handler
        let interrupted_clone = interrupted.clone();
        ctrlc::set_handler(move || {
            interrupted_clone.store(true, Ordering::SeqCst);
        }).expect("Error setting Ctrl-C handler");
        
        Self {
            interrupted,
            can_resume: true,
        }
    }
    
    /// Check if interrupted
    pub fn is_interrupted(&self) -> bool {
        self.interrupted.load(Ordering::SeqCst)
    }
    
    /// Wait for interrupt or timeout
    pub async fn wait_for_interrupt(&self, message: &str) {
        println!();
        println!("{} {}", "🔧".bold(), message);
        println!();
        println!("{}", "Press Ctrl+C to pause gracefully...".dimmed());
        
        while !self.is_interrupted() {
            sleep(Duration::from_millis(100)).await;
        }
    }
    
    /// Handle interruption with save state
    pub async fn handle_interrupt(&self, completed: usize, remaining: usize) -> anyhow::Result<()> {
        if !self.is_interrupted() {
            return Ok(());
        }
        
        println!();
        println!("{} {}", "⏸️".bold(), "Pausing after current file...".yellow());
        
        // Save state for resume
        if self.can_resume {
            println!("{} Safe to stop. {} files improved, {} remaining.",
                "✅".green(),
                completed.to_string().green(),
                remaining.to_string().yellow()
            );
            println!();
            println!("Resume with: {}", "mmm improve --resume".cyan());
        }
        
        Ok(())
    }
}

/// Interactive confirmation prompt
pub async fn confirm(message: &str, default: bool) -> anyhow::Result<bool> {
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    
    print!("{} {} {} ", message, hint.dimmed(), ":".dimmed());
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    
    let response = input.trim().to_lowercase();
    Ok(match response.as_str() {
        "" => default,
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default,
    })
}

/// Show a selection menu
pub async fn select_option(prompt: &str, options: &[String]) -> anyhow::Result<usize> {
    println!("{}", prompt.bold());
    
    for (i, option) in options.iter().enumerate() {
        println!("  {}. {}", (i + 1).to_string().cyan(), option);
    }
    
    print!("\n{} ", "Choice:".dimmed());
    io::stdout().flush()?;
    
    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if let Ok(choice) = input.trim().parse::<usize>() {
            if choice > 0 && choice <= options.len() {
                return Ok(choice - 1);
            }
        }
        
        print!("{} Please enter a number between 1 and {}: ",
            "⚠️".yellow(),
            options.len()
        );
        io::stdout().flush()?;
    }
}

/// Progress indicator that can be interrupted
pub struct InterruptibleProgress {
    message: String,
    interrupted: Arc<AtomicBool>,
}

impl InterruptibleProgress {
    pub fn new(message: String) -> Self {
        Self {
            message,
            interrupted: Arc::new(AtomicBool::new(false)),
        }
    }
    
    /// Run with interrupt handling
    pub async fn run<F, T>(&self, task: F) -> anyhow::Result<T>
    where
        F: std::future::Future<Output = anyhow::Result<T>>,
    {
        println!("{} {}", "⏳".bold(), self.message);
        
        // Run task with interrupt checking
        tokio::select! {
            result = task => result,
            _ = self.wait_for_interrupt() => {
                anyhow::bail!("Operation interrupted by user")
            }
        }
    }
    
    async fn wait_for_interrupt(&self) {
        let interrupted = self.interrupted.clone();
        tokio::spawn(async move {
            while !interrupted.load(Ordering::SeqCst) {
                sleep(Duration::from_millis(100)).await;
            }
        }).await.ok();
    }
}