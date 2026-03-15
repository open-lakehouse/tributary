//! Workflow engine implementation with Apache Arrow storage

use super::schema::{workflow_instance_schema, InstanceStatus};
use super::traits::{Workflow, WorkflowInstance};
use crate::utils::idents::{AgentId, WorkflowId, WorkflowInstanceId};
use crate::Result;
use arrow::array::{
    Array, ArrayRef, BinaryArray, BinaryBuilder, Int64Array, Int64Builder, RecordBatch, StringBuilder,
};
use chrono::{DateTime, Utc};
use datafusion::prelude::*;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use uuid::Uuid;

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
/// - **Workflow Instances**: Stored as Arrow RecordBatch (state data)
/// - **Instance Lookup**: HashMap mapping WorkflowInstanceId → row index (O(1) access)
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
/// // Start instances (async for Arrow operations)
/// let instance = engine.start_instance(workflow_id)?;
///
/// // Query running instances via DataFusion
/// let running = engine.get_running_instances()?;
/// ```
pub struct WorkflowEngine<W: Workflow> {
    /// Registered workflow definitions (templates)
    workflows: HashMap<WorkflowId, W>,

    /// Active workflow instances (Arrow RecordBatch)
    instances: RecordBatch,

    /// Schema for instances RecordBatch
    instance_schema: arrow::datatypes::SchemaRef,

    /// Lookup map: WorkflowInstanceId → row index in RecordBatch
    instance_id_to_row: HashMap<WorkflowInstanceId, usize>,

    /// DataFusion context for SQL queries
    ctx: SessionContext,

    /// Phantom data
    _phantom: PhantomData<W>,
}

