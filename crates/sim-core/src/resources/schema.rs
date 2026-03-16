//! Arrow schemas for resource allocation
//!
//! This module defines the Arrow schemas used to store resources and tasks in RecordBatch format.
//! The dual-schema design allows efficient storage and querying of allocation state.

use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use std::sync::Arc;

/// Schema for resource storage
///
/// Stores resource capacity, availability, and metadata in columnar format.
/// Each row represents one resource instance.
///
/// # Fields
///
/// * `id` - Binary UUID representing ResourceId (16 bytes)
/// * `resource_type` - String type identifier (e.g., "kitchen_station", "checkout_lane")
/// * `capacity` - Total capacity (maximum concurrent tasks)
/// * `available` - Currently available capacity units
/// * `location_id` - Optional LocationId UUID (16 bytes) for spatial resources
/// * `metadata` - JSON string for domain-specific fields
///
/// # Example
///
/// ```rust,ignore
/// let schema = resource_schema();
/// let batch = RecordBatch::try_new(
///     schema,
///     vec![
///         Arc::new(BinaryArray::from(vec![resource_id.as_bytes()])),
///         Arc::new(StringArray::from(vec!["kitchen_station"])),
///         Arc::new(Int64Array::from(vec![10])),
///         Arc::new(Int64Array::from(vec![7])),
///         Arc::new(BinaryArray::from(vec![None])),
///         Arc::new(StringArray::from(vec![Some("{}")])),
///     ],
/// )?;
/// ```
pub fn resource_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Binary, false), // ResourceId UUID
        Field::new("resource_type", DataType::Utf8, false), // Type identifier
        Field::new("capacity", DataType::Int64, false), // Total capacity
        Field::new("available", DataType::Int64, false), // Available units
        Field::new("location_id", DataType::Binary, true), // Optional LocationId
        Field::new("metadata", DataType::Utf8, true), // JSON for domain fields
    ]))
}

/// Schema for task storage
///
/// Stores task requirements, status, and assignments in columnar format.
/// Each row represents one task instance.
///
/// # Fields
///
/// * `id` - Binary UUID representing TaskId (16 bytes)
/// * `task_type` - String type identifier matching resource_type for allocation
/// * `status` - Status string: "pending", "allocated", "completed"
/// * `required_capacity` - Capacity units needed from resource
/// * `assigned_resource_id` - Optional ResourceId UUID when allocated
/// * `workflow_instance_id` - Optional WorkflowInstanceId UUID (Phase 5)
/// * `priority` - Task priority (higher = more urgent)
/// * `metadata` - JSON string for domain-specific fields (e.g., duration, deadline)
///
/// # Example
///
/// ```rust,ignore
/// let schema = task_schema();
/// let batch = RecordBatch::try_new(
///     schema,
///     vec![
///         Arc::new(BinaryArray::from(vec![task_id.as_bytes()])),
///         Arc::new(StringArray::from(vec!["kitchen_task"])),
///         Arc::new(StringArray::from(vec!["pending"])),
///         Arc::new(Int64Array::from(vec![5])),
///         Arc::new(BinaryArray::from(vec![None])),
///         Arc::new(BinaryArray::from(vec![Some(workflow_id.as_bytes())])),
///         Arc::new(Int32Array::from(vec![10])),
///         Arc::new(StringArray::from(vec![Some(r#"{"duration_secs":30}"#)])),
///     ],
/// )?;
/// ```
pub fn task_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Binary, false), // TaskId UUID
        Field::new("task_type", DataType::Utf8, false), // Type identifier
        Field::new("status", DataType::Utf8, false), // Pending/Allocated/Complete
        Field::new("required_capacity", DataType::Int64, false), // Capacity needed
        Field::new("assigned_resource_id", DataType::Binary, true), // Optional ResourceId
        Field::new("workflow_instance_id", DataType::Binary, true), // Optional WorkflowInstanceId (Phase 5)
        Field::new("priority", DataType::Int32, false),             // Task priority
        Field::new("metadata", DataType::Utf8, true),               // JSON for domain fields
    ]))
}

/// Task status constants
pub mod task_status {
    /// Task is pending allocation
    pub const PENDING: &str = "pending";
    /// Task is allocated to a resource
    pub const ALLOCATED: &str = "allocated";
    /// Task has completed
    pub const COMPLETED: &str = "completed";
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{BinaryArray, Int32Array, Int64Array, StringArray};
    use arrow::record_batch::RecordBatch;

