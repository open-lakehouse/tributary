//! Simulation snapshot management
//!
//! This module provides functionality for capturing, storing, and restoring snapshots
//! of simulation state. Snapshots enable checkpointing, replay, and analysis of
//! simulation runs.
//!
//! # Overview
//!
//! The snapshot system consists of:
//!
//! - **Metadata**: Descriptive information about snapshot contents and timing
//! - **Manager**: Coordinates snapshot creation and restoration (to be implemented)
//!
//! # Architecture
//!
//! Snapshots leverage Apache Arrow's columnar format for efficient serialization and
//! deserialization. The columnar layout provides:
//!
//! - Zero-copy serialization to Parquet format
//! - Efficient compression of snapshot data
//! - Fast partial restoration (load only needed columns)
//! - Schema evolution support for backward compatibility
//!
//! # Example
//!
//! ```rust
//! use sim_core::snapshot::SnapshotMetadata;
//!
//! // Create metadata for a snapshot
//! let metadata = SnapshotMetadata::new(
//!     "1.0".to_string(),
//!     100,  // agents
//!     50,   // resources
//!     200,  // tasks
//!     25,   // workflow instances
//!     10,   // locations
//! );
//!
//! // Serialize to JSON
//! let json = metadata.to_json().unwrap();
//! println!("Snapshot captured at: {}", metadata.timestamp);
//! ```
//!
//! # Future Extensions
//!
//! The snapshot system is designed to support:
//!
//! - Incremental snapshots (delta compression)
//! - Parallel snapshot I/O
//! - Distributed snapshot storage
//! - Snapshot validation and integrity checking
//! - Time-travel queries across snapshots

mod manager;
mod metadata;

// Public exports
pub use manager::{SnapshotBackend, SnapshotData, SnapshotManager};
pub use metadata::SnapshotMetadata;
