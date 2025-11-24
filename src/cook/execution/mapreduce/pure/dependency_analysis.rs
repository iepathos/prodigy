//! Pure dependency analysis for command parallelization
//!
//! This module provides pure functions for analyzing command dependencies
//! to enable safe parallel execution of independent commands.

use std::collections::{HashMap, HashSet};

/// A command that can be analyzed for dependencies
#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    /// Variables this command reads
    pub reads: HashSet<String>,
    /// Variables this command writes
    pub writes: HashSet<String>,
}

/// Dependency graph for commands
#[derive(Debug, Clone)]
pub struct CommandGraph {
    /// Number of commands
    node_count: usize,
    /// Dependencies: node -> set of nodes it depends on
    dependencies: HashMap<usize, HashSet<usize>>,
}

impl CommandGraph {
    /// Create a new empty graph
    pub fn new(node_count: usize) -> Self {
        Self {
            node_count,
            dependencies: HashMap::new(),
        }
    }

    /// Add a dependency edge: `from` depends on `to`
    pub fn add_edge(&mut self, from: usize, to: usize) {
        self.dependencies.entry(from).or_default().insert(to);
    }

    /// Get number of nodes
    pub fn node_count(&self) -> usize {
        self.node_count
    }

    /// Get dependencies for a node
    pub fn dependencies(&self, node: usize) -> impl Iterator<Item = usize> + '_ {
        self.dependencies
            .get(&node)
            .map(|deps| deps.iter().copied())
            .into_iter()
            .flatten()
    }

    /// Extract parallel execution batches using topological sort
    ///
    /// Returns a list of batches where all commands in a batch can execute
    /// in parallel without violating dependencies.
    pub fn parallel_batches(&self) -> Vec<Vec<usize>> {
        let mut batches = Vec::new();
        let mut remaining: HashSet<_> = (0..self.node_count).collect();

        while !remaining.is_empty() {
            // Find nodes with no unsatisfied dependencies
            let ready: Vec<_> = remaining
                .iter()
                .filter(|&&idx| self.dependencies(idx).all(|dep| !remaining.contains(&dep)))
                .copied()
                .collect();

            // If no nodes are ready but we have remaining nodes, there's a cycle
            if ready.is_empty() {
                // Break cycle by taking any remaining node
                // This is conservative - we'll execute sequentially
                let next = *remaining.iter().next().expect("remaining not empty");
                batches.push(vec![next]);
                remaining.remove(&next);
            } else {
                batches.push(ready.clone());
                for idx in ready {
                    remaining.remove(&idx);
                }
            }
        }

        batches
    }

    /// Check if the graph has cycles
    pub fn has_cycles(&self) -> bool {
        // Use depth-first search to detect cycles
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for node in 0..self.node_count {
            if !visited.contains(&node) && self.has_cycle_dfs(node, &mut visited, &mut rec_stack) {
                return true;
            }
        }

        false
    }

    fn has_cycle_dfs(
        &self,
        node: usize,
        visited: &mut HashSet<usize>,
        rec_stack: &mut HashSet<usize>,
    ) -> bool {
        visited.insert(node);
        rec_stack.insert(node);

        for dep in self.dependencies(node) {
            if !visited.contains(&dep) {
                if self.has_cycle_dfs(dep, visited, rec_stack) {
                    return true;
                }
            } else if rec_stack.contains(&dep) {
                return true;
            }
        }

        rec_stack.remove(&node);
        false
    }
}

/// Pure: Analyze command dependencies and build dependency graph
///
/// Analyzes read/write sets of commands to detect data dependencies.
/// Command A depends on command B if A reads a variable that B writes.
///
/// This analysis is conservative: it may identify false dependencies
/// (where variables happen to have the same name but don't actually conflict),
/// but it will never miss a real dependency.
pub fn analyze_dependencies(commands: &[Command]) -> CommandGraph {
    let mut graph = CommandGraph::new(commands.len());

    // For each command, check if it depends on any prior commands
    for (idx, cmd) in commands.iter().enumerate() {
        // Look at all previous commands
        for (prior_idx, prior_cmd) in commands[..idx].iter().enumerate() {
            // Command depends on prior if it reads what prior writes
            // This is Read-After-Write (RAW) dependency
            if cmd.reads.iter().any(|var| prior_cmd.writes.contains(var)) {
                graph.add_edge(idx, prior_idx);
            }

            // Or if it writes what prior writes (Write-After-Write)
            if cmd.writes.iter().any(|var| prior_cmd.writes.contains(var)) {
                graph.add_edge(idx, prior_idx);
            }

            // Or if it writes what prior reads (Write-After-Read)
            if cmd.writes.iter().any(|var| prior_cmd.reads.contains(var)) {
                graph.add_edge(idx, prior_idx);
            }
        }
    }

    graph
}

