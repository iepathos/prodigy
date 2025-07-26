//! Celebration features for achievements and gamification

use chrono::{DateTime, Datelike, Local};
use colored::*;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Achievement system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Achievement {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub unlocked: bool,
    pub unlocked_at: Option<DateTime<Local>>,
    pub progress: u32,
    pub target: u32,
}

impl Achievement {
    /// Create a new achievement
    pub fn new(id: &str, name: &str, description: &str, icon: &str, target: u32) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            icon: icon.to_string(),
            unlocked: false,
            unlocked_at: None,
            progress: 0,
            target,
        }
    }

    /// Update progress
    pub fn update_progress(&mut self, amount: u32) -> bool {
        if self.unlocked {
            return false;
        }

        self.progress = (self.progress + amount).min(self.target);

        if self.progress >= self.target {
            self.unlocked = true;
            self.unlocked_at = Some(Local::now());
            true
        } else {
            false
        }
    }

    /// Display achievement unlock
    pub fn display_unlock(&self) {
        println!();
        println!(
            "{} {} {}",
            self.icon,
            "Achievement Unlocked:".green().bold(),
            self.name.yellow().bold()
        );
        println!("   {}", self.description.dimmed());
        println!();
    }

    /// Display progress
    pub fn display_progress(&self) {
        if self.unlocked {
            return;
        }

        let percentage = (self.progress as f32 / self.target as f32 * 100.0) as u32;
        let bar_width = 20usize;
        let filled = percentage as usize * bar_width / 100;
        let empty = bar_width.saturating_sub(filled);

        println!("{} Progress towards next achievement:", "🎯".bold());
        println!(
            "   {} - {}/{} {}",
            self.name.cyan(),
            self.progress,
            self.target,
            self.description.dimmed()
        );
        println!(
            "   {}{}",
            "█".repeat(filled).green(),
            "░".repeat(empty).dimmed()
        );
    }
}

/// Achievement manager
pub struct AchievementManager {
    achievements: HashMap<String, Achievement>,
}

impl Default for AchievementManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AchievementManager {
    /// Create a new achievement manager with predefined achievements
    pub fn new() -> Self {
        let mut achievements = HashMap::new();

        // Define achievements
        let achievement_list = vec![
            Achievement::new(
                "error_slayer",
                "Error Slayer",
                "Fixed 100 error handling issues",
                "🏆",
                100,
            ),
            Achievement::new("test_master", "Test Master", "Added 100 tests", "🎯", 100),
            Achievement::new(
                "doc_hero",
                "Documentation Hero",
                "Documented 50 APIs",
                "📚",
                50,
            ),
            Achievement::new(
                "quality_champion",
                "Quality Champion",
                "Reached 9.0/10 quality score",
                "⭐",
                1,
            ),
            Achievement::new(
                "speed_demon",
                "Speed Demon",
                "Completed 10 improvements in under 30 seconds each",
                "⚡",
                10,
            ),
            Achievement::new(
                "consistent_improver",
                "Consistent Improver",
                "7 day improvement streak",
                "🔥",
                7,
            ),
        ];

        for achievement in achievement_list {
            achievements.insert(achievement.id.clone(), achievement);
        }

        Self { achievements }
    }

    /// Update achievement progress
    pub fn update(&mut self, achievement_id: &str, progress: u32) -> Option<&Achievement> {
        if let Some(achievement) = self.achievements.get_mut(achievement_id) {
            if achievement.update_progress(progress) {
                return Some(achievement);
            }
        }
        None
    }

    /// Get next achievement to work towards
    pub fn get_next_achievement(&self) -> Option<&Achievement> {
        self.achievements
            .values()
            .filter(|a| !a.unlocked && a.progress > 0)
            .max_by_key(|a| (a.progress as f32 / a.target as f32 * 100.0) as u32)
    }

    /// Get unlocked achievements count
    pub fn unlocked_count(&self) -> usize {
        self.achievements.values().filter(|a| a.unlocked).count()
    }
}

/// Streak tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Streak {
    pub current: u32,
    pub longest: u32,
    pub last_activity: Option<DateTime<Local>>,
}

impl Default for Streak {
    fn default() -> Self {
        Self::new()
    }
}

impl Streak {
    /// Create a new streak tracker
    pub fn new() -> Self {
        Self {
            current: 0,
            longest: 0,
            last_activity: None,
        }
    }

    /// Update streak
    pub fn update(&mut self) -> bool {
        let now = Local::now();
        let mut streak_continues = false;

        if let Some(last) = self.last_activity {
            let days_diff = (now.ordinal() - last.ordinal()) as i32;

            if days_diff == 1 {
                // Consecutive day
                self.current += 1;
                streak_continues = true;
            } else if days_diff == 0 {
                // Same day, no change
                streak_continues = true;
            } else {
                // Streak broken
                self.current = 1;
            }
        } else {
            // First activity
            self.current = 1;
        }

        self.last_activity = Some(now);
        self.longest = self.longest.max(self.current);

        streak_continues && self.current > 1
    }