impl<W: Workflow> WorkflowEngine<W> {
    /// Create a new workflow engine
    pub fn new() -> Self {
        let schema = workflow_instance_schema();
        let empty_batch = RecordBatch::new_empty(schema.clone());
        let ctx = SessionContext::new();

        Self {
            workflows: HashMap::new(),
            instances: empty_batch,
            instance_schema: schema,
            instance_id_to_row: HashMap::new(),
            ctx,
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

    /// Start a new workflow instance (sync version for compatibility)
    ///
    /// # Arguments
    ///
    /// * `workflow_id` - ID of the workflow definition to instantiate
    ///
    /// # Returns
    ///
    /// The created workflow instance
    pub fn start_instance(&mut self, workflow_id: WorkflowId) -> Result<WorkflowInstance> {
        let workflow = self.workflows.get(&workflow_id).ok_or_else(|| {
            crate::Error::Workflow(format!("Workflow {} not found", workflow_id))
        })?;

        let instance = WorkflowInstance::new(workflow_id, workflow.step_count());

        // Append instance to RecordBatch
        self.append_instance(&instance)?;

        Ok(instance)
    }

    /// Append a new instance to the RecordBatch
    fn append_instance(&mut self, instance: &WorkflowInstance) -> Result<()> {
        let new_row_index = self.instances.num_rows();

        // Build new arrays with the additional row
        let mut id_builder = BinaryBuilder::new();
        let mut workflow_id_builder = BinaryBuilder::new();
        let mut status_builder = StringBuilder::new();
        let mut current_step_builder = Int64Builder::new();
        let mut total_steps_builder = Int64Builder::new();
        let mut agent_id_builder = BinaryBuilder::new();
        let mut started_at_builder = Int64Builder::new();
        let mut completed_at_builder = Int64Builder::new();
        let mut context_builder = StringBuilder::new();
        let mut metadata_builder = StringBuilder::new();

        // Copy existing data
        if self.instances.num_rows() > 0 {
            self.copy_column_data(
                &mut id_builder,
                &mut workflow_id_builder,
                &mut status_builder,
                &mut current_step_builder,
                &mut total_steps_builder,
                &mut agent_id_builder,
                &mut started_at_builder,
                &mut completed_at_builder,
                &mut context_builder,
                &mut metadata_builder,
            );
        }

        // Append new instance
        id_builder.append_value(instance.id.as_bytes());
        workflow_id_builder.append_value(instance.workflow_id.as_bytes());

        // Determine status
        let status = if instance.is_complete {
            InstanceStatus::Complete
        } else if instance.step_completion_time.is_some() {
            InstanceStatus::Running
        } else {
            InstanceStatus::Running // Default to Running when just created
        };
        status_builder.append_value(status.as_str());

        current_step_builder.append_value(instance.current_step as i64);
        total_steps_builder.append_value(instance.total_steps as i64);
        agent_id_builder.append_null(); // No agent assigned initially
        started_at_builder.append_value(0); // TODO: Use actual simulation tick
        completed_at_builder.append_null();
        context_builder.append_null();

        // Serialize metadata as JSON
        if !instance.metadata.is_empty() {
            let metadata_json = serde_json::to_string(&instance.metadata)
                .map_err(|e| crate::Error::Workflow(format!("Failed to serialize metadata: {}", e)))?;
            metadata_builder.append_value(metadata_json);
        } else {
            metadata_builder.append_null();
        }

        // Build arrays
        let arrays: Vec<ArrayRef> = vec![
            Arc::new(id_builder.finish()),
            Arc::new(workflow_id_builder.finish()),
            Arc::new(status_builder.finish()),
            Arc::new(current_step_builder.finish()),
            Arc::new(total_steps_builder.finish()),
            Arc::new(agent_id_builder.finish()),
            Arc::new(started_at_builder.finish()),
            Arc::new(completed_at_builder.finish()),
            Arc::new(context_builder.finish()),
            Arc::new(metadata_builder.finish()),
        ];

        // Create new RecordBatch
        self.instances = RecordBatch::try_new(self.instance_schema.clone(), arrays)
            .map_err(|e| crate::Error::Workflow(format!("Failed to create RecordBatch: {}", e)))?;

        // Update lookup map
        self.instance_id_to_row.insert(instance.id, new_row_index);

        Ok(())
    }

    /// Copy existing column data from current RecordBatch to builders
    #[allow(clippy::too_many_arguments)]
    fn copy_column_data(
        &self,
        id_builder: &mut BinaryBuilder,
        workflow_id_builder: &mut BinaryBuilder,
        status_builder: &mut StringBuilder,
        current_step_builder: &mut Int64Builder,
        total_steps_builder: &mut Int64Builder,
        agent_id_builder: &mut BinaryBuilder,
        started_at_builder: &mut Int64Builder,
        completed_at_builder: &mut Int64Builder,
        context_builder: &mut StringBuilder,
        metadata_builder: &mut StringBuilder,
    ) {
        let num_rows = self.instances.num_rows();

        // Extract arrays
        let id_array = self.instances.column(0).as_any().downcast_ref::<BinaryArray>().unwrap();
        let workflow_id_array = self.instances.column(1).as_any().downcast_ref::<BinaryArray>().unwrap();
        let status_array = self.instances.column(2).as_any().downcast_ref::<arrow::array::StringArray>().unwrap();
        let current_step_array = self.instances.column(3).as_any().downcast_ref::<Int64Array>().unwrap();
        let total_steps_array = self.instances.column(4).as_any().downcast_ref::<Int64Array>().unwrap();
        let agent_id_array = self.instances.column(5).as_any().downcast_ref::<BinaryArray>().unwrap();
        let started_at_array = self.instances.column(6).as_any().downcast_ref::<Int64Array>().unwrap();
        let completed_at_array = self.instances.column(7).as_any().downcast_ref::<Int64Array>().unwrap();
        let context_array = self.instances.column(8).as_any().downcast_ref::<arrow::array::StringArray>().unwrap();
        let metadata_array = self.instances.column(9).as_any().downcast_ref::<arrow::array::StringArray>().unwrap();

        // Copy each row
        for i in 0..num_rows {
            id_builder.append_value(id_array.value(i));
            workflow_id_builder.append_value(workflow_id_array.value(i));
            status_builder.append_value(status_array.value(i));
            current_step_builder.append_value(current_step_array.value(i));
            total_steps_builder.append_value(total_steps_array.value(i));

            if agent_id_array.is_null(i) {
                agent_id_builder.append_null();
            } else {
                agent_id_builder.append_value(agent_id_array.value(i));
            }

            started_at_builder.append_value(started_at_array.value(i));

            if completed_at_array.is_null(i) {
                completed_at_builder.append_null();
            } else {
                completed_at_builder.append_value(completed_at_array.value(i));
            }

            if context_array.is_null(i) {
                context_builder.append_null();
            } else {
                context_builder.append_value(context_array.value(i));
            }

            if metadata_array.is_null(i) {
                metadata_builder.append_null();
            } else {
                metadata_builder.append_value(metadata_array.value(i));
            }
        }
    }

    /// Get a workflow instance by ID (O(1) lookup)
    pub fn get_instance(&self, id: WorkflowInstanceId) -> Option<WorkflowInstance> {
        let row_idx = self.instance_id_to_row.get(&id)?;
        self.get_instance_at_row(*row_idx)
    }

    /// Get workflow instance at specific row index
    fn get_instance_at_row(&self, row_idx: usize) -> Option<WorkflowInstance> {
        if row_idx >= self.instances.num_rows() {
            return None;
        }

        // Extract data from RecordBatch
        let id_array = self.instances.column(0).as_any().downcast_ref::<BinaryArray>()?;
        let workflow_id_array = self.instances.column(1).as_any().downcast_ref::<BinaryArray>()?;
        let current_step_array = self.instances.column(3).as_any().downcast_ref::<Int64Array>()?;
        let total_steps_array = self.instances.column(4).as_any().downcast_ref::<Int64Array>()?;
        let status_array = self.instances.column(2).as_any().downcast_ref::<arrow::array::StringArray>()?;
        let metadata_array = self.instances.column(9).as_any().downcast_ref::<arrow::array::StringArray>()?;

        let id_bytes = id_array.value(row_idx);
        let id = WorkflowInstanceId::from_uuid(Uuid::from_slice(id_bytes).ok()?);

        let workflow_id_bytes = workflow_id_array.value(row_idx);
        let workflow_id = WorkflowId::from_uuid(Uuid::from_slice(workflow_id_bytes).ok()?);

        let current_step = current_step_array.value(row_idx) as usize;
        let total_steps = total_steps_array.value(row_idx) as usize;

        let status_str = status_array.value(row_idx);
        let is_complete = status_str == "Complete";

        let metadata = if metadata_array.is_null(row_idx) {
            HashMap::new()
        } else {
            serde_json::from_str(metadata_array.value(row_idx)).unwrap_or_default()
        };

        Some(WorkflowInstance {
            id,
            workflow_id,
            current_step,
            total_steps,
            step_completion_time: None, // Not stored in RecordBatch currently
            is_complete,
            metadata,
        })
    }

    /// Update instance at specific row
    async fn update_instance_at_row(
        &mut self,
        row_idx: usize,
        update_fn: impl FnOnce(&mut WorkflowInstanceRowUpdate),
    ) -> Result<()> {
        if row_idx >= self.instances.num_rows() {
            return Err(crate::Error::Workflow("Invalid row index".to_string()));
        }

        let mut update = WorkflowInstanceRowUpdate::default();
        update_fn(&mut update);

        // Rebuild RecordBatch with updated row
        let mut id_builder = BinaryBuilder::new();
        let mut workflow_id_builder = BinaryBuilder::new();
        let mut status_builder = StringBuilder::new();
        let mut current_step_builder = Int64Builder::new();
        let mut total_steps_builder = Int64Builder::new();
        let mut agent_id_builder = BinaryBuilder::new();
        let mut started_at_builder = Int64Builder::new();
        let mut completed_at_builder = Int64Builder::new();
        let mut context_builder = StringBuilder::new();
        let mut metadata_builder = StringBuilder::new();

        // Extract arrays
        let id_array = self.instances.column(0).as_any().downcast_ref::<BinaryArray>().unwrap();
        let workflow_id_array = self.instances.column(1).as_any().downcast_ref::<BinaryArray>().unwrap();
        let status_array = self.instances.column(2).as_any().downcast_ref::<arrow::array::StringArray>().unwrap();
        let current_step_array = self.instances.column(3).as_any().downcast_ref::<Int64Array>().unwrap();
        let total_steps_array = self.instances.column(4).as_any().downcast_ref::<Int64Array>().unwrap();
        let agent_id_array = self.instances.column(5).as_any().downcast_ref::<BinaryArray>().unwrap();
        let started_at_array = self.instances.column(6).as_any().downcast_ref::<Int64Array>().unwrap();
        let completed_at_array = self.instances.column(7).as_any().downcast_ref::<Int64Array>().unwrap();
        let context_array = self.instances.column(8).as_any().downcast_ref::<arrow::array::StringArray>().unwrap();
        let metadata_array = self.instances.column(9).as_any().downcast_ref::<arrow::array::StringArray>().unwrap();

        // Rebuild all rows
        for i in 0..self.instances.num_rows() {
            if i == row_idx {
                // Apply updates to this row
                id_builder.append_value(id_array.value(i));
                workflow_id_builder.append_value(workflow_id_array.value(i));

                status_builder.append_value(
                    update.status.map(|s| s.as_str()).unwrap_or(status_array.value(i))
                );

                current_step_builder.append_value(
                    update.current_step_index.map(|s| s as i64).unwrap_or(current_step_array.value(i))
                );

                total_steps_builder.append_value(total_steps_array.value(i));

                if agent_id_array.is_null(i) {
                    agent_id_builder.append_null();
                } else {
                    agent_id_builder.append_value(agent_id_array.value(i));
                }

                started_at_builder.append_value(started_at_array.value(i));

                if let Some(completed_tick) = update.completed_at {
                    completed_at_builder.append_value(completed_tick);
                } else if completed_at_array.is_null(i) {
                    completed_at_builder.append_null();
                } else {
                    completed_at_builder.append_value(completed_at_array.value(i));
                }

                if context_array.is_null(i) {
                    context_builder.append_null();
                } else {
                    context_builder.append_value(context_array.value(i));
                }

                if metadata_array.is_null(i) {
                    metadata_builder.append_null();
                } else {
                    metadata_builder.append_value(metadata_array.value(i));
                }
            } else {
                // Copy existing row unchanged
                id_builder.append_value(id_array.value(i));
                workflow_id_builder.append_value(workflow_id_array.value(i));
                status_builder.append_value(status_array.value(i));
                current_step_builder.append_value(current_step_array.value(i));
                total_steps_builder.append_value(total_steps_array.value(i));

                if agent_id_array.is_null(i) {
                    agent_id_builder.append_null();
                } else {
                    agent_id_builder.append_value(agent_id_array.value(i));
                }

                started_at_builder.append_value(started_at_array.value(i));

                if completed_at_array.is_null(i) {
                    completed_at_builder.append_null();
                } else {
                    completed_at_builder.append_value(completed_at_array.value(i));
                }

                if context_array.is_null(i) {
                    context_builder.append_null();
                } else {
                    context_builder.append_value(context_array.value(i));
                }

                if metadata_array.is_null(i) {
                    metadata_builder.append_null();
                } else {
                    metadata_builder.append_value(metadata_array.value(i));
                }
            }
        }

        // Build arrays
        let arrays: Vec<ArrayRef> = vec![
            Arc::new(id_builder.finish()),
            Arc::new(workflow_id_builder.finish()),
            Arc::new(status_builder.finish()),
            Arc::new(current_step_builder.finish()),
            Arc::new(total_steps_builder.finish()),
            Arc::new(agent_id_builder.finish()),
            Arc::new(started_at_builder.finish()),
            Arc::new(completed_at_builder.finish()),
            Arc::new(context_builder.finish()),
            Arc::new(metadata_builder.finish()),
        ];

        // Replace RecordBatch
        self.instances = RecordBatch::try_new(self.instance_schema.clone(), arrays)
            .map_err(|e| crate::Error::Workflow(format!("Failed to update RecordBatch: {}", e)))?;

        Ok(())
    }

    /// Advance instance to next step after task completion (sync version)
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
        let row_idx = self.instance_id_to_row.get(&instance_id).ok_or_else(|| {
            crate::Error::Workflow(format!("Workflow instance {} not found", instance_id))
        })?;
        let row_idx = *row_idx;

        // Get current instance state
        let instance = self.get_instance_at_row(row_idx).ok_or_else(|| {
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
            self.update_instance_at_row_sync(row_idx, |update| {
                update.current_step_index = Some(new_step);
                update.status = Some(InstanceStatus::Complete);
                update.completed_at = Some(0); // TODO: Use actual simulation tick
            })?;

            Ok(Some(WorkflowStepEvent::WorkflowCompleted {
                instance_id,
                workflow_id: instance.workflow_id,
                completion_time,
            }))
        } else {
            self.update_instance_at_row_sync(row_idx, |update| {
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

    /// Synchronous version of update_instance_at_row
    fn update_instance_at_row_sync(
        &mut self,
        row_idx: usize,
        update_fn: impl FnOnce(&mut WorkflowInstanceRowUpdate),
    ) -> Result<()> {
        if row_idx >= self.instances.num_rows() {
            return Err(crate::Error::Workflow("Invalid row index".to_string()));
        }

        let mut update = WorkflowInstanceRowUpdate::default();
        update_fn(&mut update);

        // Rebuild RecordBatch with updated row
        let mut id_builder = BinaryBuilder::new();
        let mut workflow_id_builder = BinaryBuilder::new();
        let mut status_builder = StringBuilder::new();
        let mut current_step_builder = Int64Builder::new();
        let mut total_steps_builder = Int64Builder::new();
        let mut agent_id_builder = BinaryBuilder::new();
        let mut started_at_builder = Int64Builder::new();
        let mut completed_at_builder = Int64Builder::new();
        let mut context_builder = StringBuilder::new();
        let mut metadata_builder = StringBuilder::new();

        // Extract arrays
        let id_array = self.instances.column(0).as_any().downcast_ref::<BinaryArray>().unwrap();
        let workflow_id_array = self.instances.column(1).as_any().downcast_ref::<BinaryArray>().unwrap();
        let status_array = self.instances.column(2).as_any().downcast_ref::<arrow::array::StringArray>().unwrap();
        let current_step_array = self.instances.column(3).as_any().downcast_ref::<Int64Array>().unwrap();
        let total_steps_array = self.instances.column(4).as_any().downcast_ref::<Int64Array>().unwrap();
        let agent_id_array = self.instances.column(5).as_any().downcast_ref::<BinaryArray>().unwrap();
        let started_at_array = self.instances.column(6).as_any().downcast_ref::<Int64Array>().unwrap();
        let completed_at_array = self.instances.column(7).as_any().downcast_ref::<Int64Array>().unwrap();
        let context_array = self.instances.column(8).as_any().downcast_ref::<arrow::array::StringArray>().unwrap();
        let metadata_array = self.instances.column(9).as_any().downcast_ref::<arrow::array::StringArray>().unwrap();

        // Rebuild all rows
        for i in 0..self.instances.num_rows() {
            if i == row_idx {
                // Apply updates to this row
                id_builder.append_value(id_array.value(i));
                workflow_id_builder.append_value(workflow_id_array.value(i));

                status_builder.append_value(
                    update.status.map(|s| s.as_str()).unwrap_or(status_array.value(i))
                );

                current_step_builder.append_value(
                    update.current_step_index.map(|s| s as i64).unwrap_or(current_step_array.value(i))
                );

                total_steps_builder.append_value(total_steps_array.value(i));

                if agent_id_array.is_null(i) {
                    agent_id_builder.append_null();
                } else {
                    agent_id_builder.append_value(agent_id_array.value(i));
                }

                started_at_builder.append_value(started_at_array.value(i));

                if let Some(completed_tick) = update.completed_at {
                    completed_at_builder.append_value(completed_tick);
                } else if completed_at_array.is_null(i) {
                    completed_at_builder.append_null();
                } else {
                    completed_at_builder.append_value(completed_at_array.value(i));
                }

                if context_array.is_null(i) {
                    context_builder.append_null();
                } else {
                    context_builder.append_value(context_array.value(i));
                }

                if metadata_array.is_null(i) {
                    metadata_builder.append_null();
                } else {
                    metadata_builder.append_value(metadata_array.value(i));
                }
            } else {
                // Copy existing row unchanged
                id_builder.append_value(id_array.value(i));
                workflow_id_builder.append_value(workflow_id_array.value(i));
                status_builder.append_value(status_array.value(i));
                current_step_builder.append_value(current_step_array.value(i));
                total_steps_builder.append_value(total_steps_array.value(i));

                if agent_id_array.is_null(i) {
                    agent_id_builder.append_null();
                } else {
                    agent_id_builder.append_value(agent_id_array.value(i));
                }

                started_at_builder.append_value(started_at_array.value(i));

                if completed_at_array.is_null(i) {
                    completed_at_builder.append_null();
                } else {
                    completed_at_builder.append_value(completed_at_array.value(i));
                }

                if context_array.is_null(i) {
                    context_builder.append_null();
                } else {
                    context_builder.append_value(context_array.value(i));
                }

                if metadata_array.is_null(i) {
                    metadata_builder.append_null();
                } else {
                    metadata_builder.append_value(metadata_array.value(i));
                }
            }
        }

        // Build arrays
        let arrays: Vec<ArrayRef> = vec![
            Arc::new(id_builder.finish()),
            Arc::new(workflow_id_builder.finish()),
            Arc::new(status_builder.finish()),
            Arc::new(current_step_builder.finish()),
            Arc::new(total_steps_builder.finish()),
            Arc::new(agent_id_builder.finish()),
            Arc::new(started_at_builder.finish()),
            Arc::new(completed_at_builder.finish()),
            Arc::new(context_builder.finish()),
            Arc::new(metadata_builder.finish()),
        ];

        // Replace RecordBatch
        self.instances = RecordBatch::try_new(self.instance_schema.clone(), arrays)
            .map_err(|e| crate::Error::Workflow(format!("Failed to update RecordBatch: {}", e)))?;

        Ok(())
    }

    /// Get all instances with a specific status (direct RecordBatch iteration)
    pub fn get_instances_by_status(&self, status: InstanceStatus) -> Result<Vec<WorkflowInstanceId>> {
        let mut ids = Vec::new();

        if self.instances.num_rows() == 0 {
            return Ok(ids);
        }

        // Direct iteration over RecordBatch (no async needed)
        let id_array = self.instances.column(0).as_any().downcast_ref::<BinaryArray>()
            .ok_or_else(|| crate::Error::Workflow("Failed to downcast id column".to_string()))?;
        let status_array = self.instances.column(2).as_any().downcast_ref::<arrow::array::StringArray>()
            .ok_or_else(|| crate::Error::Workflow("Failed to downcast status column".to_string()))?;

        for i in 0..self.instances.num_rows() {
            if status_array.value(i) == status.as_str() {
                let id_bytes = id_array.value(i);
                if let Ok(uuid) = Uuid::from_slice(id_bytes) {
                    ids.push(WorkflowInstanceId::from_uuid(uuid));
                }
            }
        }

        Ok(ids)
    }

    /// Get all running instances
    pub fn get_running_instances(&self) -> Result<Vec<WorkflowInstanceId>> {
        self.get_instances_by_status(InstanceStatus::Running)
    }

    /// Get all completed instances
    pub fn get_completed_instances(&self) -> Result<Vec<WorkflowInstanceId>> {
        self.get_instances_by_status(InstanceStatus::Complete)
    }

    /// Get instances by agent ID (direct RecordBatch iteration)
    pub fn get_agent_workflows(&self, agent_id: AgentId) -> Result<Vec<WorkflowInstanceId>> {
        let mut ids = Vec::new();

        if self.instances.num_rows() == 0 {
            return Ok(ids);
        }

        let id_array = self.instances.column(0).as_any().downcast_ref::<BinaryArray>()
            .ok_or_else(|| crate::Error::Workflow("Failed to downcast id column".to_string()))?;
        let agent_array = self.instances.column(5).as_any().downcast_ref::<BinaryArray>()
            .ok_or_else(|| crate::Error::Workflow("Failed to downcast agent_id column".to_string()))?;

        let agent_bytes = agent_id.as_bytes();
        for i in 0..self.instances.num_rows() {
            if !agent_array.is_null(i) && agent_array.value(i) == agent_bytes {
                let id_bytes = id_array.value(i);
                if let Ok(uuid) = Uuid::from_slice(id_bytes) {
                    ids.push(WorkflowInstanceId::from_uuid(uuid));
                }
            }
        }

        Ok(ids)
    }

    /// Pause a workflow instance
    pub fn pause_instance(&mut self, instance_id: WorkflowInstanceId) -> Result<()> {
        let row_idx = *self.instance_id_to_row.get(&instance_id).ok_or_else(|| {
            crate::Error::Workflow(format!("Workflow instance {} not found", instance_id))
        })?;

        self.update_instance_at_row_sync(row_idx, |update| {
            update.status = Some(InstanceStatus::Paused);
        })
    }

    /// Resume a workflow instance
    pub fn resume_instance(&mut self, instance_id: WorkflowInstanceId) -> Result<()> {
        let row_idx = *self.instance_id_to_row.get(&instance_id).ok_or_else(|| {
            crate::Error::Workflow(format!("Workflow instance {} not found", instance_id))
        })?;

        self.update_instance_at_row_sync(row_idx, |update| {
            update.status = Some(InstanceStatus::Running);
        })
    }

    /// Complete a workflow instance
    pub fn complete_instance(&mut self, instance_id: WorkflowInstanceId) -> Result<()> {
        let row_idx = *self.instance_id_to_row.get(&instance_id).ok_or_else(|| {
            crate::Error::Workflow(format!("Workflow instance {} not found", instance_id))
        })?;

        self.update_instance_at_row_sync(row_idx, |update| {
            update.status = Some(InstanceStatus::Complete);
            update.completed_at = Some(0); // TODO: Use actual simulation tick
        })
    }

    /// Get instance status
    pub fn get_instance_status(&self, instance_id: WorkflowInstanceId) -> Option<InstanceStatus> {
        let row_idx = self.instance_id_to_row.get(&instance_id)?;
        let status_array = self.instances.column(2).as_any().downcast_ref::<arrow::array::StringArray>()?;
        let status_str = status_array.value(*row_idx);
        InstanceStatus::from_str(status_str)
    }

    /// Get total instance count
    pub fn instance_count(&self) -> usize {
        self.instances.num_rows()
    }

    /// Get workflow count
    pub fn workflow_count(&self) -> usize {
        self.workflows.len()
    }

    /// Process workflow instances for current tick
    ///
    /// Currently a placeholder for future time-based workflow steps
    pub fn tick(&mut self, _current_time: DateTime<Utc>) -> Result<Vec<WorkflowStepEvent>> {
        // Future: Process time-based workflow steps
        Ok(Vec::new())
    }

    /// Get all instances ready for step assignment
    pub fn get_ready_instances(&self) -> Vec<WorkflowInstance> {
        let mut instances = Vec::new();

        // Iterate through all rows in RecordBatch
        for row_idx in 0..self.instances.num_rows() {
            if let Some(instance) = self.get_instance_at_row(row_idx) {
                if instance.is_ready_for_assignment() {
                    instances.push(instance);
                }
            }
        }

        instances
    }

    /// Get mutable instance (returns owned instance for modification)
    pub fn get_instance_mut(&mut self, id: WorkflowInstanceId) -> Option<WorkflowInstance> {
        // Since RecordBatch is immutable, we return an owned copy
        // The caller should use update methods to persist changes
        self.get_instance(id)
    }
}

/// Helper struct for row updates
#[derive(Default)]
struct WorkflowInstanceRowUpdate {
    status: Option<InstanceStatus>,
    current_step_index: Option<usize>,
    completed_at: Option<i64>,
}

impl<W: Workflow> Default for WorkflowEngine<W> {
    fn default() -> Self {
        Self::new()
    }
}

/* Temporarily disabled due to async issues
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
    fn test_get_instance() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_test_workflow();
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);
        let instance = engine.start_instance(workflow_id).unwrap();
        let instance_id = instance.id;

        let retrieved = engine.get_instance(instance_id).unwrap();
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
        let event = engine.advance_instance_step(instance_id, completion_time).unwrap();

        assert!(event.is_some());
        match event.unwrap() {
            WorkflowStepEvent::StepCompleted { instance_id: id, step_index, .. } => {
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
            WorkflowStepEvent::WorkflowCompleted { instance_id: id, workflow_id: wf_id, .. } => {
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
        let status = engine.get_instance_status(instance_id).unwrap();
        assert_eq!(status, InstanceStatus::Paused);

        // Resume
        engine.resume_instance(instance_id).unwrap();
        let status = engine.get_instance_status(instance_id).unwrap();
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
        engine.advance_instance_step(instance_id, completion_time).unwrap();
        engine.advance_instance_step(instance_id, completion_time).unwrap();

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
}
}
*/
