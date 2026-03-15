//! Workflow engine implementation

use super::traits::{Workflow, WorkflowInstance};
use crate::context::SimulationContext;
use crate::utils::idents::{WorkflowId, WorkflowInstanceId};
use crate::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::marker::PhantomData;

/// Events emitted by workflow progression
#[derive(Debug, Clone)]
pub enum WorkflowStepEvent {
    /// A workflow step was completed
    StepCompleted {
        instance_id: WorkflowInstanceId,
        step_index: usize,
        completion_time: DateTime<Utc>,
    },
    /// An entire workflow was completed
    WorkflowCompleted {
        instance_id: WorkflowInstanceId,
        workflow_id: WorkflowId,
        completion_time: DateTime<Utc>,
    },
}

/// Manages workflow execution
///
/// The workflow engine tracks active workflow instances and manages their progression
/// through multiple steps.
///
/// # Type Parameters
///
/// * `W` - Workflow type
///
/// # Example
///
/// ```rust,ignore
/// use sim_core::WorkflowEngine;
///
/// let mut engine = WorkflowEngine::<MyWorkflow>::new();
///
/// // Register workflow definitions
/// engine.register_workflow(cooking_workflow);
///
/// // Start instances
/// let instance = engine.start_instance(workflow_id)?;
///
/// // Advance instances based on time
/// let completed = engine.advance_instances(&ctx)?;
/// ```
pub struct WorkflowEngine<W: Workflow> {
    /// Registered workflow definitions
    workflows: HashMap<WorkflowId, W>,
    /// Active workflow instances
    instances: HashMap<WorkflowInstanceId, WorkflowInstance>,
    /// Phantom data
    _phantom: PhantomData<W>,
}

impl<W: Workflow> WorkflowEngine<W> {
    /// Create a new workflow engine
    pub fn new() -> Self {
        Self {
            workflows: HashMap::new(),
            instances: HashMap::new(),
            _phantom: PhantomData,
        }
    }

    /// Register a workflow definition
    pub fn register_workflow(&mut self, workflow: W) {
        let id = workflow.id();
        self.workflows.insert(id, workflow);
    }

    /// Get a workflow definition
    pub fn get_workflow(&self, id: WorkflowId) -> Option<&W> {
        self.workflows.get(&id)
    }

    /// Start a new workflow instance
    ///
    /// # Arguments
    ///
    /// * `workflow_id` - ID of the workflow definition to instantiate
    ///
    /// # Returns
    ///
    /// The created workflow instance
    pub fn start_instance(&mut self, workflow_id: WorkflowId) -> Result<WorkflowInstance> {
        let workflow = self
            .workflows
            .get(&workflow_id)
            .ok_or_else(|| crate::Error::Workflow(format!("Workflow {} not found", workflow_id)))?;

        let instance = WorkflowInstance::new(workflow_id, workflow.step_count());
        let id = instance.id;
        self.instances.insert(id, instance.clone());

        Ok(instance)
    }

    /// Get a workflow instance
    pub fn get_instance(&self, id: WorkflowInstanceId) -> Option<&WorkflowInstance> {
        self.instances.get(&id)
    }

    /// Get a mutable workflow instance
    pub fn get_instance_mut(&mut self, id: WorkflowInstanceId) -> Option<&mut WorkflowInstance> {
        self.instances.get_mut(&id)
    }

    /// Process workflow instances for current tick
    ///
    /// This method is called once per simulation tick to process time-based
    /// workflow progressions. Currently, most workflows are resource-based
    /// (progression happens via `advance_instance_step()`), so this mainly
    /// serves as a hook for future time-based steps.
    ///
    /// # Arguments
    ///
    /// * `current_time` - Current simulation time
    ///
    /// # Returns
    ///
    /// Vector of workflow step events emitted during this tick
    pub fn tick(&mut self, _current_time: DateTime<Utc>) -> Result<Vec<WorkflowStepEvent>> {
        let events = Vec::new();

        // For now, most workflows are resource-based (tasks complete when resources finish)
        // In the future, we could add time-based steps (e.g., "wait 5 minutes")
        // that would be processed here

        Ok(events)
    }

