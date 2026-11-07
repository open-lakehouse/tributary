// Custom Workflow Implementation Template
//
// This template provides a well-commented boilerplate for creating new workflow types.
// Workflows represent multi-stage processing pipelines.

use sim_core::{Workflow, WorkflowId, WorkflowStep, Task, TaskId, TaskFactory, WorkflowInstanceId};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

// =============================================================================
// Step 1: Define Your Workflow Struct
// =============================================================================

/// Your custom workflow type.
/// Workflows define a sequence of steps that must be completed in order.
#[derive(Debug, Clone)]
pub struct MyWorkflow {
    /// Unique identifier (required)
    id: WorkflowId,

    /// Workflow name for debugging
    name: String,

    /// Ordered sequence of steps (required)
    steps: Vec<WorkflowStep>,

    // Add your domain-specific fields:
    /// Example: Workflow category
    category: String,

    /// Example: SLA duration
    sla_duration: Duration,
}

// =============================================================================
// Step 2: Implement the Workflow Trait
// =============================================================================

impl Workflow for MyWorkflow {
    // -------------------------------------------------------------------------
    // Required: Unique identifier
    // -------------------------------------------------------------------------
    fn id(&self) -> WorkflowId {
        self.id
    }

    // -------------------------------------------------------------------------
    // Required: Workflow type for categorization
    // -------------------------------------------------------------------------
    fn workflow_type(&self) -> &str {
        "my_workflow"  // Change to your domain-specific type
    }

    // -------------------------------------------------------------------------
    // Required: Workflow steps
    // -------------------------------------------------------------------------
    fn steps(&self) -> &[WorkflowStep] {
        &self.steps
    }

    // -------------------------------------------------------------------------
    // Optional: Step count (has default implementation)
    // -------------------------------------------------------------------------
    fn step_count(&self) -> usize {
        self.steps.len()
    }

    // -------------------------------------------------------------------------
    // Optional: Get specific step (has default implementation)
    // -------------------------------------------------------------------------
    fn get_step(&self, index: usize) -> Option<&WorkflowStep> {
        self.steps.get(index)
    }
}

// =============================================================================
// Step 3: Constructor and Builder Methods
// =============================================================================

impl MyWorkflow {
    /// Create a new workflow with empty steps
    pub fn new(id: WorkflowId, name: String) -> Self {
        Self {
            id,
            name,
            steps: Vec::new(),
            category: "default".to_string(),
            sla_duration: Duration::hours(24),
        }
    }

    /// Add a step to the workflow
    pub fn add_step(mut self, step: WorkflowStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Set workflow category
    pub fn with_category(mut self, category: String) -> Self {
        self.category = category;
        self
    }

    /// Set SLA duration
    pub fn with_sla(mut self, duration: Duration) -> Self {
        self.sla_duration = duration;
        self
    }

    /// Calculate total estimated duration
    pub fn total_estimated_duration(&self) -> Duration {
        self.steps
            .iter()
            .map(|step| step.expected_duration)
            .sum()
    }

    /// Check if workflow meets SLA for given start time
    pub fn meets_sla(&self, start_time: DateTime<Utc>, current_time: DateTime<Utc>) -> bool {
        let elapsed = current_time - start_time;
        elapsed <= self.sla_duration
    }
}

// =============================================================================
// Step 4: Workflow Factory/Builder
// =============================================================================

/// Factory for creating common workflow configurations
pub struct MyWorkflowFactory;

impl MyWorkflowFactory {
    /// Create a standard workflow with predefined steps
    pub fn create_standard() -> MyWorkflow {
        let workflow_id = WorkflowId::new();

        MyWorkflow::new(workflow_id, "Standard Process".to_string())
            .add_step(WorkflowStep {
                step_index: 0,
                required_resource_type: "preprocessing".to_string(),
                expected_duration: Duration::minutes(10),
                metadata: HashMap::from([
                    ("description".to_string(), "Initial preprocessing".to_string()),
                ]),
            })
            .add_step(WorkflowStep {
                step_index: 1,
                required_resource_type: "main_processing".to_string(),
                expected_duration: Duration::minutes(30),
                metadata: HashMap::from([
                    ("description".to_string(), "Main processing step".to_string()),
                    ("quality_check".to_string(), "true".to_string()),
                ]),
            })
            .add_step(WorkflowStep {
                step_index: 2,
                required_resource_type: "postprocessing".to_string(),
                expected_duration: Duration::minutes(5),
                metadata: HashMap::from([
                    ("description".to_string(), "Final postprocessing".to_string()),
                ]),
            })
            .with_sla(Duration::hours(2))
    }

