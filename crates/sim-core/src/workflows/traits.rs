//! Core traits for workflows

use crate::utils::idents::{WorkflowId, WorkflowInstanceId};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// A single step in a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// Index of this step (0-based)
    pub step_index: usize,
    /// Type of resource required for this step
    pub required_resource_type: String,
    /// Expected duration for this step
    pub expected_duration: Duration,
    /// Optional metadata about this step
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Instance of a workflow being executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInstance {
    /// Unique identifier for this instance
    pub id: WorkflowInstanceId,
    /// ID of the workflow definition
    pub workflow_id: WorkflowId,
    /// Current step index
    pub current_step: usize,
    /// Total number of steps
    pub total_steps: usize,
    /// Time when current step is expected to complete
    pub step_completion_time: Option<DateTime<Utc>>,
    /// Whether this workflow instance is complete
    pub is_complete: bool,
    /// Custom metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl WorkflowInstance {
    /// Create a new workflow instance
    pub fn new(workflow_id: WorkflowId, total_steps: usize) -> Self {
        Self {
            id: WorkflowInstanceId::new(),
            workflow_id,
            current_step: 0,
            total_steps,
            step_completion_time: None,
            is_complete: false,
            metadata: HashMap::new(),
        }
    }

    /// Check if this instance is waiting for a step to complete
    pub fn is_processing(&self) -> bool {
        self.step_completion_time.is_some() && !self.is_complete
    }

    /// Check if ready for next step assignment
    pub fn is_ready_for_assignment(&self) -> bool {
        self.step_completion_time.is_none() && !self.is_complete
    }

    /// Advance to the next step
    pub fn advance_step(&mut self, completion_time: DateTime<Utc>) {
        self.current_step += 1;
        self.step_completion_time = Some(completion_time);

        if self.current_step >= self.total_steps {
            self.is_complete = true;
        }
    }

    /// Mark step as assigned but not yet complete
    pub fn mark_assigned(&mut self, completion_time: DateTime<Utc>) {
        self.step_completion_time = Some(completion_time);
    }

    /// Get progress as a percentage (0.0 to 1.0)
    pub fn progress(&self) -> f64 {
        if self.total_steps == 0 {
            0.0
        } else {
            self.current_step as f64 / self.total_steps as f64
        }
    }
}

/// A workflow definition
///
/// Workflows represent multi-stage processing pipelines. Each workflow consists of
/// a sequence of steps that must be completed in order.
///
/// # Example
///
/// ```rust,ignore
/// struct CookingWorkflow {
///     id: WorkflowId,
///     steps: Vec<WorkflowStep>,
/// }
///
/// impl Workflow for CookingWorkflow {
///     fn id(&self) -> WorkflowId {
///         self.id
///     }
///
///     fn workflow_type(&self) -> &str {
///         "cooking"
///     }
///
///     fn steps(&self) -> &[WorkflowStep] {
///         &self.steps
///     }
/// }
/// ```
pub trait Workflow: Send + Sync + fmt::Debug {
    /// Unique identifier for this workflow definition
    fn id(&self) -> WorkflowId;

    /// Type of workflow (e.g., "cooking", "assembly", "fulfillment")
    fn workflow_type(&self) -> &str;

    /// All steps in this workflow
    fn steps(&self) -> &[WorkflowStep];

    /// Get the number of steps
    fn step_count(&self) -> usize {
        self.steps().len()
    }

    /// Get a specific step by index
    fn get_step(&self, index: usize) -> Option<&WorkflowStep> {
        self.steps().get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_instance_creation() {
        let workflow_id = WorkflowId::new();
        let instance = WorkflowInstance::new(workflow_id, 3);

        assert_eq!(instance.workflow_id, workflow_id);
        assert_eq!(instance.current_step, 0);
        assert_eq!(instance.total_steps, 3);
        assert!(!instance.is_complete);
        assert!(instance.is_ready_for_assignment());
        assert!(!instance.is_processing());
    }

    #[test]
    fn test_workflow_instance_progression() {
        let mut instance = WorkflowInstance::new(WorkflowId::new(), 2);

        // Assign first step
        let completion = Utc::now() + Duration::seconds(30);
        instance.mark_assigned(completion);
        assert!(instance.is_processing());
        assert!(!instance.is_ready_for_assignment());

        // Complete first step
        instance.step_completion_time = None; // Reset for next assignment
        assert_eq!(instance.current_step, 0);

        // Advance to next step
        instance.advance_step(completion);
        assert_eq!(instance.current_step, 1);
        assert!(!instance.is_complete);

        // Complete final step
        instance.step_completion_time = None;
        instance.advance_step(completion);
        assert_eq!(instance.current_step, 2);
        assert!(instance.is_complete);
    }

    #[test]
    fn test_workflow_instance_progress() {
        let mut instance = WorkflowInstance::new(WorkflowId::new(), 4);

        assert_eq!(instance.progress(), 0.0);

        instance.current_step = 2;
        assert_eq!(instance.progress(), 0.5);

        instance.current_step = 4;
        assert_eq!(instance.progress(), 1.0);
    }

    #[derive(Debug)]
    struct TestWorkflow {
        id: WorkflowId,
        steps: Vec<WorkflowStep>,
    }

    impl Workflow for TestWorkflow {
        fn id(&self) -> WorkflowId {
            self.id
        }

        fn workflow_type(&self) -> &str {
            "test"
        }

        fn steps(&self) -> &[WorkflowStep] {
            &self.steps
        }
    }

    #[test]
    fn test_workflow_trait() {
        let workflow = TestWorkflow {
            id: WorkflowId::new(),
            steps: vec![
                WorkflowStep {
                    step_index: 0,
                    required_resource_type: "prep".to_string(),
                    expected_duration: Duration::seconds(30),
                    metadata: HashMap::new(),
                },
                WorkflowStep {
                    step_index: 1,
                    required_resource_type: "cook".to_string(),
                    expected_duration: Duration::seconds(60),
                    metadata: HashMap::new(),
                },
            ],
        };

        assert_eq!(workflow.workflow_type(), "test");
        assert_eq!(workflow.step_count(), 2);
        assert_eq!(workflow.get_step(0).unwrap().required_resource_type, "prep");
        assert_eq!(workflow.get_step(1).unwrap().required_resource_type, "cook");
        assert!(workflow.get_step(2).is_none());
    }
}
