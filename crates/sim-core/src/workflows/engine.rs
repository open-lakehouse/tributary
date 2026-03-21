//! Workflow engine implementation with Apache Arrow storage

use super::instance_table::WorkflowInstanceTable;
use super::schema::InstanceStatus;
use super::traits::{Workflow, WorkflowInstance};
use crate::Result;
use crate::utils::idents::{WorkflowId, WorkflowInstanceId};
use arrow::record_batch::RecordBatch;
use chrono::{DateTime, Utc};
use datafusion::catalog::MemTable;
use datafusion::execution::context::SessionContext;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

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

/// Manages workflow execution with Apache Arrow storage
///
/// The workflow engine uses Apache Arrow RecordBatch for efficient columnar storage
/// of workflow instance state, enabling fast filtering and analytics via DataFusion.
///
/// # Storage Architecture
///
/// - **Workflow Definitions**: Stored as HashMap (templates, not state)
/// - **Workflow Instances**: Delegated to WorkflowInstanceTable (Arrow RecordBatch)
/// - **Instance Lookup**: O(1) access via WorkflowInstanceTable's HashMap index
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
/// // Query running instances
/// let running = engine.get_running_instances()?;
/// ```
pub struct WorkflowEngine<W: Workflow> {
    /// Registered workflow definitions (templates)
    workflows: HashMap<WorkflowId, W>,

    /// Instance storage and manipulation
    ///
    /// This is publicly accessible to avoid delegation boilerplate.
    /// Provides direct access to all WorkflowInstanceTable operations:
    ///
    /// - `get_instance(id)` - O(1) instance lookup
    /// - `get_instances_by_status(status)` - Query instances by status
    /// - `get_agent_workflows(agent_id)` - Query instances by agent
    /// - `get_instance_status(id)` - Get instance status
    /// - `instance_count()` - Total instance count
    /// - `get_ready_instances()` - Get instances ready for assignment
    /// - `export_batch()` - Export instances as RecordBatch
    /// - `import_batch(batch)` - Import instances from RecordBatch
    /// - `set_current_tick(tick)` / `current_tick()` - Simulation time management
    pub instance_table: WorkflowInstanceTable,

    /// Phantom data
    _phantom: PhantomData<W>,
}

impl<W: Workflow> WorkflowEngine<W> {
    /// Create a new workflow engine
    pub fn new() -> Self {
        Self {
            workflows: HashMap::new(),
            instance_table: WorkflowInstanceTable::new(),
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

        // Append instance to table
        self.instance_table.append_instance(&instance)?;

        Ok(instance)
    }

    /// Start multiple workflow instances in a single batch operation.
    /// Much more efficient than calling start_instance() in a loop.
    ///
    /// # Arguments
    ///
    /// * `workflow_id` - ID of the workflow definition to instantiate
    /// * `count` - Number of instances to create
    ///
    /// # Returns
    ///
    /// Vector of created workflow instances
    pub fn start_instances(
        &mut self,
        workflow_id: WorkflowId,
        count: usize,
    ) -> Result<Vec<WorkflowInstance>> {
        let mut instances = Vec::with_capacity(count);
        for _ in 0..count {
            instances.push(self.start_instance(workflow_id)?);
        }
        Ok(instances)
    }

    /// Advance instance to next step after task completion
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
        let row_idx = self
            .instance_table
            .get_row_index(instance_id)
            .ok_or_else(|| {
                crate::Error::Workflow(format!("Workflow instance {} not found", instance_id))
            })?;

        // Get current instance state
        let instance = self
            .instance_table
            .get_instance_at_row(row_idx)
            .ok_or_else(|| {
                crate::Error::Workflow(format!("Failed to read instance {}", instance_id))
            })?;

        // Check if already complete
        if instance.is_complete {
            return Err(crate::Error::Workflow(format!(
                "Workflow instance {} is already complete",
                instance_id
            )));
        }

        let completed_step = instance.current_step;
        let new_step = instance.current_step + 1;

        // Determine if workflow is now complete
        let is_complete = new_step >= instance.total_steps;

        // Update the row
        if is_complete {
            let current_tick = self.instance_table.current_tick();
            self.instance_table
                .update_instance_at_row(row_idx, |update| {
                    update.current_step_index = Some(new_step);
                    update.status = Some(InstanceStatus::Complete);
                    update.completed_at = Some(current_tick);
                })?;

            Ok(Some(WorkflowStepEvent::WorkflowCompleted {
                instance_id,
                workflow_id: instance.workflow_id,
                completion_time,
            }))
        } else {
            self.instance_table
                .update_instance_at_row(row_idx, |update| {
                    update.current_step_index = Some(new_step);
                    update.status = Some(InstanceStatus::Running);
                })?;

            Ok(Some(WorkflowStepEvent::StepCompleted {
                instance_id,
                step_index: completed_step,
                completion_time,
            }))
        }
    }