    /// Create an expedited workflow with shorter durations
    pub fn create_expedited() -> MyWorkflow {
        let workflow_id = WorkflowId::new();

        MyWorkflow::new(workflow_id, "Expedited Process".to_string())
            .add_step(WorkflowStep {
                step_index: 0,
                required_resource_type: "express_processing".to_string(),
                expected_duration: Duration::minutes(5),
                metadata: HashMap::new(),
            })
            .add_step(WorkflowStep {
                step_index: 1,
                required_resource_type: "express_packaging".to_string(),
                expected_duration: Duration::minutes(2),
                metadata: HashMap::new(),
            })
            .with_category("expedited".to_string())
            .with_sla(Duration::minutes(30))
    }
}

// =============================================================================
// Step 5: Define Task Type for Workflow
// =============================================================================

/// Tasks created from workflow steps
#[derive(Debug, Clone)]
pub struct MyWorkflowTask {
    id: TaskId,
    workflow_instance_id: Option<WorkflowInstanceId>,
    step_index: usize,
    resource_type: String,
    duration: Duration,
    /// Domain-specific task data
    metadata: HashMap<String, String>,
}

impl Task for MyWorkflowTask {
    fn id(&self) -> TaskId {
        self.id
    }

    fn required_resource_type(&self) -> &str {
        &self.resource_type
    }

    fn estimated_duration(&self) -> Duration {
        self.duration
    }

    fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
        self.workflow_instance_id
    }
}

// =============================================================================
// Step 6: Implement TaskFactory
// =============================================================================

/// Factory for creating tasks from workflow steps
pub struct MyWorkflowTaskFactory {
    /// Example: Shared context for task creation
    workflow_context: HashMap<WorkflowInstanceId, WorkflowContext>,
}

/// Domain-specific context for a workflow instance
#[derive(Debug, Clone)]
pub struct WorkflowContext {
    pub priority: i32,
    pub customer_id: String,
    pub metadata: HashMap<String, String>,
}

impl MyWorkflowTaskFactory {
    pub fn new() -> Self {
        Self {
            workflow_context: HashMap::new(),
        }
    }

    /// Register context for a workflow instance
    pub fn register_context(&mut self, instance_id: WorkflowInstanceId, context: WorkflowContext) {
        self.workflow_context.insert(instance_id, context);
    }

    /// Get context for an instance
    pub fn get_context(&self, instance_id: WorkflowInstanceId) -> Option<&WorkflowContext> {
        self.workflow_context.get(&instance_id)
    }
}

impl TaskFactory<MyWorkflowTask> for MyWorkflowTaskFactory {
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        current_time: DateTime<Utc>,
    ) -> MyWorkflowTask {
        // Get workflow-specific context
        let context = self.get_context(instance_id);

        // Build task metadata from step and context
        let mut metadata = step.metadata.clone();
        metadata.insert("created_at".to_string(), current_time.to_rfc3339());

        if let Some(ctx) = context {
            metadata.insert("customer_id".to_string(), ctx.customer_id.clone());
            metadata.insert("priority".to_string(), ctx.priority.to_string());
        }

        // Create task
        MyWorkflowTask {
            id: TaskId::new(),
            workflow_instance_id: Some(instance_id),
            step_index: step.step_index,
            resource_type: step.required_resource_type.clone(),
            duration: step.expected_duration,
            metadata,
        }
    }
}

// =============================================================================
// Step 7: Workflow Integration Example
// =============================================================================