    /// Advance instance to next step after task completion
    ///
    /// This method is called when a task associated with a workflow instance
    /// completes. It marks the current step as complete and moves to the next step.
    ///
    /// # Arguments
    ///
    /// * `instance_id` - ID of the workflow instance to advance
    /// * `completion_time` - Time when the task/step completed
    ///
    /// # Returns
    ///
    /// Optional workflow step event (StepCompleted or WorkflowCompleted)
    pub fn advance_instance_step(
        &mut self,
        instance_id: WorkflowInstanceId,
        completion_time: DateTime<Utc>,
    ) -> Result<Option<WorkflowStepEvent>> {
        let instance = self
            .instances
            .get_mut(&instance_id)
            .ok_or_else(|| {
                crate::Error::Workflow(format!("Workflow instance {} not found", instance_id))
            })?;

        // Check if workflow is already complete
        if instance.is_complete {
            return Err(crate::Error::Workflow(format!(
                "Workflow instance {} is already complete",
                instance_id
            )));
        }

        // Store current step for event
        let completed_step = instance.current_step;

        // Mark current step complete and advance
        instance.step_completion_time = None;
        instance.current_step += 1;

        // Check if workflow is complete
        if instance.current_step >= instance.total_steps {
            instance.is_complete = true;

            return Ok(Some(WorkflowStepEvent::WorkflowCompleted {
                instance_id,
                workflow_id: instance.workflow_id,
                completion_time,
            }));
        }

        // Still has more steps
        Ok(Some(WorkflowStepEvent::StepCompleted {
            instance_id,
            step_index: completed_step,
            completion_time,
        }))
    }

    /// Advance workflow instances
    ///
    /// Checks all instances and completes steps that have passed their completion time.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Simulation context with current time
    ///
    /// # Returns
    ///
    /// Vector of instance IDs that completed
    pub fn advance_instances(&mut self, ctx: &SimulationContext) -> Result<Vec<WorkflowInstanceId>> {
        let mut completed = Vec::new();
        let current_time = ctx.current_time();

        for instance in self.instances.values_mut() {
            if instance.is_processing()
                && let Some(completion_time) = instance.step_completion_time
                && current_time >= completion_time
            {
                // Step is complete, ready for next step
                instance.step_completion_time = None;

                if instance.current_step >= instance.total_steps {
                    instance.is_complete = true;
                    completed.push(instance.id);
                }
            }
        }

        Ok(completed)
    }

    /// Get all instances ready for step assignment
    pub fn get_ready_instances(&self) -> Vec<&WorkflowInstance> {
        self.instances
            .values()
            .filter(|i| i.is_ready_for_assignment())
            .collect()
    }

    /// Get all processing instances
    pub fn get_processing_instances(&self) -> Vec<&WorkflowInstance> {
        self.instances
            .values()
            .filter(|i| i.is_processing())
            .collect()
    }

    /// Get all completed instances
    pub fn get_completed_instances(&self) -> Vec<&WorkflowInstance> {
        self.instances
            .values()
            .filter(|i| i.is_complete)
            .collect()
    }

    /// Remove completed instances
    pub fn remove_completed(&mut self) -> Vec<WorkflowInstance> {
        let completed_ids: Vec<_> = self
            .instances
            .iter()
            .filter(|(_, i)| i.is_complete)
            .map(|(id, _)| *id)
            .collect();

        completed_ids
            .into_iter()
            .filter_map(|id| self.instances.remove(&id))
            .collect()
    }

    /// Get total instance count
    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }

    /// Get workflow count
    pub fn workflow_count(&self) -> usize {
        self.workflows.len()
    }
}

