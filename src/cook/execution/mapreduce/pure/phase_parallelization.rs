//! Phase parallelization using dependency analysis
//!
//! This module demonstrates how to use dependency analysis to enable parallel
//! execution of commands in setup and reduce phases.
//!
//! Key concept: Commands with no dependencies can run in parallel, while
//! dependent commands must wait for their dependencies to complete.

use crate::cook::execution::mapreduce::pure::dependency_analysis::{
    analyze_dependencies, Command as DepCommand,
};
use std::collections::HashSet;

/// Represents a command to be executed in a phase
#[derive(Debug, Clone)]
pub struct PhaseCommand {
    pub id: String,
    pub command: String,
    pub reads: HashSet<String>,
    pub writes: HashSet<String>,
}

/// A batch of commands that can be executed in parallel
#[derive(Debug, Clone)]
pub struct ParallelBatch {
    pub batch_id: usize,
    pub commands: Vec<PhaseCommand>,
}

/// Result from analyzing commands for parallel execution
#[derive(Debug, Clone)]
pub struct ParallelizationPlan {
    /// Batches of commands that can run in parallel
    pub batches: Vec<ParallelBatch>,
    /// Total number of commands
    pub total_commands: usize,
    /// Maximum parallelism (largest batch size)
    pub max_parallelism: usize,
}

/// Analyze phase commands and create a parallelization plan
///
/// This is the KEY function for setup/reduce parallelization.
/// It uses dependency analysis to identify which commands can run in parallel.
///
/// # Example
///
/// ```ignore
/// let commands = vec![
///     PhaseCommand {
///         id: "cmd1".to_string(),
///         command: "echo 'hello'".to_string(),
///         reads: HashSet::new(),
///         writes: set!["output.txt"],
///     },
///     PhaseCommand {
///         id: "cmd2".to_string(),
///         command: "cat output.txt".to_string(),
///         reads: set!["output.txt"],
///         writes: HashSet::new(),
///     },
///     PhaseCommand {
///         id: "cmd3".to_string(),
///         command: "echo 'world'".to_string(),
///         reads: HashSet::new(),
///         writes: set!["output2.txt"],
///     },
/// ];
///
/// let plan = create_parallelization_plan(&commands);
/// // Result: 2 batches
/// // Batch 0: [cmd1, cmd3] - can run in parallel
/// // Batch 1: [cmd2] - must wait for cmd1 to complete
/// ```
pub fn create_parallelization_plan(commands: &[PhaseCommand]) -> ParallelizationPlan {
    if commands.is_empty() {
        return ParallelizationPlan {
            batches: vec![],
            total_commands: 0,
            max_parallelism: 0,
        };
    }

    // Convert to dependency analysis Command format
    let dep_commands: Vec<DepCommand> = commands
        .iter()
        .map(|cmd| DepCommand {
            reads: cmd.reads.clone(),
            writes: cmd.writes.clone(),
        })
        .collect();

    // Build dependency graph using existing pure function
    let graph = analyze_dependencies(&dep_commands);

    // Get parallel batches from the graph
    let batch_indices = graph.parallel_batches();

    // Create command batches
    let mut batches = Vec::new();
    let mut max_parallelism = 0;

    for (batch_id, indices) in batch_indices.iter().enumerate() {
        let batch_commands: Vec<PhaseCommand> =
            indices.iter().map(|&i| commands[i].clone()).collect();

        max_parallelism = max_parallelism.max(batch_commands.len());

        batches.push(ParallelBatch {
            batch_id,
            commands: batch_commands,
        });
    }

    ParallelizationPlan {
        batches,
        total_commands: commands.len(),
        max_parallelism,
    }
}

/// Calculate expected speedup from parallelization
///
/// Uses Amdahl's Law to estimate speedup based on parallel batches.
/// This is a simplified model that doesn't account for overhead.
pub fn calculate_expected_speedup(plan: &ParallelizationPlan) -> f64 {
    if plan.total_commands == 0 {
        return 1.0;
    }

    // Sequential time = number of commands
    let sequential_time = plan.total_commands as f64;

    // Parallel time = number of batches (assuming uniform command duration)
    let parallel_time = plan.batches.len() as f64;

    sequential_time / parallel_time
}

/// Analyze commands to extract read/write sets
///
/// This is a simplified version. Real implementation would parse command strings
/// to detect file operations, variable reads/writes, etc.
pub fn analyze_command_dependencies(command: &str) -> (HashSet<String>, HashSet<String>) {
    let mut reads = HashSet::new();
    let mut writes = HashSet::new();

    // Simple heuristics for demonstration
    if command.contains('>') || command.contains("write") {
        // Likely writes a file
        writes.insert("output".to_string());
    }

    if command.contains("cat") || command.contains("read") || command.contains('<') {
        // Likely reads a file
        reads.insert("input".to_string());
    }

    (reads, writes)
}

