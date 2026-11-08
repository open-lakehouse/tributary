//! Simulation snapshot management
//!
//! This module provides functionality for capturing, storing, and restoring snapshots
//! of simulation state. Snapshots enable checkpointing, replay, and analysis of
//! simulation runs.
//!
//! # Overview
//!
//! - [`SnapshotManager`] stores [`SnapshotData`] (one Arrow batch per entity
//!   type plus the simulation clock) in memory or as Parquet files.
//! - [`SnapshotMetadata`] describes a stored snapshot.
//!
//! Most callers go through
//! [`SimulationRunner::save_snapshot`](crate::SimulationRunner::save_snapshot)
//! and [`SimulationRunner::restore_snapshot`](crate::SimulationRunner::restore_snapshot),
//! which capture and restore agents, resources, tasks, workflow instances and
//! the clock together.

mod hook_state;
mod manager;
mod metadata;

// Public exports
pub use arrow::array::RecordBatch;
pub use hook_state::{hook_state_batch, hook_state_from_batch};
pub use manager::{SnapshotBackend, SnapshotClock, SnapshotData, SnapshotManager};
pub use metadata::SnapshotMetadata;
