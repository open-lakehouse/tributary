//! Snapshot metadata structures
//!
//! This module provides metadata for simulation snapshots, capturing essential
//! information about the state of a simulation at a specific point in time.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Metadata describing a simulation snapshot
///
/// Contains key statistics and identifying information about the simulation state
/// at the time the snapshot was created. This metadata allows quick inspection of
/// snapshot contents without loading the full data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SnapshotMetadata {
    /// Unique ID for this snapshot (Uuid v7)
    pub snapshot_id: Uuid,

    /// ID of the simulation this snapshot belongs to (Uuid v7)
    pub simulation_id: Uuid,

    /// Human-readable label (optional)
    pub label: Option<String>,

    /// Simulation clock time when the snapshot was taken
    pub sim_time: DateTime<Utc>,

    /// Version of the snapshot format (e.g., "1.0")
    pub version: String,

    /// Wall-clock timestamp when the snapshot was created (UTC)
    pub timestamp: DateTime<Utc>,

    /// Number of agents in the snapshot
    pub agent_count: usize,

    /// Number of resources in the snapshot
    pub resource_count: usize,

    /// Number of tasks in the snapshot
    pub task_count: usize,

    /// Number of workflow instances in the snapshot
    pub workflow_instance_count: usize,

    /// Number of locations in the snapshot
    pub location_count: usize,
}

impl SnapshotMetadata {
    /// Creates a snapshot metadata instance with caller-supplied IDs.
    #[allow(clippy::too_many_arguments)]
    pub fn with_ids(
        snapshot_id: Uuid,
        simulation_id: Uuid,
        label: Option<String>,
        sim_time: DateTime<Utc>,
        version: String,
        agent_count: usize,
        resource_count: usize,
        task_count: usize,
        workflow_instance_count: usize,
        location_count: usize,
    ) -> Self {
        Self {
            snapshot_id,
            simulation_id,
            label,
            sim_time,
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
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserializes metadata from a JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SnapshotMetadata {
        SnapshotMetadata::with_ids(
            Uuid::now_v7(),
            Uuid::now_v7(),
            None,
            Utc::now(),
            "1.0".to_string(),
            100,
            50,
            200,
            25,
            10,
        )
    }

    #[test]
    fn test_with_ids() {
        let snap_id = Uuid::now_v7();
        let sim_id = Uuid::now_v7();
        let sim_time = Utc::now();
        let metadata = SnapshotMetadata::with_ids(
            snap_id,
            sim_id,
            Some("my_label".to_string()),
            sim_time,
            "1.0".to_string(),
            10,
            5,
            20,
            3,
            0,
        );
        assert_eq!(metadata.snapshot_id, snap_id);
        assert_eq!(metadata.simulation_id, sim_id);
        assert_eq!(metadata.label.as_deref(), Some("my_label"));
    }

    #[test]
    fn test_json_roundtrip() {
        let original = sample();

        let json = original.to_json().expect("Failed to serialize");
        let restored = SnapshotMetadata::from_json(&json).expect("Failed to deserialize");

        assert_eq!(original.version, restored.version);
        assert_eq!(original.agent_count, restored.agent_count);
        assert_eq!(original.snapshot_id, restored.snapshot_id);
        assert_eq!(original.simulation_id, restored.simulation_id);
    }

    #[test]
    fn test_timestamp_is_set() {
        let metadata = sample();

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
    fn test_json_contains_new_fields() {
        let metadata = sample();

        let json = metadata.to_json().expect("Failed to serialize");

        assert!(json.contains("snapshot_id"));
        assert!(json.contains("simulation_id"));
        assert!(json.contains("sim_time"));
        assert!(json.contains("version"));
        assert!(json.contains("agent_count"));
    }
}
