use crate::worktree::{InterruptionType, WorktreeManager};
use anyhow::Result;
use chrono::Utc;
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;
use std::sync::Arc;
use std::thread;

/// Set up interrupt handlers for graceful shutdown
///
/// This function installs signal handlers for SIGINT (Ctrl-C) and SIGTERM
/// that will update the worktree state to mark it as interrupted before exit.
pub fn setup_interrupt_handlers(
    worktree_manager: Arc<WorktreeManager>,
    session_name: String,
) -> Result<()> {
    let mut signals = Signals::new([SIGINT, SIGTERM])?;

    thread::spawn(move || {
        #[allow(clippy::never_loop)]
        for sig in signals.forever() {
            match sig {
                SIGINT => {
                    update_interrupted_state(
                        &worktree_manager,
                        &session_name,
                        InterruptionType::UserInterrupt,
                    );
                    std::process::exit(130); // Standard exit code for SIGINT
                }
                SIGTERM => {
                    update_interrupted_state(
                        &worktree_manager,
                        &session_name,
                        InterruptionType::Termination,
                    );
                    std::process::exit(143); // Standard exit code for SIGTERM
                }
                _ => unreachable!(),
            }
        }
    });

    Ok(())
}

/// Update the worktree state to mark it as interrupted
fn update_interrupted_state(
    worktree_manager: &WorktreeManager,
    session_name: &str,
    interruption_type: InterruptionType,
) {
    let _ = worktree_manager.update_session_state(session_name, |state| {
        state.status = crate::worktree::WorktreeStatus::Interrupted;
        state.interrupted_at = Some(Utc::now());
        state.interruption_type = Some(interruption_type);
        state.resumable = true;
    });
}