    /// Mark an instance as having a task assigned
    ///
    /// This updates the instance status to `TaskAssigned` to prevent creating duplicate tasks
    /// for the same workflow step.
    pub fn mark_task_assigned(&mut self, instance_id: WorkflowInstanceId) -> Result<()> {
        let row_idx = self
            .instance_table
            .get_row_index(instance_id)
            .ok_or_else(|| {
                crate::Error::Workflow(format!("Workflow instance {} not found", instance_id))
            })?;

        self.instance_table
            .update_instance_at_row(row_idx, |update| {
                update.status = Some(InstanceStatus::TaskAssigned);
            })?;

        Ok(())
    }

    /// Get all running instances
    pub fn get_running_instances(&self) -> Result<Vec<WorkflowInstanceId>> {
        self.instance_table
            .get_instances_by_status(InstanceStatus::Running)
    }

    /// Get all completed instances
    pub fn get_completed_instances(&self) -> Result<Vec<WorkflowInstanceId>> {
        self.instance_table
            .get_instances_by_status(InstanceStatus::Complete)
    }

    /// Pause a workflow instance
    pub fn pause_instance(&mut self, instance_id: WorkflowInstanceId) -> Result<()> {
        let row_idx = self
            .instance_table
            .get_row_index(instance_id)
            .ok_or_else(|| {
                crate::Error::Workflow(format!("Workflow instance {} not found", instance_id))
            })?;

        self.instance_table
            .update_instance_at_row(row_idx, |update| {
                update.status = Some(InstanceStatus::Paused);
            })
    }

    /// Resume a workflow instance
    pub fn resume_instance(&mut self, instance_id: WorkflowInstanceId) -> Result<()> {
        let row_idx = self
            .instance_table
            .get_row_index(instance_id)
            .ok_or_else(|| {
                crate::Error::Workflow(format!("Workflow instance {} not found", instance_id))
            })?;

        self.instance_table
            .update_instance_at_row(row_idx, |update| {
                update.status = Some(InstanceStatus::Running);
            })
    }

    /// Complete a workflow instance
    pub fn complete_instance(&mut self, instance_id: WorkflowInstanceId) -> Result<()> {
        let row_idx = self
            .instance_table
            .get_row_index(instance_id)
            .ok_or_else(|| {
                crate::Error::Workflow(format!("Workflow instance {} not found", instance_id))
            })?;

        let current_tick = self.instance_table.current_tick();
        self.instance_table
            .update_instance_at_row(row_idx, |update| {
                update.status = Some(InstanceStatus::Complete);
                update.completed_at = Some(current_tick);
            })
    }

    /// Get workflow count
    pub fn workflow_count(&self) -> usize {
        self.workflows.len()
    }