/// Extract variable reads from a shell command string
///
/// This is a heuristic parser that looks for common patterns:
/// - $VAR
/// - ${VAR}
/// - ${VAR:-default}
pub fn extract_variable_reads(cmd_str: &str) -> HashSet<String> {
    let mut vars = HashSet::new();
    let mut chars = cmd_str.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' {
            if chars.peek() == Some(&'{') {
                // ${VAR} or ${VAR:-default}
                chars.next(); // consume '{'
                let var_name: String = chars
                    .by_ref()
                    .take_while(|&c| c != '}' && c != ':' && c != '-')
                    .collect();
                if !var_name.is_empty() {
                    vars.insert(var_name);
                }
            } else {
                // $VAR
                let var_name: String = chars
                    .by_ref()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !var_name.is_empty() {
                    vars.insert(var_name);
                }
            }
        }
    }

    vars
}

/// Extract variable writes from a shell command string
///
/// Looks for assignment patterns:
/// - VAR=value
/// - export VAR=value
pub fn extract_variable_writes(cmd_str: &str) -> HashSet<String> {
    let mut vars = HashSet::new();

    // Split by semicolons and newlines to handle multiple commands
    for part in cmd_str.split(&[';', '\n'][..]) {
        let trimmed = part.trim();

        // Handle export VAR=value
        let assignment_part = if let Some(after_export) = trimmed.strip_prefix("export ") {
            after_export
        } else {
            trimmed
        };

        // Look for VAR=value pattern
        if let Some(eq_pos) = assignment_part.find('=') {
            let var_name = assignment_part[..eq_pos].trim();
            // Check if it looks like a valid variable name
            if !var_name.is_empty() && var_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                vars.insert(var_name.to_string());
            }
        }
    }

    vars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_dependencies_independent_commands() {
        let commands = vec![
            Command {
                reads: HashSet::new(),
                writes: ["A".to_string()].into_iter().collect(),
            },
            Command {
                reads: HashSet::new(),
                writes: ["B".to_string()].into_iter().collect(),
            },
            Command {
                reads: HashSet::new(),
                writes: ["C".to_string()].into_iter().collect(),
            },
        ];

        let graph = analyze_dependencies(&commands);
        let batches = graph.parallel_batches();

        // All commands are independent, should be in one batch
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 3);
    }

    #[test]
    fn test_analyze_dependencies_sequential() {
        let commands = vec![
            Command {
                reads: HashSet::new(),
                writes: ["A".to_string()].into_iter().collect(),
            },
            Command {
                reads: ["A".to_string()].into_iter().collect(),
                writes: ["B".to_string()].into_iter().collect(),
            },
            Command {
                reads: ["B".to_string()].into_iter().collect(),
                writes: ["C".to_string()].into_iter().collect(),
            },
        ];

        let graph = analyze_dependencies(&commands);
        let batches = graph.parallel_batches();

        // Commands must execute sequentially
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0], vec![0]);
        assert_eq!(batches[1], vec![1]);
        assert_eq!(batches[2], vec![2]);
    }

    #[test]
    fn test_analyze_dependencies_partial_parallelism() {
        let commands = vec![
            Command {
                reads: HashSet::new(),
                writes: ["A".to_string()].into_iter().collect(),
            },
            Command {
                reads: ["A".to_string()].into_iter().collect(),
                writes: ["B".to_string()].into_iter().collect(),
            },
            Command {
                reads: ["A".to_string()].into_iter().collect(),
                writes: ["C".to_string()].into_iter().collect(),
            },
        ];

        let graph = analyze_dependencies(&commands);
        let batches = graph.parallel_batches();

        // Command 0 must be first, then 1 and 2 can run in parallel
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0], vec![0]);
        assert_eq!(batches[1].len(), 2);
        assert!(batches[1].contains(&1));
        assert!(batches[1].contains(&2));
    }

    #[test]
    fn test_extract_variable_reads() {
        let cmd = "echo $FOO ${BAR} ${BAZ:-default}";
        let reads = extract_variable_reads(cmd);

        assert_eq!(reads.len(), 3);
        assert!(reads.contains("FOO"));
        assert!(reads.contains("BAR"));
        assert!(reads.contains("BAZ"));
    }

    #[test]
    fn test_extract_variable_writes() {
        let cmd = "FOO=1; export BAR=2; BAZ=3";
        let writes = extract_variable_writes(cmd);

        assert_eq!(writes.len(), 3);
        assert!(writes.contains("FOO"));
        assert!(writes.contains("BAR"));
        assert!(writes.contains("BAZ"));
    }

    #[test]
    fn test_graph_has_no_cycles() {
        let commands = vec![
            Command {
                reads: HashSet::new(),
                writes: ["A".to_string()].into_iter().collect(),
            },
            Command {
                reads: ["A".to_string()].into_iter().collect(),
                writes: ["B".to_string()].into_iter().collect(),
            },
        ];

        let graph = analyze_dependencies(&commands);
        assert!(!graph.has_cycles());
    }
}