/// Example: Create phase commands from shell commands
///
/// This shows how to convert workflow steps into PhaseCommand structures
/// for parallelization analysis.
pub fn create_phase_commands_from_shell(shell_commands: &[String]) -> Vec<PhaseCommand> {
    shell_commands
        .iter()
        .enumerate()
        .map(|(i, cmd)| {
            let (reads, writes) = analyze_command_dependencies(cmd);
            PhaseCommand {
                id: format!("cmd-{}", i),
                command: cmd.clone(),
                reads,
                writes,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set<T>(items: &[T]) -> HashSet<String>
    where
        T: ToString,
    {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_create_parallelization_plan_independent_commands() {
        let commands = vec![
            PhaseCommand {
                id: "cmd1".to_string(),
                command: "echo 'hello'".to_string(),
                reads: HashSet::new(),
                writes: set(&["file1.txt"]),
            },
            PhaseCommand {
                id: "cmd2".to_string(),
                command: "echo 'world'".to_string(),
                reads: HashSet::new(),
                writes: set(&["file2.txt"]),
            },
            PhaseCommand {
                id: "cmd3".to_string(),
                command: "echo 'parallel'".to_string(),
                reads: HashSet::new(),
                writes: set(&["file3.txt"]),
            },
        ];

        let plan = create_parallelization_plan(&commands);

        // All commands are independent, should be in one batch
        assert_eq!(plan.batches.len(), 1);
        assert_eq!(plan.batches[0].commands.len(), 3);
        assert_eq!(plan.max_parallelism, 3);
        assert_eq!(plan.total_commands, 3);
    }

    #[test]
    fn test_create_parallelization_plan_dependent_commands() {
        let commands = vec![
            PhaseCommand {
                id: "cmd1".to_string(),
                command: "echo 'data' > file.txt".to_string(),
                reads: HashSet::new(),
                writes: set(&["file.txt"]),
            },
            PhaseCommand {
                id: "cmd2".to_string(),
                command: "cat file.txt".to_string(),
                reads: set(&["file.txt"]),
                writes: HashSet::new(),
            },
            PhaseCommand {
                id: "cmd3".to_string(),
                command: "echo 'more data' > file2.txt".to_string(),
                reads: HashSet::new(),
                writes: set(&["file2.txt"]),
            },
        ];

        let plan = create_parallelization_plan(&commands);

        // cmd1 and cmd3 can run in parallel (batch 0)
        // cmd2 must wait for cmd1 (batch 1)
        assert_eq!(plan.batches.len(), 2);
        assert_eq!(plan.batches[0].commands.len(), 2); // cmd1, cmd3
        assert_eq!(plan.batches[1].commands.len(), 1); // cmd2
        assert_eq!(plan.max_parallelism, 2);
    }

    #[test]
    fn test_calculate_expected_speedup() {
        // All independent commands
        let plan = ParallelizationPlan {
            batches: vec![ParallelBatch {
                batch_id: 0,
                commands: vec![],
            }],
            total_commands: 10,
            max_parallelism: 10,
        };
        let speedup = calculate_expected_speedup(&plan);
        assert_eq!(speedup, 10.0);

        // Sequential execution (one command per batch)
        let plan = ParallelizationPlan {
            batches: vec![
                ParallelBatch {
                    batch_id: 0,
                    commands: vec![],
                },
                ParallelBatch {
                    batch_id: 1,
                    commands: vec![],
                },
                ParallelBatch {
                    batch_id: 2,
                    commands: vec![],
                },
            ],
            total_commands: 3,
            max_parallelism: 1,
        };
        let speedup = calculate_expected_speedup(&plan);
        assert_eq!(speedup, 1.0);
    }

    #[test]
    fn test_analyze_command_dependencies() {
        // Write command
        let (_reads, writes) = analyze_command_dependencies("echo 'test' > output.txt");
        assert!(writes.contains("output"));

        // Read command
        let (reads, _writes) = analyze_command_dependencies("cat input.txt");
        assert!(reads.contains("input"));

        // Read and write
        let (reads, writes) = analyze_command_dependencies("cat input.txt > output.txt");
        assert!(reads.contains("input"));
        assert!(writes.contains("output"));
    }

    #[test]
    fn test_create_phase_commands_from_shell() {
        let shell_commands = vec![
            "echo 'hello' > file1.txt".to_string(),
            "cat file1.txt".to_string(),
            "echo 'world' > file2.txt".to_string(),
        ];

        let commands = create_phase_commands_from_shell(&shell_commands);

        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0].id, "cmd-0");
        assert_eq!(commands[1].id, "cmd-1");
        assert_eq!(commands[2].id, "cmd-2");
    }

    #[test]
    fn test_empty_plan() {
        let plan = create_parallelization_plan(&[]);

        assert_eq!(plan.batches.len(), 0);
        assert_eq!(plan.total_commands, 0);
        assert_eq!(plan.max_parallelism, 0);
        assert_eq!(calculate_expected_speedup(&plan), 1.0);
    }
}