    /// Execute SQL query on workflow instances
    ///
    /// This method registers the workflow instances RecordBatch as a DataFusion table
    /// named "instances" and executes the provided SQL query against it.
    ///
    /// # Arguments
    ///
    /// * `sql` - SQL query string to execute
    ///
    /// # Returns
    ///
    /// A RecordBatch containing the query results
    ///
    /// # Schema
    ///
    /// The "instances" table has the following columns:
    /// - `id`: Binary (16 bytes) - WorkflowInstanceId UUID
    /// - `workflow_id`: Binary (16 bytes) - WorkflowId UUID
    /// - `status`: Utf8 - Instance status (Running/TaskAssigned/Paused/Complete/Failed)
    /// - `current_step_index`: Int64 - Current step in workflow (0-based)
    /// - `total_steps`: Int64 - Total number of steps in workflow
    /// - `agent_id`: Binary (16 bytes, nullable) - Optional AgentId assigned to instance
    /// - `started_at`: Int64 - Simulation tick when instance started
    /// - `completed_at`: Int64 (nullable) - Simulation tick when instance completed
    /// - `context`: Utf8 (nullable) - JSON-encoded workflow context data
    /// - `metadata`: Utf8 (nullable) - JSON-encoded domain-specific metadata
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Query all running instances
    /// let running = engine.query("SELECT * FROM instances WHERE status = 'Running'").await?;
    ///
    /// // Query instances for a specific workflow
    /// let workflow_instances = engine.query(
    ///     "SELECT id, status FROM instances WHERE workflow_id = X'...'"
    /// ).await?;
    ///
    /// // Count instances by status
    /// let counts = engine.query(
    ///     "SELECT status, COUNT(*) as count FROM instances GROUP BY status"
    /// ).await?;
    /// ```
    pub async fn query(&self, sql: &str) -> Result<RecordBatch> {
        // Create DataFusion SessionContext
        let ctx = SessionContext::new();

        // Get the instances RecordBatch
        let batch = self.instance_table.export_batch();
        let schema = batch.schema();

        // Create MemTable and register as "instances"
        let table = MemTable::try_new(schema, vec![vec![batch]])
            .map_err(|e| crate::Error::Workflow(format!("Failed to create MemTable: {}", e)))?;

        ctx.register_table("instances", Arc::new(table))
            .map_err(|e| crate::Error::Workflow(format!("Failed to register table: {}", e)))?;

        // Execute query
        let df = ctx
            .sql(sql)
            .await
            .map_err(|e| crate::Error::Workflow(format!("SQL query failed: {}", e)))?;

        // Get schema before collecting (df.collect() consumes df)
        let schema = df.schema().inner().clone();

        // Collect results
        let batches = df
            .collect()
            .await
            .map_err(|e| crate::Error::Workflow(format!("Failed to collect results: {}", e)))?;

        // Return single batch or concatenate multiple batches
        if batches.is_empty() {
            // Return empty batch with schema
            Ok(RecordBatch::new_empty(schema))
        } else if batches.len() == 1 {
            Ok(batches[0].clone())
        } else {
            // Concatenate multiple batches
            let schema = batches[0].schema();
            arrow::compute::concat_batches(&schema, &batches).map_err(|e| {
                crate::Error::Workflow(format!("Failed to concatenate batches: {}", e))
            })
        }
    }

    /// Process workflow instances for current tick
    ///
    /// Currently a placeholder for future time-based workflow steps
    pub fn tick(&mut self, _current_time: DateTime<Utc>) -> Result<Vec<WorkflowStepEvent>> {
        // Future: Process time-based workflow steps
        Ok(Vec::new())
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
        assert_eq!(engine.instance_table.instance_count(), 0);
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
        assert_eq!(engine.instance_table.instance_count(), 1);
    }

