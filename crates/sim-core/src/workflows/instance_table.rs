//! WorkflowInstanceTable - Arrow-based storage and manipulation for workflow instances

use super::schema::{InstanceStatus, workflow_instance_schema};
use super::traits::WorkflowInstance;
use crate::Result;
use crate::utils::idents::{AgentId, WorkflowId, WorkflowInstanceId};
use arrow::array::{
    Array, ArrayRef, BinaryArray, BinaryBuilder, Int64Array, Int64Builder, RecordBatch,
    StringBuilder,
};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Encapsulates RecordBatch storage and manipulation for workflow instances
///
/// This struct provides a clean separation between Arrow storage operations
/// and the high-level workflow business logic in WorkflowEngine.
///
/// # Responsibilities
///
/// - RecordBatch storage and schema management
/// - HashMap index for O(1) instance lookups
/// - Array building and manipulation
/// - Instance CRUD operations (Create, Read, Update, Delete)
/// - Batch export/import for persistence
///
/// # Example
///
/// ```rust,ignore
/// let table = WorkflowInstanceTable::new();
/// table.append_instance(&instance)?;
/// let retrieved = table.get_instance(instance_id);
/// ```
pub struct WorkflowInstanceTable {
    /// Active workflow instances (Arrow RecordBatch)
    instances: RecordBatch,

    /// Schema for instances RecordBatch
    instance_schema: arrow::datatypes::SchemaRef,

    /// Lookup map: WorkflowInstanceId → row index in RecordBatch
    instance_id_to_row: HashMap<WorkflowInstanceId, usize>,

    /// Current simulation tick for timestamp tracking
    current_tick: i64,
}

impl WorkflowInstanceTable {
    /// Create a new empty instance table
    pub fn new() -> Self {
        let schema = workflow_instance_schema();
        let empty_batch = RecordBatch::new_empty(schema.clone());

        Self {
            instances: empty_batch,
            instance_schema: schema,
            instance_id_to_row: HashMap::new(),
            current_tick: 0,
        }
    }

    /// Set the current simulation tick
    pub fn set_current_tick(&mut self, tick: i64) {
        self.current_tick = tick;
    }

    /// Get the current simulation tick
    pub fn current_tick(&self) -> i64 {
        self.current_tick
    }

    /// Get total instance count
    pub fn instance_count(&self) -> usize {
        self.instances.num_rows()
    }

