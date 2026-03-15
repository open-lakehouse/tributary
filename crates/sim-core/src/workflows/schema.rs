//! Arrow schema definitions for workflow instances
//!
//! This module defines the Apache Arrow schema for storing workflow instance state
//! in a columnar format, enabling efficient querying via DataFusion.

use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use std::sync::Arc;

/// Arrow schema for workflow instances
///
/// This schema stores workflow instance state in a columnar format optimized for
/// batch queries via DataFusion. Each row represents a single workflow instance.
///
/// # Schema Fields
///
/// - `id`: Binary (16 bytes) - WorkflowInstanceId UUID
/// - `workflow_id`: Binary (16 bytes) - WorkflowId UUID
/// - `status`: Utf8 - Instance status (Running/Paused/Complete/Failed)
/// - `current_step_index`: Int64 - Current step in workflow (0-based)
/// - `total_steps`: Int64 - Total number of steps in workflow
/// - `agent_id`: Binary (16 bytes, nullable) - Optional AgentId assigned to instance
/// - `started_at`: Int64 - Simulation tick when instance started
/// - `completed_at`: Int64 (nullable) - Simulation tick when instance completed
/// - `context`: Utf8 (nullable) - JSON-encoded workflow context data
/// - `metadata`: Utf8 (nullable) - JSON-encoded domain-specific metadata
///
/// # Storage Format
///
/// - UUIDs are stored as 16-byte binary arrays (native UUID format)
/// - Timestamps are stored as Int64 tick counts for efficient range queries
/// - JSON fields (context, metadata) stored as UTF-8 strings for flexibility
/// - Status is stored as string for readability (could be optimized to int8)
///
/// # Performance Characteristics
///
/// - Columnar storage enables efficient filtering on status, agent_id, etc.
/// - Binary UUID storage (16 bytes) is more compact than string (36 bytes)
/// - Nullable fields only store values when present (no storage overhead for None)
/// - DataFusion can push down predicates for optimal query performance
///
/// # Example Usage
///
/// ```rust,ignore
/// use arrow::array::*;
/// use arrow::record_batch::RecordBatch;
///
/// let schema = workflow_instance_schema();
///
/// // Create empty batch
/// let batch = RecordBatch::new_empty(schema.clone());
///
/// // Query with DataFusion
/// let ctx = SessionContext::new();
/// let provider = MemTable::try_new(schema, vec![vec![batch]])?;
/// ctx.register_table("instances", Arc::new(provider))?;
///
/// let df = ctx.sql("SELECT id FROM instances WHERE status = 'Running'").await?;
/// ```
pub fn workflow_instance_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Binary, false),
        Field::new("workflow_id", DataType::Binary, false),
        Field::new("status", DataType::Utf8, false),
        Field::new("current_step_index", DataType::Int64, false),
        Field::new("total_steps", DataType::Int64, false),
        Field::new("agent_id", DataType::Binary, true),
        Field::new("started_at", DataType::Int64, false),
        Field::new("completed_at", DataType::Int64, true),
        Field::new("context", DataType::Utf8, true),
        Field::new("metadata", DataType::Utf8, true),
    ]))
}

/// Workflow instance status values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceStatus {
    /// Instance is actively running
    Running,
    /// Instance is paused and not processing
    Paused,
    /// Instance has completed successfully
    Complete,
    /// Instance has failed
    Failed,
}

impl InstanceStatus {
    /// Convert status to string for Arrow storage
    pub fn as_str(&self) -> &'static str {
        match self {
            InstanceStatus::Running => "Running",
            InstanceStatus::Paused => "Paused",
            InstanceStatus::Complete => "Complete",
            InstanceStatus::Failed => "Failed",
        }
    }

    /// Parse status from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Running" => Some(InstanceStatus::Running),
            "Paused" => Some(InstanceStatus::Paused),
            "Complete" => Some(InstanceStatus::Complete),
            "Failed" => Some(InstanceStatus::Failed),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_creation() {
        let schema = workflow_instance_schema();

        assert_eq!(schema.fields().len(), 10);
        assert_eq!(schema.field(0).name(), "id");
        assert_eq!(schema.field(1).name(), "workflow_id");
        assert_eq!(schema.field(2).name(), "status");
        assert_eq!(schema.field(3).name(), "current_step_index");
        assert_eq!(schema.field(4).name(), "total_steps");
        assert_eq!(schema.field(5).name(), "agent_id");
        assert_eq!(schema.field(6).name(), "started_at");
        assert_eq!(schema.field(7).name(), "completed_at");
        assert_eq!(schema.field(8).name(), "context");
        assert_eq!(schema.field(9).name(), "metadata");
    }

    #[test]
    fn test_schema_field_types() {
        let schema = workflow_instance_schema();

        // UUIDs stored as binary
        assert!(matches!(schema.field(0).data_type(), DataType::Binary));
        assert!(matches!(schema.field(1).data_type(), DataType::Binary));

        // Status is string
        assert!(matches!(schema.field(2).data_type(), DataType::Utf8));

        // Counters are Int64
        assert!(matches!(schema.field(3).data_type(), DataType::Int64));
        assert!(matches!(schema.field(4).data_type(), DataType::Int64));
    }

    #[test]
    fn test_schema_nullability() {
        let schema = workflow_instance_schema();

        // Required fields
        assert!(!schema.field(0).is_nullable()); // id
        assert!(!schema.field(1).is_nullable()); // workflow_id
        assert!(!schema.field(2).is_nullable()); // status
        assert!(!schema.field(3).is_nullable()); // current_step_index
        assert!(!schema.field(4).is_nullable()); // total_steps
        assert!(!schema.field(6).is_nullable()); // started_at

        // Optional fields
        assert!(schema.field(5).is_nullable()); // agent_id
        assert!(schema.field(7).is_nullable()); // completed_at
        assert!(schema.field(8).is_nullable()); // context
        assert!(schema.field(9).is_nullable()); // metadata
    }

    #[test]
    fn test_instance_status_str_conversion() {
        assert_eq!(InstanceStatus::Running.as_str(), "Running");
        assert_eq!(InstanceStatus::Paused.as_str(), "Paused");
        assert_eq!(InstanceStatus::Complete.as_str(), "Complete");
        assert_eq!(InstanceStatus::Failed.as_str(), "Failed");
    }

    #[test]
    fn test_instance_status_parsing() {
        assert_eq!(InstanceStatus::from_str("Running"), Some(InstanceStatus::Running));
        assert_eq!(InstanceStatus::from_str("Paused"), Some(InstanceStatus::Paused));
        assert_eq!(InstanceStatus::from_str("Complete"), Some(InstanceStatus::Complete));
        assert_eq!(InstanceStatus::from_str("Failed"), Some(InstanceStatus::Failed));
        assert_eq!(InstanceStatus::from_str("Invalid"), None);
    }
}