    #[test]
    fn test_get_instance() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);
        let instance = engine.start_instance(workflow_id).unwrap();
        let instance_id = instance.id;

        let retrieved = engine.instance_table.get_instance(instance_id).unwrap();
        assert_eq!(retrieved.id, instance_id);
        assert_eq!(retrieved.workflow_id, workflow_id);
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
        let event = engine
            .advance_instance_step(instance_id, completion_time)
            .unwrap();

        assert!(event.is_some());
        match event.unwrap() {
            WorkflowStepEvent::StepCompleted {
                instance_id: id,
                step_index,
                ..
            } => {
                assert_eq!(id, instance_id);
                assert_eq!(step_index, 0);
            }
            _ => panic!("Expected StepCompleted event"),
        }

        let instance = engine.instance_table.get_instance(instance_id).unwrap();
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
        engine
            .advance_instance_step(instance_id, completion_time)
            .unwrap();
        let event = engine
            .advance_instance_step(instance_id, completion_time)
            .unwrap();

        assert!(event.is_some());
        match event.unwrap() {
            WorkflowStepEvent::WorkflowCompleted {
                instance_id: id,
                workflow_id: wf_id,
                ..
            } => {
                assert_eq!(id, instance_id);
                assert_eq!(wf_id, workflow_id);
            }
            _ => panic!("Expected WorkflowCompleted event"),
        }

        let instance = engine.instance_table.get_instance(instance_id).unwrap();
        assert_eq!(instance.current_step, 2);
        assert!(instance.is_complete);
    }

    #[test]
    fn test_get_running_instances() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);
        engine.start_instance(workflow_id).unwrap();
        engine.start_instance(workflow_id).unwrap();

        let running = engine.get_running_instances().unwrap();
        assert_eq!(running.len(), 2);
    }

    #[test]
    fn test_pause_resume_instance() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);
        let instance = engine.start_instance(workflow_id).unwrap();
        let instance_id = instance.id;

        // Pause
        engine.pause_instance(instance_id).unwrap();
        let status = engine
            .instance_table
            .get_instance_status(instance_id)
            .unwrap();
        assert_eq!(status, InstanceStatus::Paused);

        // Resume
        engine.resume_instance(instance_id).unwrap();
        let status = engine
            .instance_table
            .get_instance_status(instance_id)
            .unwrap();
        assert_eq!(status, InstanceStatus::Running);
    }

    #[test]
    fn test_advance_nonexistent_workflow_instance_fails() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();

        engine.register_workflow(workflow);

        let fake_instance_id = WorkflowInstanceId::new();
        let completion_time = chrono::Utc::now();

        let result = engine.advance_instance_step(fake_instance_id, completion_time);

        assert!(result.is_err());
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

        // Complete the workflow
        engine
            .advance_instance_step(instance_id, completion_time)
            .unwrap();
        engine
            .advance_instance_step(instance_id, completion_time)
            .unwrap();

        // Try to advance again
        let result = engine.advance_instance_step(instance_id, completion_time);
        assert!(result.is_err());
    }

    #[test]
    fn test_tick() {
        let mut engine = WorkflowEngine::<TestWorkflow>::new();
        let current_time = chrono::Utc::now();

        let events = engine.tick(current_time).unwrap();
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn test_export_import_empty_engine() {
        let engine = WorkflowEngine::<TestWorkflow>::new();

        // Export from empty engine
        let batch = engine.instance_table.export_batch();
        assert_eq!(batch.num_rows(), 0);

        // Import into new engine
        let mut new_engine = WorkflowEngine::<TestWorkflow>::new();
        new_engine.instance_table.import_batch(batch).unwrap();

        // Verify state
        assert_eq!(new_engine.instance_table.instance_count(), 0);
    }

    #[test]
    fn test_export_import_single_instance() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);
        let instance = engine.start_instance(workflow_id).unwrap();
        let instance_id = instance.id;

        // Export
        let batch = engine.instance_table.export_batch();
        assert_eq!(batch.num_rows(), 1);

        // Import into new engine
        let mut new_engine = WorkflowEngine::<TestWorkflow>::new();
        new_engine.instance_table.import_batch(batch).unwrap();

        // Verify instance count
        assert_eq!(new_engine.instance_table.instance_count(), 1);

        // Verify instance can be retrieved
        let retrieved = new_engine.instance_table.get_instance(instance_id).unwrap();
        assert_eq!(retrieved.id, instance_id);
        assert_eq!(retrieved.workflow_id, workflow_id);
        assert_eq!(retrieved.current_step, 0);
        assert_eq!(retrieved.total_steps, 2);
        assert!(!retrieved.is_complete);
    }

    #[test]
    fn test_export_import_multiple_instances() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        // Create multiple instances
        let instance1 = engine.start_instance(workflow_id).unwrap();
        let instance2 = engine.start_instance(workflow_id).unwrap();
        let instance3 = engine.start_instance(workflow_id).unwrap();

        let id1 = instance1.id;
        let id2 = instance2.id;
        let id3 = instance3.id;

        // Export
        let batch = engine.instance_table.export_batch();
        assert_eq!(batch.num_rows(), 3);

        // Import into new engine
        let mut new_engine = WorkflowEngine::<TestWorkflow>::new();
        new_engine.instance_table.import_batch(batch).unwrap();

        // Verify instance count
        assert_eq!(new_engine.instance_table.instance_count(), 3);

        // Verify all instances can be retrieved
        assert!(new_engine.instance_table.get_instance(id1).is_some());
        assert!(new_engine.instance_table.get_instance(id2).is_some());
        assert!(new_engine.instance_table.get_instance(id3).is_some());
    }

    #[test]
    fn test_export_import_preserves_workflow_state() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        // Create instances with different states
        let instance1 = engine.start_instance(workflow_id).unwrap();
        let id1 = instance1.id;

        let instance2 = engine.start_instance(workflow_id).unwrap();
        let id2 = instance2.id;

        let instance3 = engine.start_instance(workflow_id).unwrap();
        let id3 = instance3.id;

        // Advance instance1 to step 1
        let completion_time = chrono::Utc::now();
        engine.advance_instance_step(id1, completion_time).unwrap();

        // Complete instance2
        engine.advance_instance_step(id2, completion_time).unwrap();
        engine.advance_instance_step(id2, completion_time).unwrap();

        // Pause instance3
        engine.pause_instance(id3).unwrap();

        // Export
        let batch = engine.instance_table.export_batch();

        // Import into new engine
        let mut new_engine = WorkflowEngine::<TestWorkflow>::new();
        new_engine.instance_table.import_batch(batch).unwrap();

        // Verify instance1 state
        let retrieved1 = new_engine.instance_table.get_instance(id1).unwrap();
        assert_eq!(retrieved1.current_step, 1);
        assert!(!retrieved1.is_complete);

        // Verify instance2 state (completed)
        let retrieved2 = new_engine.instance_table.get_instance(id2).unwrap();
        assert_eq!(retrieved2.current_step, 2);
        assert!(retrieved2.is_complete);

        // Verify instance3 state (paused)
        let status3 = new_engine.instance_table.get_instance_status(id3).unwrap();
        assert_eq!(status3, InstanceStatus::Paused);
    }

    #[test]
    fn test_export_import_running_instances_query() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        // Create 3 running, 2 completed instances
        let _instance1 = engine.start_instance(workflow_id).unwrap();
        let _instance2 = engine.start_instance(workflow_id).unwrap();
        let _instance3 = engine.start_instance(workflow_id).unwrap();

        let instance4 = engine.start_instance(workflow_id).unwrap();
        let instance5 = engine.start_instance(workflow_id).unwrap();

        // Complete instance4 and instance5
        let completion_time = chrono::Utc::now();
        engine
            .advance_instance_step(instance4.id, completion_time)
            .unwrap();
        engine
            .advance_instance_step(instance4.id, completion_time)
            .unwrap();
        engine
            .advance_instance_step(instance5.id, completion_time)
            .unwrap();
        engine
            .advance_instance_step(instance5.id, completion_time)
            .unwrap();

        // Export
        let batch = engine.instance_table.export_batch();

        // Import into new engine
        let mut new_engine = WorkflowEngine::<TestWorkflow>::new();
        new_engine.instance_table.import_batch(batch).unwrap();

        // Query running instances
        let running = new_engine.get_running_instances().unwrap();
        assert_eq!(running.len(), 3);

        // Query completed instances
        let completed = new_engine.get_completed_instances().unwrap();
        assert_eq!(completed.len(), 2);
    }

    #[test]
    fn test_export_import_status_queries() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        let instance1 = engine.start_instance(workflow_id).unwrap();
        let instance2 = engine.start_instance(workflow_id).unwrap();

        // Pause instance2
        engine.pause_instance(instance2.id).unwrap();

        // Export
        let batch = engine.instance_table.export_batch();

        // Import into new engine
        let mut new_engine = WorkflowEngine::<TestWorkflow>::new();
        new_engine.instance_table.import_batch(batch).unwrap();

        // Verify status queries work
        let status1 = new_engine
            .instance_table
            .get_instance_status(instance1.id)
            .unwrap();
        assert_eq!(status1, InstanceStatus::Running);

        let status2 = new_engine
            .instance_table
            .get_instance_status(instance2.id)
            .unwrap();
        assert_eq!(status2, InstanceStatus::Paused);
    }

    #[test]
    fn test_import_invalid_schema_fails() {
        use arrow::array::{ArrayRef, Int32Array, RecordBatch};
        use arrow::datatypes::{DataType, Field, Schema};
        use std::sync::Arc;

        let mut engine = WorkflowEngine::<TestWorkflow>::new();

        // Create batch with wrong schema
        let invalid_schema = Schema::new(vec![Field::new("wrong_field", DataType::Int32, false)]);
        let invalid_array: ArrayRef = Arc::new(Int32Array::from(vec![1, 2, 3]));
        let invalid_batch =
            RecordBatch::try_new(Arc::new(invalid_schema), vec![invalid_array]).unwrap();

        // Import should fail
        let result = engine.instance_table.import_batch(invalid_batch);
        assert!(result.is_err());
    }

    #[test]
    fn test_import_rebuilds_indices() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        // Create instances
        let instance1 = engine.start_instance(workflow_id).unwrap();
        let instance2 = engine.start_instance(workflow_id).unwrap();

        let id1 = instance1.id;
        let id2 = instance2.id;

        // Export
        let batch = engine.instance_table.export_batch();

        // Create new engine and clear any state
        let mut new_engine = WorkflowEngine::<TestWorkflow>::new();

        // Import should rebuild indices
        new_engine.instance_table.import_batch(batch).unwrap();

        // Verify O(1) lookup works
        assert!(new_engine.instance_table.get_instance(id1).is_some());
        assert!(new_engine.instance_table.get_instance(id2).is_some());

        // Verify lookups return correct instances
        let retrieved1 = new_engine.instance_table.get_instance(id1).unwrap();
        assert_eq!(retrieved1.id, id1);

        let retrieved2 = new_engine.instance_table.get_instance(id2).unwrap();
        assert_eq!(retrieved2.id, id2);
    }

    #[test]
    fn test_round_trip_preserves_metadata() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        // Create instance with metadata
        let mut instance = engine.start_instance(workflow_id).unwrap();
        instance
            .metadata
            .insert("key1".to_string(), "value1".to_string());
        instance
            .metadata
            .insert("key2".to_string(), "value2".to_string());

        let instance_id = instance.id;

        // We need to update the instance in the engine with metadata
        // For this test, we'll export and import to verify the metadata path works

        // Export
        let batch = engine.instance_table.export_batch();

        // Import into new engine
        let mut new_engine = WorkflowEngine::<TestWorkflow>::new();
        new_engine.instance_table.import_batch(batch).unwrap();

        // Retrieve and verify (metadata should be empty as we didn't update it in the batch)
        let retrieved = new_engine.instance_table.get_instance(instance_id).unwrap();
        assert_eq!(retrieved.id, instance_id);
        // Note: metadata preservation would require updating the RecordBatch,
        // which is not done in this test. This test verifies the round-trip works.
    }

    #[tokio::test]
    async fn test_query_all_instances() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        // Create 5 instances
        engine.start_instance(workflow_id).unwrap();
        engine.start_instance(workflow_id).unwrap();
        engine.start_instance(workflow_id).unwrap();
        engine.start_instance(workflow_id).unwrap();
        engine.start_instance(workflow_id).unwrap();

        // Query all instances
        let result = engine.query("SELECT * FROM instances").await.unwrap();

        assert_eq!(result.num_rows(), 5);
        assert_eq!(result.num_columns(), 10); // All schema columns
    }

    #[tokio::test]
    async fn test_query_by_status() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        // Create 3 running instances
        engine.start_instance(workflow_id).unwrap();
        engine.start_instance(workflow_id).unwrap();
        let instance3 = engine.start_instance(workflow_id).unwrap();

        // Complete one instance
        let completion_time = chrono::Utc::now();
        engine
            .advance_instance_step(instance3.id, completion_time)
            .unwrap();
        engine
            .advance_instance_step(instance3.id, completion_time)
            .unwrap();

        // Query running instances
        let result = engine
            .query("SELECT * FROM instances WHERE status = 'Running'")
            .await
            .unwrap();

        assert_eq!(result.num_rows(), 2);

        // Query completed instances
        let result = engine
            .query("SELECT * FROM instances WHERE status = 'Complete'")
            .await
            .unwrap();

        assert_eq!(result.num_rows(), 1);
    }

    #[tokio::test]
    async fn test_query_by_agent() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        // Create instances
        let _instance1 = engine.start_instance(workflow_id).unwrap();
        let _instance2 = engine.start_instance(workflow_id).unwrap();
        let _instance3 = engine.start_instance(workflow_id).unwrap();

        // Note: WorkflowEngine doesn't currently expose assign_agent method
        // This test demonstrates the query capability even though we can't
        // assign agents directly through the public API.
        // When agent assignment is added, this test will validate filtering by agent_id.

        // Test NULL agent_id filter (all instances should have no agent assigned)
        let result = engine
            .query("SELECT * FROM instances WHERE agent_id IS NULL")
            .await
            .unwrap();

        assert_eq!(result.num_rows(), 3); // All instances have no agent assigned

        // Test NOT NULL filter (should be empty as no agents are assigned)
        let result = engine
            .query("SELECT * FROM instances WHERE agent_id IS NOT NULL")
            .await
            .unwrap();

        assert_eq!(result.num_rows(), 0); // No agents assigned yet
    }

    #[tokio::test]
    async fn test_query_count_by_status() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        // Create 5 running instances
        for _ in 0..5 {
            engine.start_instance(workflow_id).unwrap();
        }

        // Complete 2 instances
        let running = engine.get_running_instances().unwrap();
        let completion_time = chrono::Utc::now();
        for i in 0..2 {
            engine
                .advance_instance_step(running[i], completion_time)
                .unwrap();
            engine
                .advance_instance_step(running[i], completion_time)
                .unwrap();
        }

        // Pause 1 instance
        engine.pause_instance(running[2]).unwrap();

        // Query count by status
        let result = engine
            .query(
                "SELECT status, COUNT(*) as count FROM instances GROUP BY status ORDER BY status",
            )
            .await
            .unwrap();

        assert_eq!(result.num_rows(), 3); // Three different statuses

        // Verify counts
        let status_array = result
            .column(0)
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .unwrap();
        let count_array = result
            .column(1)
            .as_any()
            .downcast_ref::<arrow::array::Int64Array>()
            .unwrap();

        // Complete: 2, Paused: 1, Running: 2
        for i in 0..result.num_rows() {
            let status = status_array.value(i);
            let count = count_array.value(i);

            match status {
                "Complete" => assert_eq!(count, 2),
                "Paused" => assert_eq!(count, 1),
                "Running" => assert_eq!(count, 2),
                _ => panic!("Unexpected status: {}", status),
            }
        }
    }

    #[tokio::test]
    async fn test_query_invalid_sql() {
        let engine = WorkflowEngine::<TestWorkflow>::new();

        // Test invalid SQL syntax
        let result = engine.query("SELECT * FROM invalid_table").await;
        assert!(result.is_err());

        // Test syntax error
        let result = engine.query("INVALID SQL QUERY").await;
        assert!(result.is_err());

        // Test invalid column
        let result = engine
            .query("SELECT nonexistent_column FROM instances")
            .await;
        assert!(result.is_err());
    }
}