    /// Display streak status
    pub fn display(&self) {
        if self.current > 1 {
            println!(
                "{} {} day improvement streak!",
                "🔥".bold(),
                self.current.to_string().yellow().bold()
            );

            if self.current == self.longest {
                println!("   {} This is your longest streak!", "⭐".yellow());
            } else {
                println!(
                    "   Your longest streak: {} days",
                    self.longest.to_string().dimmed()
                );
            }

            println!();
            println!("Keep it up! Run MMM tomorrow to maintain your streak.");
        }
    }
}

/// Success messages
pub struct SuccessMessage;

impl SuccessMessage {
    /// Get a random success message
    pub fn random() -> &'static str {
        const MESSAGES: &[&str] = &[
            "✨ Your code is now better!",
            "🎉 Improvements applied successfully!",
            "💪 Code quality leveled up!",
            "🚀 Your code is ready to ship!",
            "⭐ Excellent improvements made!",
            "🎯 Target quality achieved!",
            "🏆 Your code is now award-worthy!",
            "🌟 Outstanding improvements!",
            "💎 Your code sparkles with quality!",
            "🎊 Fantastic improvements completed!",
            "🥇 First-class code achieved!",
            "🌈 Beautiful improvements applied!",
        ];

        let mut rng = rand::thread_rng();
        MESSAGES.choose(&mut rng).unwrap_or(&MESSAGES[0])
    }

    /// Get a contextual success message
    pub fn contextual(improvement_type: &str, impact: f32) -> String {
        match improvement_type {
            "errors" if impact > 50.0 => {
                "🛡️ Wow! You've eliminated over half of the errors. Rock solid!"
            }
            "tests" if impact > 30.0 => {
                "🧪 Impressive test coverage boost! Your code is well-protected now."
            }
            "docs" if impact > 20.0 => {
                "📖 Documentation looking great! Future you will thank present you."
            }
            "performance" => "⚡ Speed improvements applied! Your code is now blazing fast.",
            _ => Self::random(),
        }
        .to_string()
    }
}

/// Team leaderboard (optional feature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub name: String,
    pub score: f32,
    pub daily_improvement: f32,
    pub rank: u32,
}

pub struct Leaderboard {
    members: Vec<TeamMember>,
    current_user: String,
}

impl Leaderboard {
    /// Create a new leaderboard
    pub fn new(current_user: String) -> Self {
        Self {
            members: Vec::new(),
            current_user,
        }
    }

    /// Update leaderboard
    pub fn update(&mut self, name: String, score: f32, daily_improvement: f32) {
        if let Some(member) = self.members.iter_mut().find(|m| m.name == name) {
            member.score = score;
            member.daily_improvement = daily_improvement;
        } else {
            self.members.push(TeamMember {
                name,
                score,
                daily_improvement,
                rank: 0,
            });
        }

        // Sort by score and assign ranks
        self.members
            .sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        for (i, member) in self.members.iter_mut().enumerate() {
            member.rank = (i + 1) as u32;
        }
    }

    /// Display leaderboard
    pub fn display(&self) {
        if self.members.len() < 2 {
            return; // Don't show leaderboard for single user
        }

        println!("{} {}", "📊".bold(), "Team Quality Score:".bold());
        println!();

        for member in self.members.iter().take(5) {
            let medal = match member.rank {
                1 => "🥇",
                2 => "🥈",
                3 => "🥉",
                _ => "  ",
            };

            let highlight = member.name == self.current_user;
            let star = if highlight && member.daily_improvement > 0.0 {
                " ⭐"
            } else {
                ""
            };

            let line = format!(
                "{}. {} {}:  {:.1}/10 ({}{})",
                member.rank,
                medal,
                member.name,
                member.score,
                if member.daily_improvement > 0.0 {
                    "↑"
                } else {
                    "→"
                },
                format!("{:.1} today", member.daily_improvement.abs())
            );

            if highlight {
                println!("{}{}", line.bold(), star);
            } else {
                println!("{line}");
            }
        }

        // Check if current user had biggest improvement
        if let Some(user) = self.members.iter().find(|m| m.name == self.current_user) {
            let max_improvement = self
                .members
                .iter()
                .map(|m| m.daily_improvement)
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(0.0);

            if user.daily_improvement == max_improvement && max_improvement > 0.0 {
                println!();
                println!(
                    "{}",
                    "You had the biggest improvement today! 🎉".green().bold()
                );
            }
        }
    }
}
