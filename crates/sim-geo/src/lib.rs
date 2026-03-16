//! Geospatial and Movement Abstractions for Agent-Based Simulations
//!
//! `sim-geo` provides geospatial capabilities including:
//! - Journey planning and tracking with road network routing
//! - Location management with H3 spatial indexing
//! - Proximity queries and spatial joins
//!
//! This crate integrates with `sim-core` to add movement capabilities to agents.

pub mod graph;
pub mod locations;
pub mod movement;
pub mod spatial;

// Re-export commonly used types
pub use graph::{EdgeId, NodeId, RoadEdge, RoadNetwork, RoadNode, RoadType};
pub use locations::{Location, LocationHandler};
pub use movement::{Journey, JourneyState, MovementHandler, VelocityModel};
pub use spatial::H3Index;

/// Result type used throughout the crate
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for the geospatial module
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("sim-core error: {0}")]
    Core(#[from] sim_core::Error),

    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("DataFusion error: {0}")]
    DataFusion(#[from] datafusion::error::DataFusionError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Routing error: {0}")]
    Routing(String),

    #[error("H3 error: {0}")]
    H3(String),

    #[error("Location error: {0}")]
    Location(String),

    #[error("Journey error: {0}")]
    Journey(String),

    #[error("{0}")]
    Custom(String),
}