    #[test]
    fn test_resource_schema_has_correct_fields() {
        let schema = resource_schema();
        assert_eq!(schema.fields().len(), 6);

        assert_eq!(schema.field(0).name(), "id");
        assert_eq!(schema.field(0).data_type(), &DataType::Binary);
        assert!(!schema.field(0).is_nullable());

        assert_eq!(schema.field(1).name(), "resource_type");
        assert_eq!(schema.field(1).data_type(), &DataType::Utf8);

        assert_eq!(schema.field(2).name(), "capacity");
        assert_eq!(schema.field(2).data_type(), &DataType::Int64);

        assert_eq!(schema.field(3).name(), "available");
        assert_eq!(schema.field(3).data_type(), &DataType::Int64);

        assert_eq!(schema.field(4).name(), "location_id");
        assert_eq!(schema.field(4).data_type(), &DataType::Binary);
        assert!(schema.field(4).is_nullable());

        assert_eq!(schema.field(5).name(), "metadata");
        assert_eq!(schema.field(5).data_type(), &DataType::Utf8);
        assert!(schema.field(5).is_nullable());
    }

    #[test]
    fn test_task_schema_has_correct_fields() {
        let schema = task_schema();
        assert_eq!(schema.fields().len(), 8);

        assert_eq!(schema.field(0).name(), "id");
        assert_eq!(schema.field(0).data_type(), &DataType::Binary);
        assert!(!schema.field(0).is_nullable());

        assert_eq!(schema.field(1).name(), "task_type");
        assert_eq!(schema.field(1).data_type(), &DataType::Utf8);

        assert_eq!(schema.field(2).name(), "status");
        assert_eq!(schema.field(2).data_type(), &DataType::Utf8);

        assert_eq!(schema.field(3).name(), "required_capacity");
        assert_eq!(schema.field(3).data_type(), &DataType::Int64);

        assert_eq!(schema.field(4).name(), "assigned_resource_id");
        assert_eq!(schema.field(4).data_type(), &DataType::Binary);
        assert!(schema.field(4).is_nullable());

        assert_eq!(schema.field(5).name(), "workflow_instance_id");
        assert_eq!(schema.field(5).data_type(), &DataType::Binary);
        assert!(schema.field(5).is_nullable());

        assert_eq!(schema.field(6).name(), "priority");
        assert_eq!(schema.field(6).data_type(), &DataType::Int32);

        assert_eq!(schema.field(7).name(), "metadata");
        assert_eq!(schema.field(7).data_type(), &DataType::Utf8);
        assert!(schema.field(7).is_nullable());
    }

    #[test]
    fn test_can_create_resource_record_batch() {
        let schema = resource_schema();

        // Create a simple resource record
        let id_array = BinaryArray::from(vec![vec![0u8; 16].as_slice()]);
        let type_array = StringArray::from(vec!["test_resource"]);
        let capacity_array = Int64Array::from(vec![10]);
        let available_array = Int64Array::from(vec![7]);
        let location_array = BinaryArray::from(vec![None::<&[u8]>]);
        let metadata_array = StringArray::from(vec![Some("{}")]);

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(id_array),
                Arc::new(type_array),
                Arc::new(capacity_array),
                Arc::new(available_array),
                Arc::new(location_array),
                Arc::new(metadata_array),
            ],
        );

        assert!(batch.is_ok());
        let batch = batch.unwrap();
        assert_eq!(batch.num_rows(), 1);
        assert_eq!(batch.num_columns(), 6);
    }

    #[test]
    fn test_can_create_task_record_batch() {
        let schema = task_schema();

        // Create a simple task record
        let id_array = BinaryArray::from(vec![vec![0u8; 16].as_slice()]);
        let type_array = StringArray::from(vec!["test_task"]);
        let status_array = StringArray::from(vec![task_status::PENDING]);
        let capacity_array = Int64Array::from(vec![5]);
        let resource_array = BinaryArray::from(vec![None::<&[u8]>]);
        let workflow_array = BinaryArray::from(vec![None::<&[u8]>]);
        let priority_array = Int32Array::from(vec![0]);
        let metadata_array = StringArray::from(vec![Some("{}")]);

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(id_array),
                Arc::new(type_array),
                Arc::new(status_array),
                Arc::new(capacity_array),
                Arc::new(resource_array),
                Arc::new(workflow_array),
                Arc::new(priority_array),
                Arc::new(metadata_array),
            ],
        );

        assert!(batch.is_ok());
        let batch = batch.unwrap();
        assert_eq!(batch.num_rows(), 1);
        assert_eq!(batch.num_columns(), 8);
    }
}