    /// Append a new instance to the RecordBatch
    pub fn append_instance(&mut self, instance: &WorkflowInstance) -> Result<()> {
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
            )?;
        }

        // Append new instance
        id_builder.append_value(instance.id.as_bytes());
        workflow_id_builder.append_value(instance.workflow_id.as_bytes());

        // Determine status
        let status = if instance.is_complete {
            InstanceStatus::Complete
        } else {
            InstanceStatus::Running // Default to Running (whether just created or in progress)
        };
        status_builder.append_value(status.as_str());

        current_step_builder.append_value(instance.current_step as i64);
        total_steps_builder.append_value(instance.total_steps as i64);
        agent_id_builder.append_null(); // No agent assigned initially
        started_at_builder.append_value(self.current_tick);
        completed_at_builder.append_null();
        context_builder.append_null();

        // Serialize metadata as JSON
        if !instance.metadata.is_empty() {
            let metadata_json = serde_json::to_string(&instance.metadata).map_err(|e| {
                crate::Error::Workflow(format!("Failed to serialize metadata: {}", e))
            })?;
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

    /// Get a workflow instance by ID (O(1) lookup)
    pub fn get_instance(&self, id: WorkflowInstanceId) -> Option<WorkflowInstance> {
        let row_idx = self.instance_id_to_row.get(&id)?;
        self.get_instance_at_row(*row_idx)
    }

    /// Get workflow instance at specific row index
    pub fn get_instance_at_row(&self, row_idx: usize) -> Option<WorkflowInstance> {
        if row_idx >= self.instances.num_rows() {
            return None;
        }

        // Extract data from RecordBatch
        let id_array = self
            .instances
            .column(0)
            .as_any()
            .downcast_ref::<BinaryArray>()?;
        let workflow_id_array = self
            .instances
            .column(1)
            .as_any()
            .downcast_ref::<BinaryArray>()?;
        let current_step_array = self
            .instances
            .column(3)
            .as_any()
            .downcast_ref::<Int64Array>()?;
        let total_steps_array = self
            .instances
            .column(4)
            .as_any()
            .downcast_ref::<Int64Array>()?;
        let status_array = self
            .instances
            .column(2)
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()?;
        let metadata_array = self
            .instances
            .column(9)
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()?;

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
            // If parsing fails, use empty metadata rather than failing the whole operation
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

    /// Update instance at specific row (O(N) operation - rebuilds RecordBatch)
    pub fn update_instance_at_row(
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

        // Extract arrays using helper
        let arrays = InstanceArrays::from_batch(&self.instances)?;

        // Rebuild all rows
        for i in 0..self.instances.num_rows() {
            if i == row_idx {
                // Apply updates to this row
                arrays.copy_row_with_update(
                    i,
                    &update,
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
            } else {
                // Copy existing row unchanged
                arrays.copy_row_to_builders(
                    i,
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
        }

        // Build arrays
        let arrays_vec: Vec<ArrayRef> = vec![
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
        self.instances = RecordBatch::try_new(self.instance_schema.clone(), arrays_vec)
            .map_err(|e| crate::Error::Workflow(format!("Failed to update RecordBatch: {}", e)))?;

        Ok(())
    }

    /// Batch update multiple workflow instances in a single RecordBatch rebuild (O(N) operation)
    ///
    /// This method is much more efficient than calling update_instance_at_row() in a loop,
    /// as it rebuilds the RecordBatch only once for all updates instead of N times.
    ///
    /// # Arguments
    ///
    /// * `updates` - Vec of (instance_id, Box<dyn FnOnce>) tuples
    ///
    /// # Returns
    ///
    /// Number of instances successfully updated
    ///
    /// # Performance
    ///
    /// - Single update: O(N) where N = total instances
    /// - Batch update: O(N) for any number of updates
    /// - Efficiency gain: ~M times faster where M = number of updates
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use crate::workflows::instance_table::{BatchUpdateList, WorkflowInstanceUpdate};
    /// let updates: BatchUpdateList = vec![
    ///     (instance_id_1, Box::new(|u| { u.status = Some(InstanceStatus::Complete); })),
    ///     (instance_id_2, Box::new(|u| { u.current_step_index = Some(2); })),
    ///     (instance_id_3, Box::new(|u| { u.status = Some(InstanceStatus::Paused); })),
    /// ];
    /// let count = table.batch_update_instances(updates)?;
    /// ```
    #[allow(dead_code)] // Public API for future use
    pub fn batch_update_instances(&mut self, updates: BatchUpdateList) -> Result<usize> {
        if updates.is_empty() {
            return Ok(0);
        }

        // Build a map of row_idx -> update for efficient lookup
        let mut row_updates: HashMap<usize, WorkflowInstanceRowUpdate> = HashMap::new();

        for (instance_id, update_fn) in updates {
            if let Some(&row_idx) = self.instance_id_to_row.get(&instance_id) {
                let mut update = WorkflowInstanceRowUpdate::default();
                update_fn(&mut update);
                row_updates.insert(row_idx, update);
            }
        }

        if row_updates.is_empty() {
            return Ok(0);
        }

        let updated_count = row_updates.len();

        // Rebuild RecordBatch with all updates applied
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

        // Extract arrays using helper
        let arrays = InstanceArrays::from_batch(&self.instances)?;

        // Rebuild all rows, applying updates where applicable
        for i in 0..self.instances.num_rows() {
            if let Some(update) = row_updates.get(&i) {
                // Apply updates to this row
                arrays.copy_row_with_update(
                    i,
                    update,
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
            } else {
                // Copy existing row unchanged
                arrays.copy_row_to_builders(
                    i,
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
        }

        // Build arrays
        let arrays_vec: Vec<ArrayRef> = vec![
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
        self.instances = RecordBatch::try_new(self.instance_schema.clone(), arrays_vec)
            .map_err(|e| crate::Error::Workflow(format!("Failed to update RecordBatch: {}", e)))?;

        Ok(updated_count)
    }

    /// Get instance status
    pub fn get_instance_status(&self, instance_id: WorkflowInstanceId) -> Option<InstanceStatus> {
        let row_idx = self.instance_id_to_row.get(&instance_id)?;
        let status_array = self
            .instances
            .column(2)
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()?;
        let status_str = status_array.value(*row_idx);
        InstanceStatus::parse(status_str)
    }

    /// Get all instances with a specific status (direct RecordBatch iteration)
    pub fn get_instances_by_status(
        &self,
        status: InstanceStatus,
    ) -> Result<Vec<WorkflowInstanceId>> {
        let mut ids = Vec::new();

        if self.instances.num_rows() == 0 {
            return Ok(ids);
        }

        // Direct iteration over RecordBatch (no async needed)
        let id_array = self
            .instances
            .column(0)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| crate::Error::Workflow("Failed to downcast id column".to_string()))?;
        let status_array = self
            .instances
            .column(2)
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .ok_or_else(|| {
                crate::Error::Workflow("Failed to downcast status column".to_string())
            })?;

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

    /// Get instances by agent ID (direct RecordBatch iteration)
    pub fn get_agent_workflows(&self, agent_id: AgentId) -> Result<Vec<WorkflowInstanceId>> {
        let mut ids = Vec::new();

        if self.instances.num_rows() == 0 {
            return Ok(ids);
        }

        let id_array = self
            .instances
            .column(0)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| crate::Error::Workflow("Failed to downcast id column".to_string()))?;
        let agent_array = self
            .instances
            .column(5)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| {
                crate::Error::Workflow("Failed to downcast agent_id column".to_string())
            })?;

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

    /// Get all instances ready for step assignment
    pub fn get_ready_instances(&self) -> Vec<WorkflowInstance> {
        let mut instances = Vec::new();

        // Iterate through all rows in RecordBatch
        for row_idx in 0..self.instances.num_rows() {
            if let Some(instance) = self.get_instance_at_row(row_idx)
                && instance.is_ready_for_assignment()
            {
                instances.push(instance);
            }
        }

        instances
    }

    /// Export the workflow instances RecordBatch
    ///
    /// Returns a copy of the internal RecordBatch containing all workflow instances.
    /// This can be used for persistence, serialization, or state transfer.
    pub fn export_batch(&self) -> RecordBatch {
        self.instances.clone()
    }

    /// Import and restore workflow instances from a RecordBatch
    ///
    /// Replaces the current workflow instances with the provided batch and rebuilds
    /// all internal indices. The batch must conform to the expected workflow instance
    /// schema.
    pub fn import_batch(&mut self, batch: RecordBatch) -> Result<()> {
        // Validate schema
        Self::validate_instance_schema(&batch)?;

        // Replace instances
        self.instances = batch;

        // Rebuild instance_id_to_row lookup map
        self.rebuild_instance_index()?;

        Ok(())
    }

    /// Get row index for an instance ID
    pub fn get_row_index(&self, id: WorkflowInstanceId) -> Option<usize> {
        self.instance_id_to_row.get(&id).copied()
    }

    /// Validates that a RecordBatch conforms to the expected workflow instance schema
    fn validate_instance_schema(batch: &RecordBatch) -> Result<()> {
        use arrow::datatypes::DataType;

        let schema = batch.schema();

        // Expected: 10 columns
        if schema.fields().len() != 10 {
            return Err(crate::Error::Workflow(format!(
                "Invalid instance schema: expected 10 columns, got {}",
                schema.fields().len()
            )));
        }

        // Validate each field
        let expected = [
            ("id", DataType::Binary, false),
            ("workflow_id", DataType::Binary, false),
            ("status", DataType::Utf8, false),
            ("current_step_index", DataType::Int64, false),
            ("total_steps", DataType::Int64, false),
            ("agent_id", DataType::Binary, true),
            ("started_at", DataType::Int64, false),
            ("completed_at", DataType::Int64, true),
            ("context", DataType::Utf8, true),
            ("metadata", DataType::Utf8, true),
        ];

        for (i, (exp_name, exp_type, exp_nullable)) in expected.iter().enumerate() {
            let field = schema.field(i);

            if field.name() != exp_name {
                return Err(crate::Error::Workflow(format!(
                    "Field {} has wrong name: expected '{}', got '{}'",
                    i,
                    exp_name,
                    field.name()
                )));
            }

            // Accept Parquet type optimizations: Utf8↔Utf8View, Binary↔BinaryView
            let type_matches = field.data_type() == exp_type
                || (matches!(exp_type, DataType::Utf8)
                    && matches!(field.data_type(), DataType::Utf8View))
                || (matches!(exp_type, DataType::Binary)
                    && matches!(field.data_type(), DataType::BinaryView));

            if !type_matches {
                return Err(crate::Error::Workflow(format!(
                    "Field '{}' has wrong type: expected {:?}, got {:?}",
                    exp_name,
                    exp_type,
                    field.data_type()
                )));
            }

            if field.is_nullable() != *exp_nullable {
                return Err(crate::Error::Workflow(format!(
                    "Field '{}' has wrong nullability: expected {}, got {}",
                    exp_name,
                    exp_nullable,
                    field.is_nullable()
                )));
            }
        }

        Ok(())
    }

    /// Rebuild the instance_id_to_row lookup map from the current RecordBatch
    fn rebuild_instance_index(&mut self) -> Result<()> {
        self.instance_id_to_row.clear();

        if self.instances.num_rows() == 0 {
            return Ok(());
        }

        // Extract id column
        let id_array = self
            .instances
            .column(0)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| {
                crate::Error::Workflow("Failed to downcast id column to BinaryArray".to_string())
            })?;

        // Build lookup map
        for row_idx in 0..self.instances.num_rows() {
            let id_bytes = id_array.value(row_idx);
            let uuid = Uuid::from_slice(id_bytes).map_err(|e| {
                crate::Error::Workflow(format!("Invalid UUID in row {}: {}", row_idx, e))
            })?;
            let instance_id = WorkflowInstanceId::from_uuid(uuid);

            self.instance_id_to_row.insert(instance_id, row_idx);
        }

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
    ) -> Result<()> {
        let num_rows = self.instances.num_rows();
        let arrays = InstanceArrays::from_batch(&self.instances)?;

        // Copy each row using helper
        for i in 0..num_rows {
            arrays.copy_row_to_builders(
                i,
                id_builder,
                workflow_id_builder,
                status_builder,
                current_step_builder,
                total_steps_builder,
                agent_id_builder,
                started_at_builder,
                completed_at_builder,
                context_builder,
                metadata_builder,
            );
        }

        Ok(())
    }
}

impl Default for WorkflowInstanceTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper struct for row updates
#[derive(Default)]
pub struct WorkflowInstanceRowUpdate {
    pub status: Option<InstanceStatus>,
    pub current_step_index: Option<usize>,
    pub completed_at: Option<i64>,
}

/// Type alias for batch update closures
#[allow(dead_code)] // Public API for future use
pub type WorkflowInstanceUpdate = Box<dyn FnOnce(&mut WorkflowInstanceRowUpdate)>;

/// Type alias for batch update parameter list
#[allow(dead_code)] // Public API for future use
pub type BatchUpdateList = Vec<(WorkflowInstanceId, WorkflowInstanceUpdate)>;

/// Helper struct to extract and hold references to RecordBatch arrays
struct InstanceArrays<'a> {
    id: &'a BinaryArray,
    workflow_id: &'a BinaryArray,
    status: &'a arrow::array::StringArray,
    current_step: &'a Int64Array,
    total_steps: &'a Int64Array,
    agent_id: &'a BinaryArray,
    started_at: &'a Int64Array,
    completed_at: &'a Int64Array,
    context: &'a arrow::array::StringArray,
    metadata: &'a arrow::array::StringArray,
}

impl<'a> InstanceArrays<'a> {
    /// Extract array references from a RecordBatch
    fn from_batch(batch: &'a RecordBatch) -> Result<Self> {
        Ok(Self {
            id: batch
                .column(0)
                .as_any()
                .downcast_ref::<BinaryArray>()
                .ok_or_else(|| {
                    crate::Error::Workflow(
                        "Failed to downcast id column to BinaryArray".to_string(),
                    )
                })?,
            workflow_id: batch
                .column(1)
                .as_any()
                .downcast_ref::<BinaryArray>()
                .ok_or_else(|| {
                    crate::Error::Workflow(
                        "Failed to downcast workflow_id column to BinaryArray".to_string(),
                    )
                })?,
            status: batch
                .column(2)
                .as_any()
                .downcast_ref::<arrow::array::StringArray>()
                .ok_or_else(|| {
                    crate::Error::Workflow(
                        "Failed to downcast status column to StringArray".to_string(),
                    )
                })?,
            current_step: batch
                .column(3)
                .as_any()
                .downcast_ref::<Int64Array>()
                .ok_or_else(|| {
                    crate::Error::Workflow(
                        "Failed to downcast current_step column to Int64Array".to_string(),
                    )
                })?,
            total_steps: batch
                .column(4)
                .as_any()
                .downcast_ref::<Int64Array>()
                .ok_or_else(|| {
                    crate::Error::Workflow(
                        "Failed to downcast total_steps column to Int64Array".to_string(),
                    )
                })?,
            agent_id: batch
                .column(5)
                .as_any()
                .downcast_ref::<BinaryArray>()
                .ok_or_else(|| {
                    crate::Error::Workflow(
                        "Failed to downcast agent_id column to BinaryArray".to_string(),
                    )
                })?,
            started_at: batch
                .column(6)
                .as_any()
                .downcast_ref::<Int64Array>()
                .ok_or_else(|| {
                    crate::Error::Workflow(
                        "Failed to downcast started_at column to Int64Array".to_string(),
                    )
                })?,
            completed_at: batch
                .column(7)
                .as_any()
                .downcast_ref::<Int64Array>()
                .ok_or_else(|| {
                    crate::Error::Workflow(
                        "Failed to downcast completed_at column to Int64Array".to_string(),
                    )
                })?,
            context: batch
                .column(8)
                .as_any()
                .downcast_ref::<arrow::array::StringArray>()
                .ok_or_else(|| {
                    crate::Error::Workflow(
                        "Failed to downcast context column to StringArray".to_string(),
                    )
                })?,
            metadata: batch
                .column(9)
                .as_any()
                .downcast_ref::<arrow::array::StringArray>()
                .ok_or_else(|| {
                    crate::Error::Workflow(
                        "Failed to downcast metadata column to StringArray".to_string(),
                    )
                })?,
        })
    }

    /// Copy a row from the arrays into builders
    #[allow(clippy::too_many_arguments)]
    fn copy_row_to_builders(
        &self,
        row_idx: usize,
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
        id_builder.append_value(self.id.value(row_idx));
        workflow_id_builder.append_value(self.workflow_id.value(row_idx));
        status_builder.append_value(self.status.value(row_idx));
        current_step_builder.append_value(self.current_step.value(row_idx));
        total_steps_builder.append_value(self.total_steps.value(row_idx));

        if self.agent_id.is_null(row_idx) {
            agent_id_builder.append_null();
        } else {
            agent_id_builder.append_value(self.agent_id.value(row_idx));
        }

        started_at_builder.append_value(self.started_at.value(row_idx));

        if self.completed_at.is_null(row_idx) {
            completed_at_builder.append_null();
        } else {
            completed_at_builder.append_value(self.completed_at.value(row_idx));
        }

        if self.context.is_null(row_idx) {
            context_builder.append_null();
        } else {
            context_builder.append_value(self.context.value(row_idx));
        }

        if self.metadata.is_null(row_idx) {
            metadata_builder.append_null();
        } else {
            metadata_builder.append_value(self.metadata.value(row_idx));
        }
    }

    /// Copy a row with updates applied
    #[allow(clippy::too_many_arguments)]
    fn copy_row_with_update(
        &self,
        row_idx: usize,
        update: &WorkflowInstanceRowUpdate,
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
        id_builder.append_value(self.id.value(row_idx));
        workflow_id_builder.append_value(self.workflow_id.value(row_idx));

        status_builder.append_value(
            update
                .status
                .map(|s| s.as_str())
                .unwrap_or(self.status.value(row_idx)),
        );

        current_step_builder.append_value(
            update
                .current_step_index
                .map(|s| s as i64)
                .unwrap_or(self.current_step.value(row_idx)),
        );

        total_steps_builder.append_value(self.total_steps.value(row_idx));

        if self.agent_id.is_null(row_idx) {
            agent_id_builder.append_null();
        } else {
            agent_id_builder.append_value(self.agent_id.value(row_idx));
        }

        started_at_builder.append_value(self.started_at.value(row_idx));

        if let Some(completed_tick) = update.completed_at {
            completed_at_builder.append_value(completed_tick);
        } else if self.completed_at.is_null(row_idx) {
            completed_at_builder.append_null();
        } else {
            completed_at_builder.append_value(self.completed_at.value(row_idx));
        }

        if self.context.is_null(row_idx) {
            context_builder.append_null();
        } else {
            context_builder.append_value(self.context.value(row_idx));
        }

        if self.metadata.is_null(row_idx) {
            metadata_builder.append_null();
        } else {
            metadata_builder.append_value(self.metadata.value(row_idx));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::traits::WorkflowInstance;
    use super::*;
    use std::time::Instant;

    fn create_test_instance(workflow_id: WorkflowId) -> WorkflowInstance {
        WorkflowInstance::new(workflow_id, 3)
    }

    #[test]
    fn test_batch_update_basic() {
        let mut table = WorkflowInstanceTable::new();
        let workflow_id = WorkflowId::new();

        // Create 5 instances
        let instances: Vec<_> = (0..5)
            .map(|_| {
                let inst = create_test_instance(workflow_id);
                table.append_instance(&inst).unwrap();
                inst
            })
            .collect();

        // Batch update 3 instances
        let updates: BatchUpdateList = vec![
            (
                instances[0].id,
                Box::new(|u| {
                    u.current_step_index = Some(1);
                }),
            ),
            (
                instances[2].id,
                Box::new(|u| {
                    u.current_step_index = Some(2);
                }),
            ),
            (
                instances[4].id,
                Box::new(|u| {
                    u.status = Some(InstanceStatus::Paused);
                }),
            ),
        ];

        let count = table.batch_update_instances(updates).unwrap();
        assert_eq!(count, 3);

        // Verify updates
        let inst0 = table.get_instance(instances[0].id).unwrap();
        assert_eq!(inst0.current_step, 1);

        let inst1 = table.get_instance(instances[1].id).unwrap();
        assert_eq!(inst1.current_step, 0); // Unchanged

        let inst2 = table.get_instance(instances[2].id).unwrap();
        assert_eq!(inst2.current_step, 2);

        let status4 = table.get_instance_status(instances[4].id).unwrap();
        assert_eq!(status4, InstanceStatus::Paused);
    }

    #[test]
    fn test_batch_update_empty() {
        let mut table = WorkflowInstanceTable::new();
        let updates: BatchUpdateList = vec![];
        let count = table.batch_update_instances(updates).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_batch_update_nonexistent_ids() {
        let mut table = WorkflowInstanceTable::new();
        let workflow_id = WorkflowId::new();
        let instance = create_test_instance(workflow_id);
        table.append_instance(&instance).unwrap();

        let fake_id = WorkflowInstanceId::new();
        let updates: BatchUpdateList = vec![
            (
                instance.id,
                Box::new(|u| {
                    u.current_step_index = Some(1);
                }),
            ),
            (
                fake_id,
                Box::new(|u| {
                    u.current_step_index = Some(2);
                }),
            ),
        ];

        let count = table.batch_update_instances(updates).unwrap();
        assert_eq!(count, 1);

        let inst = table.get_instance(instance.id).unwrap();
        assert_eq!(inst.current_step, 1);
    }

    #[test]
    fn test_batch_update_performance() {
        let mut table = WorkflowInstanceTable::new();
        let workflow_id = WorkflowId::new();

        // Create 1000 instances
        let instances: Vec<_> = (0..1000)
            .map(|_| {
                let inst = create_test_instance(workflow_id);
                table.append_instance(&inst).unwrap();
                inst
            })
            .collect();

        // Test 1: Individual updates (O(N²))
        let mut table_single = WorkflowInstanceTable::new();
        let instances_single: Vec<_> = (0..1000)
            .map(|_| {
                let inst = create_test_instance(workflow_id);
                table_single.append_instance(&inst).unwrap();
                inst
            })
            .collect();

        let start = Instant::now();
        for instance in instances_single.iter().take(100) {
            let row_idx = *table_single.instance_id_to_row.get(&instance.id).unwrap();
            table_single
                .update_instance_at_row(row_idx, |u| {
                    u.current_step_index = Some(1);
                })
                .unwrap();
        }
        let single_duration = start.elapsed();

        // Test 2: Batch update (O(N))
        let start = Instant::now();
        let updates: BatchUpdateList = instances
            .iter()
            .take(100)
            .map(|inst| {
                let update: WorkflowInstanceUpdate =
                    Box::new(|u: &mut WorkflowInstanceRowUpdate| {
                        u.current_step_index = Some(1);
                    });
                (inst.id, update)
            })
            .collect();
        let count = table.batch_update_instances(updates).unwrap();
        let batch_duration = start.elapsed();

        assert_eq!(count, 100);

        // Verify all 100 instances were updated
        for instance in instances.iter().take(100) {
            let inst = table.get_instance(instance.id).unwrap();
            assert_eq!(inst.current_step, 1);
        }

        // Calculate speedup
        let speedup = single_duration.as_secs_f64() / batch_duration.as_secs_f64();

        println!("Performance comparison for 100 updates on 1000 instances:");
        println!("  Individual updates: {:?}", single_duration);
        println!("  Batch update: {:?}", batch_duration);
        println!("  Speedup: {:.2}x", speedup);

        // Batch should be significantly faster (at least 10x for 100 updates)
        assert!(
            speedup > 10.0,
            "Batch update should be at least 10x faster, got {:.2}x",
            speedup
        );
    }

    #[test]
    fn test_batch_update_complete_workflows() {
        let mut table = WorkflowInstanceTable::new();
        let workflow_id = WorkflowId::new();

        // Create 10 instances
        let instances: Vec<_> = (0..10)
            .map(|_| {
                let inst = create_test_instance(workflow_id);
                table.append_instance(&inst).unwrap();
                inst
            })
            .collect();

        table.set_current_tick(100);
        let current_tick = table.current_tick();

        let updates: BatchUpdateList = instances
            .iter()
            .map(|inst| {
                let update: WorkflowInstanceUpdate =
                    Box::new(move |u: &mut WorkflowInstanceRowUpdate| {
                        u.status = Some(InstanceStatus::Complete);
                        u.current_step_index = Some(3);
                        u.completed_at = Some(current_tick);
                    });
                (inst.id, update)
            })
            .collect();

        let count = table.batch_update_instances(updates).unwrap();
        assert_eq!(count, 10);

        // Verify all are complete
        let completed = table
            .get_instances_by_status(InstanceStatus::Complete)
            .unwrap();
        assert_eq!(completed.len(), 10);
    }

    #[test]
    fn test_batch_update_mixed_operations() {
        let mut table = WorkflowInstanceTable::new();
        let workflow_id = WorkflowId::new();

        // Create 6 instances
        let instances: Vec<_> = (0..6)
            .map(|_| {
                let inst = create_test_instance(workflow_id);
                table.append_instance(&inst).unwrap();
                inst
            })
            .collect();

        table.set_current_tick(200);
        let current_tick = table.current_tick();

        let updates: BatchUpdateList = vec![
            // Complete first two
            (
                instances[0].id,
                Box::new(move |u| {
                    u.status = Some(InstanceStatus::Complete);
                    u.current_step_index = Some(3);
                    u.completed_at = Some(current_tick);
                }),
            ),
            (
                instances[1].id,
                Box::new(move |u| {
                    u.status = Some(InstanceStatus::Complete);
                    u.current_step_index = Some(3);
                    u.completed_at = Some(current_tick);
                }),
            ),
            // Pause next two
            (
                instances[2].id,
                Box::new(|u| {
                    u.status = Some(InstanceStatus::Paused);
                }),
            ),
            (
                instances[3].id,
                Box::new(|u| {
                    u.status = Some(InstanceStatus::Paused);
                }),
            ),
            // Advance next two to step 1
            (
                instances[4].id,
                Box::new(|u| {
                    u.current_step_index = Some(1);
                    u.status = Some(InstanceStatus::Running);
                }),
            ),
            (
                instances[5].id,
                Box::new(|u| {
                    u.current_step_index = Some(1);
                    u.status = Some(InstanceStatus::Running);
                }),
            ),
        ];

        let count = table.batch_update_instances(updates).unwrap();
        assert_eq!(count, 6);

        // Verify completed
        let completed = table
            .get_instances_by_status(InstanceStatus::Complete)
            .unwrap();
        assert_eq!(completed.len(), 2);

        // Verify paused
        let status2 = table.get_instance_status(instances[2].id).unwrap();
        assert_eq!(status2, InstanceStatus::Paused);

        // Verify running and steps
        let inst4 = table.get_instance(instances[4].id).unwrap();
        assert_eq!(inst4.current_step, 1);
    }
}