#[allow(dead_code)]
fn example_workflow_usage() -> sim_core::Result<()> {
    use sim_core::{WorkflowEngine, SimulationRunner, SimulationConfig};

    // Create workflow
    let workflow = MyWorkflowFactory::create_standard();
    let workflow_id = workflow.id();

    // Create workflow engine
    let mut engine = WorkflowEngine::new();
    engine.register_workflow(workflow);

    // Start workflow instance
    let instance = engine.start_instance(workflow_id)?;
    println!("Started workflow instance: {:?}", instance.id);

    // Create task factory with context
    let mut task_factory = MyWorkflowTaskFactory::new();
    task_factory.register_context(
        instance.id,
        WorkflowContext {
            priority: 5,
            customer_id: "customer-123".to_string(),
            metadata: HashMap::from([
                ("order_id".to_string(), "order-456".to_string()),
            ]),
        },
    );

    // Register with simulation runner
    let config = SimulationConfig::default();
    let mut runner = SimulationRunner::new(config);
    let mut registry = WorkflowRegistry::new();
    registry.register("orders".to_string(), engine)?;
    runner.set_workflow_registry(registry);
    runner.set_task_factory(Box::new(task_factory));

    // The runner will automatically:
    // 1. Detect ready workflow instances
    // 2. Call task_factory.create_task() for current step
    // 3. Add tasks to resource allocator
    // 4. Advance workflow when tasks complete

    Ok(())
}

// =============================================================================
// Step 8: Workflow Event Handling
// =============================================================================

use sim_events::{Event, EventPayload};
use serde::Serialize;

/// Event emitted when workflow step starts
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowStepStartedEvent {
    pub workflow_id: String,
    pub instance_id: String,
    pub step_index: usize,
    pub resource_type: String,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for WorkflowStepStartedEvent {
    fn event_type(&self) -> &str {
        "io.workflow.step.started"
    }

    fn source(&self) -> String {
        format!("/workflows/{}/instances/{}", self.workflow_id, self.instance_id)
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}

/// Event emitted when workflow completes
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowCompletedEvent {
    pub workflow_id: String,
    pub instance_id: String,
    pub duration_seconds: f64,
    pub met_sla: bool,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for WorkflowCompletedEvent {
    fn event_type(&self) -> &str {
        "io.workflow.completed"
    }

    fn source(&self) -> String {
        format!("/workflows/{}/instances/{}", self.workflow_id, self.instance_id)
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}

// =============================================================================
// Step 9: Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_creation() {
        let workflow = MyWorkflowFactory::create_standard();

        assert_eq!(workflow.workflow_type(), "my_workflow");
        assert_eq!(workflow.step_count(), 3);
        assert_eq!(workflow.total_estimated_duration(), Duration::minutes(45));
    }

    #[test]
    fn test_workflow_steps() {
        let workflow = MyWorkflowFactory::create_standard();

        let step0 = workflow.get_step(0).unwrap();
        assert_eq!(step0.step_index, 0);
        assert_eq!(step0.required_resource_type, "preprocessing");

        let step2 = workflow.get_step(2).unwrap();
        assert_eq!(step2.step_index, 2);
        assert_eq!(step2.required_resource_type, "postprocessing");
    }

    #[test]
    fn test_sla_check() {
        let workflow = MyWorkflowFactory::create_standard();

        let start_time = Utc::now();
        let within_sla = start_time + Duration::hours(1);
        let beyond_sla = start_time + Duration::hours(3);

        assert!(workflow.meets_sla(start_time, within_sla));
        assert!(!workflow.meets_sla(start_time, beyond_sla));
    }

    #[test]
    fn test_task_factory() {
        let mut factory = MyWorkflowTaskFactory::new();
        let instance_id = WorkflowInstanceId::new();

        factory.register_context(
            instance_id,
            WorkflowContext {
                priority: 10,
                customer_id: "test-customer".to_string(),
                metadata: HashMap::new(),
            },
        );

        let step = WorkflowStep {
            step_index: 0,
            required_resource_type: "test".to_string(),
            expected_duration: Duration::minutes(10),
            metadata: HashMap::new(),
        };

        let task = factory.create_task(instance_id, &step, Utc::now());

        assert_eq!(task.workflow_instance_id, Some(instance_id));
        assert_eq!(task.resource_type, "test");
        assert!(task.metadata.contains_key("customer_id"));
    }
}
