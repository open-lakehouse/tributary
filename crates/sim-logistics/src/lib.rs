//! Logistics Domain Simulation
//!
//! `sim-logistics` implements a complete package delivery simulation using the
//! generic agent-based simulation framework from `sim-core`.
//!
//! # Domain Model
//!
//! ## Agents
//! - **Package**: Passive agent tracking package lifecycle
//! - **Driver**: Active agent handling pickup and delivery
//!
//! ## Resources
//! - **SortingStation**: Package sorting with capacity constraints
//! - **LoadingDock**: Vehicle loading operations
//!
//! ## Workflows
//! - **PackageShipmentWorkflow**: 5-step delivery pipeline
//!
//! ## Locations
//! - **Warehouse**: Package processing centers
//! - **DeliveryAddress**: Final destinations with H3 indexing
//!
//! # Example
//!
//! ```rust,ignore
//! use sim_logistics::LogisticsSimulation;
//!
//! let mut sim = LogisticsSimulation::new();
//! sim.run_for_hours(24)?;
//!
//! let metrics = sim.get_metrics();
//! println!("Packages delivered: {}", metrics.packages_delivered);
//! println!("Driver utilization: {:.1}%", metrics.driver_utilization);
//! ```
//!
//! # Framework Validation
//!
//! This crate validates the framework's generality by implementing a complete
//! logistics simulation with **ZERO modifications** to sim-core, sim-geo, or
//! sim-events.

pub mod agents;
pub mod events;
pub mod locations;
pub mod resources;
pub mod simulation;
pub mod workflows;

#[cfg(test)]
mod lifecycle_invariant_tests;

// Re-export commonly used types
pub use agents::{Driver, DriverState, Package, PackagePriority, PackageState};
pub use events::{
    PackageDeliveredEvent, PackageInTransitEvent, PackageLoadedEvent, PackageReceivedEvent,
    PackageSortedEvent,
};
pub use locations::{DeliveryAddress, Warehouse};
pub use resources::{LoadingDock, SortingStation, VehicleLoadingState};
pub use simulation::{LogisticsMetrics, LogisticsSimulation, LogisticsSimulationHooks};
pub use workflows::{PackageShipmentWorkflow, ShipmentTask, ShipmentTaskFactory};

/// Result type for logistics operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for logistics domain
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("sim-core error: {0}")]
    Core(#[from] sim_core::Error),

    #[error("sim-events error: {0}")]
    Events(#[from] sim_events::Error),

    #[error("Logistics error: {0}")]
    Logistics(String),

    #[error("{0}")]
    Custom(String),
}
