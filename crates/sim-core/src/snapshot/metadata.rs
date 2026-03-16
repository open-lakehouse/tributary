//! Snapshot metadata structures
//!
//! This module provides metadata for simulation snapshots, capturing essential
//! information about the state of a simulation at a specific point in time.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Metadata describing a simulation snapshot
///
/// Contains key statistics and identifying information about the simulation state
/// at the time the snapshot was created. This metadata allows quick inspection of
/// snapshot contents without loading the full data.
///
/// # Example
///
/// ```rust
/// use sim_core::snapshot::SnapshotMetadata;
/// use chrono::Utc;
///
/// let metadata = SnapshotMetadata::new(
///     "1.0".to_string(),
///     100,  // agents
///     50,   // resources
///     200,  // tasks
///     25,   // workflow instances
///     10,   // locations
/// );
///
/// // Serialize to JSON
/// let json = metadata.to_json().unwrap();
/// println!("Snapshot metadata: {}", json);
///
/// // Deserialize from JSON
/// let restored = SnapshotMetadata::from_json(&json).unwrap();
/// assert_eq!(restored.agent_count, 100);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SnapshotMetadata {
    /// Version of the snapshot format (e.g., "1.0")
    ///
    /// This allows for future schema evolution and backward compatibility checking.
    pub version: String,

    /// Timestamp when the snapshot was created (UTC)
    ///
    /// Records the exact moment the snapshot was taken, useful for temporal
    /// analysis and snapshot ordering.
    pub timestamp: DateTime<Utc>,

    /// Number of agents in the snapshot
    ///
    /// Total count of all agents present in the simulation at snapshot time.
    pub agent_count: usize,

    /// Number of resources in the snapshot
    ///
    /// Total count of all resources (capacity-constrained processors) in the simulation.
    pub resource_count: usize,

    /// Number of tasks in the snapshot
    ///
    /// Total count of all tasks, including queued, active, and completed tasks.
    pub task_count: usize,

    /// Number of workflow instances in the snapshot
    ///
    /// Total count of all workflow instances across all stages.
    pub workflow_instance_count: usize,

    /// Number of locations in the snapshot
    ///
    /// Total count of distinct locations referenced in the simulation.
    pub location_count: usize,
}

impl SnapshotMetadata {
    /// Creates a new snapshot metadata instance
    ///
    /// # Arguments
    ///
    /// * `version` - Version string for the snapshot format (e.g., "1.0")
    /// * `agent_count` - Number of agents in the snapshot
    /// * `resource_count` - Number of resources in the snapshot
    /// * `task_count` - Number of tasks in the snapshot
    /// * `workflow_instance_count` - Number of workflow instances in the snapshot
    /// * `location_count` - Number of locations in the snapshot
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_core::snapshot::SnapshotMetadata;
    ///
    /// let metadata = SnapshotMetadata::new(
    ///     "1.0".to_string(),
    ///     100, 50, 200, 25, 10
    /// );
    /// ```
    pub fn new(
        version: String,
        agent_count: usize,
        resource_count: usize,
        task_count: usize,
        workflow_instance_count: usize,
        location_count: usize,
    ) -> Self {
        Self {
            version,
            timestamp: Utc::now(),
            agent_count,
            resource_count,
            task_count,
            workflow_instance_count,
            location_count,
        }
    }

    /// Serializes the metadata to a JSON string
    ///
    /// # Returns
    ///
    /// A `Result` containing the JSON string representation, or a serialization error.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails (rare, as all fields are serializable).
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_core::snapshot::SnapshotMetadata;
    ///
    /// let metadata = SnapshotMetadata::new(
    ///     "1.0".to_string(),
    ///     100, 50, 200, 25, 10
    /// );
    ///
    /// let json = metadata.to_json().expect("Failed to serialize");
    /// println!("Metadata: {}", json);
    /// ```
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserializes metadata from a JSON string
    ///
    /// # Arguments
    ///
    /// * `json` - JSON string containing serialized metadata
    ///
    /// # Returns
    ///
    /// A `Result` containing the deserialized metadata, or a deserialization error.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The JSON is malformed
    /// - Required fields are missing
    /// - Field types don't match expected types
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_core::snapshot::SnapshotMetadata;
    ///
    /// let json = r#"{
    ///     "version": "1.0",
    ///     "timestamp": "2024-01-15T10:30:00Z",
    ///     "agent_count": 100,
    ///     "resource_count": 50,
    ///     "task_count": 200,
    ///     "workflow_instance_count": 25,
    ///     "location_count": 10
    /// }"#;
    ///
    /// let metadata = SnapshotMetadata::from_json(json).expect("Failed to deserialize");
    /// assert_eq!(metadata.agent_count, 100);
    /// ```
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_metadata() {
        let metadata = SnapshotMetadata::new("1.0".to_string(), 100, 50, 200, 25, 10);

        assert_eq!(metadata.version, "1.0");
        assert_eq!(metadata.agent_count, 100);
        assert_eq!(metadata.resource_count, 50);
        assert_eq!(metadata.task_count, 200);
        assert_eq!(metadata.workflow_instance_count, 25);
        assert_eq!(metadata.location_count, 10);
    }

    #[test]
    fn test_json_roundtrip() {
        let original = SnapshotMetadata::new("1.0".to_string(), 100, 50, 200, 25, 10);

        let json = original.to_json().expect("Failed to serialize");
        let restored = SnapshotMetadata::from_json(&json).expect("Failed to deserialize");

        assert_eq!(original.version, restored.version);
        assert_eq!(original.agent_count, restored.agent_count);
        assert_eq!(original.resource_count, restored.resource_count);
        assert_eq!(original.task_count, restored.task_count);
        assert_eq!(
            original.workflow_instance_count,
            restored.workflow_instance_count
        );
        assert_eq!(original.location_count, restored.location_count);
    }

    #[test]
    fn test_timestamp_is_set() {
        let metadata = SnapshotMetadata::new("1.0".to_string(), 0, 0, 0, 0, 0);

        // Timestamp should be recent (within last second)
        let now = Utc::now();
        let diff = now.signed_duration_since(metadata.timestamp);
        assert!(diff.num_seconds() < 2);
    }

    #[test]
    fn test_from_json_invalid() {
        let invalid_json = r#"{"not": "valid"}"#;
        let result = SnapshotMetadata::from_json(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_json_format() {
        let metadata = SnapshotMetadata::new("1.0".to_string(), 100, 50, 200, 25, 10);

        let json = metadata.to_json().expect("Failed to serialize");

        // Should contain all fields
        assert!(json.contains("version"));
        assert!(json.contains("timestamp"));
        assert!(json.contains("agent_count"));
        assert!(json.contains("resource_count"));
        assert!(json.contains("task_count"));
        assert!(json.contains("workflow_instance_count"));
        assert!(json.contains("location_count"));
    }
}