impl<W: Workflow> Default for WorkflowEngine<W> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflows::WorkflowStep;
    use chrono::Duration;

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

    fn create_test_workflow() -> TestWorkflow {
        TestWorkflow {
            id: WorkflowId::new(),
            steps: vec![
                WorkflowStep {
                    step_index: 0,
                    required_resource_type: "step1".to_string(),
                    expected_duration: Duration::seconds(10),
                    metadata: HashMap::new(),
                },
                WorkflowStep {
                    step_index: 1,
                    required_resource_type: "step2".to_string(),
                    expected_duration: Duration::seconds(20),
                    metadata: HashMap::new(),
                },
            ],
        }
    }

    #[test]
    fn test_engine_creation() {
        let engine = WorkflowEngine::<TestWorkflow>::new();
        assert_eq!(engine.workflow_count(), 0);
        assert_eq!(engine.instance_count(), 0);
    }

    #[test]
    fn test_register_workflow() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let id = workflow.id();

        engine.register_workflow(workflow);

        assert_eq!(engine.workflow_count(), 1);
        assert!(engine.get_workflow(id).is_some());
    }

    #[test]
    fn test_start_instance() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        let instance = engine.start_instance(workflow_id).unwrap();

        assert_eq!(instance.workflow_id, workflow_id);
        assert_eq!(instance.total_steps, 2);
        assert_eq!(instance.current_step, 0);
        assert!(!instance.is_complete);
        assert_eq!(engine.instance_count(), 1);
    }

    #[test]
    fn test_advance_instances() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        let mut instance = engine.start_instance(workflow_id).unwrap();
        let instance_id = instance.id;

        // Assign first step
        let completion_time = chrono::Utc::now() + Duration::seconds(10);
        instance.mark_assigned(completion_time);
        *engine.get_instance_mut(instance_id).unwrap() = instance;

        // Advance time past completion
        let config = crate::context::SimulationConfig::default();
        let mut ctx = SimulationContext::new(chrono::Utc::now(), config);
        ctx.set_current_time(completion_time + Duration::seconds(1));

        let completed = engine.advance_instances(&ctx).unwrap();

        // Step should be complete but workflow not finished yet
        assert_eq!(completed.len(), 0);
        let instance = engine.get_instance(instance_id).unwrap();
        assert!(!instance.is_processing());
        assert!(instance.is_ready_for_assignment());
    }

    #[test]
    fn test_get_ready_instances() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);
        engine.start_instance(workflow_id).unwrap();

        let ready = engine.get_ready_instances();
        assert_eq!(ready.len(), 1);
    }

    #[test]
    fn test_tick() {
        let mut engine = WorkflowEngine::<TestWorkflow>::new();
        let current_time = chrono::Utc::now();

        let events = engine.tick(current_time).unwrap();

        // For now, tick should return empty events (no time-based steps)
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn test_advance_instance_step() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);
        let instance = engine.start_instance(workflow_id).unwrap();
        let instance_id = instance.id;

        let completion_time = chrono::Utc::now();

        // Advance first step
        let event = engine.advance_instance_step(instance_id, completion_time).unwrap();

        assert!(event.is_some());
        match event.unwrap() {
            super::WorkflowStepEvent::StepCompleted { instance_id: id, step_index, .. } => {
                assert_eq!(id, instance_id);
                assert_eq!(step_index, 0);
            },
            _ => panic!("Expected StepCompleted event"),
        }

        let instance = engine.get_instance(instance_id).unwrap();
        assert_eq!(instance.current_step, 1);
        assert!(!instance.is_complete);
    }

    #[test]
    fn test_advance_instance_step_complete_workflow() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);
        let instance = engine.start_instance(workflow_id).unwrap();
        let instance_id = instance.id;

        let completion_time = chrono::Utc::now();

        // Advance through all steps
        engine.advance_instance_step(instance_id, completion_time).unwrap();

        let event = engine.advance_instance_step(instance_id, completion_time).unwrap();

        assert!(event.is_some());
        match event.unwrap() {
            super::WorkflowStepEvent::WorkflowCompleted { instance_id: id, workflow_id: wf_id, .. } => {
                assert_eq!(id, instance_id);
                assert_eq!(wf_id, workflow_id);
            },
            _ => panic!("Expected WorkflowCompleted event"),
        }

        let instance = engine.get_instance(instance_id).unwrap();
        assert_eq!(instance.current_step, 2);
        assert!(instance.is_complete);
    }

    #[test]
    fn test_advance_nonexistent_workflow_instance_fails() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();

        engine.register_workflow(workflow);

        // Try to advance a non-existent instance
        let fake_instance_id = WorkflowInstanceId::new();
        let completion_time = chrono::Utc::now();

        let result = engine.advance_instance_step(fake_instance_id, completion_time);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("Workflow error: Workflow instance {} not found", fake_instance_id)
        );
    }

    #[test]
    fn test_advance_already_complete_workflow_fails() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);
        let instance = engine.start_instance(workflow_id).unwrap();
        let instance_id = instance.id;

        let completion_time = chrono::Utc::now();

        // Advance through all steps to complete the workflow
        engine.advance_instance_step(instance_id, completion_time).unwrap();
        engine.advance_instance_step(instance_id, completion_time).unwrap();

        // Verify workflow is complete
        let instance = engine.get_instance(instance_id).unwrap();
        assert!(instance.is_complete);

        // Try to advance again
        let result = engine.advance_instance_step(instance_id, completion_time);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("Workflow error: Workflow instance {} is already complete", instance_id)
        );
    }
}
