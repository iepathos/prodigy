//! Pure step planning functions for workflow execution
//!
//! This module provides pure functions for analyzing and planning workflow step execution.
//! All functions are side-effect free and can be tested without I/O.

use crate::cook::workflow::normalized::{NormalizedStep, NormalizedWorkflow};

/// Plan for executing a step
#[derive(Debug, Clone)]
pub struct StepPlan {
    /// Step index
    pub index: usize,
    /// Step to execute
    pub step: NormalizedStep,
    /// Whether this step requires a commit
    pub commit_required: bool,
    /// Whether this step is idempotent (safe to retry)
    pub idempotent: bool,
    /// Dependencies (step indices that must complete first)
    pub depends_on: Vec<usize>,
}

/// Plan steps for workflow execution (pure function)
///
/// Analyzes the workflow and produces a sequence of step plans.
/// This is a pure function with no I/O.
///
/// By default, steps are executed sequentially, meaning each step depends on all
/// previous steps. Future enhancements could support parallel execution by analyzing
/// step dependencies.
pub fn plan_steps(workflow: &NormalizedWorkflow) -> Vec<StepPlan> {
    workflow
        .steps
        .iter()
        .enumerate()
        .map(|(idx, step)| {
            let commit_required = step.commit_required;
            // Default to idempotent unless marked otherwise
            let idempotent = true;

            StepPlan {
                index: idx,
                step: step.clone(),
                commit_required,
                idempotent,
                // Sequential by default - all previous steps are dependencies
                depends_on: (0..idx).collect(),
            }
        })
        .collect()
}

/// Decision for resuming a step
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeDecision {
    /// Skip this step (already completed)
    Skip,
    /// Execute this step
    Execute,
    /// Execute with warning
    WarnAndExecute { warning: String },
}

/// Determine if a step is safe to resume (idempotent)
///
/// This is a pure function that decides whether a step should be executed,
/// skipped, or executed with a warning based on its completion status and
/// idempotency.
pub fn is_safe_to_resume(plan: &StepPlan, was_completed: bool) -> ResumeDecision {
    if was_completed {
        ResumeDecision::Skip
    } else if plan.idempotent {
        ResumeDecision::Execute
    } else {
        ResumeDecision::WarnAndExecute {
            warning: format!(
                "Step {} is not marked as idempotent. Re-execution may have side effects.",
                plan.index
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::workflow::normalized::{
        ExecutionMode, NormalizedStep, NormalizedWorkflow, StepCommand, StepHandlers,
    };
    use std::collections::HashMap;
    use std::sync::Arc;

    fn create_test_step(id: &str, commit_required: bool) -> NormalizedStep {
        NormalizedStep {
            id: id.into(),
            command: StepCommand::Shell("echo test".into()),
            validation: None,
            handlers: StepHandlers::default(),
            timeout: None,
            working_dir: None,
            env: Arc::new(HashMap::new()),
            outputs: None,
            commit_required,
            when: None,
        }
    }

    fn create_test_workflow(num_steps: usize) -> NormalizedWorkflow {
        let steps: Vec<NormalizedStep> = (0..num_steps)
            .map(|i| create_test_step(&format!("step-{}", i), false))
            .collect();

        NormalizedWorkflow {
            name: "test-workflow".into(),
            steps: Arc::from(steps),
            execution_mode: ExecutionMode::Sequential,
            variables: Arc::new(HashMap::new()),
        }
    }

    #[test]
    fn test_plan_steps_sequential() {
        let workflow = create_test_workflow(3);
        let plans = plan_steps(&workflow);

        assert_eq!(plans.len(), 3);
        assert_eq!(plans[0].depends_on, Vec::<usize>::new());
        assert_eq!(plans[1].depends_on, vec![0]);
        assert_eq!(plans[2].depends_on, vec![0, 1]);
    }

    #[test]
    fn test_plan_steps_empty_workflow() {
        let workflow = create_test_workflow(0);
        let plans = plan_steps(&workflow);

        assert_eq!(plans.len(), 0);
    }

    #[test]
    fn test_plan_steps_single_step() {
        let workflow = create_test_workflow(1);
        let plans = plan_steps(&workflow);

        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].depends_on, Vec::<usize>::new());
        assert_eq!(plans[0].index, 0);
    }

    #[test]
    fn test_plan_steps_preserves_commit_required() {
        let steps = vec![
            create_test_step("step-1", false),
            create_test_step("step-2", true),
        ];

        let workflow = NormalizedWorkflow {
            name: "test-workflow".into(),
            steps: Arc::from(steps),
            execution_mode: ExecutionMode::Sequential,
            variables: Arc::new(HashMap::new()),
        };

        let plans = plan_steps(&workflow);

        assert!(!plans[0].commit_required);
        assert!(plans[1].commit_required);
    }

    #[test]
    fn test_is_safe_to_resume_completed() {
        let plan = StepPlan {
            index: 0,
            step: create_test_step("test", false),
            commit_required: false,
            idempotent: true,
            depends_on: vec![],
        };

        let decision = is_safe_to_resume(&plan, true);
        assert_eq!(decision, ResumeDecision::Skip);
    }

    #[test]
    fn test_is_safe_to_resume_idempotent() {
        let plan = StepPlan {
            index: 0,
            step: create_test_step("test", false),
            commit_required: false,
            idempotent: true,
            depends_on: vec![],
        };

        let decision = is_safe_to_resume(&plan, false);
        assert_eq!(decision, ResumeDecision::Execute);
    }

    #[test]
    fn test_is_safe_to_resume_non_idempotent() {
        let plan = StepPlan {
            index: 0,
            step: create_test_step("test", false),
            commit_required: false,
            idempotent: false,
            depends_on: vec![],
        };

        let decision = is_safe_to_resume(&plan, false);
        match decision {
            ResumeDecision::WarnAndExecute { warning } => {
                assert!(warning.contains("not marked as idempotent"));
            }
            _ => panic!("Expected WarnAndExecute"),
        }
    }
}
