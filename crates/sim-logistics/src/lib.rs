//! Logistics Domain Simulation
//!
//! `sim-logistics` implements a complete package delivery simulation using the
//! generic agent-based simulation framework from `sim-core`.
//!
//! # Domain Model
//!
//! - **Package** agents: passive; their stage (`created → received → sorted →
//!   in_transit → delivered`) follows their shipment workflow
//! - **Station** resources: receivers, sorting stations, loading docks,
//!   drivers (capacity 1 each) and delivery confirmers
//! - **PackageShipmentWorkflow**: receive → sort (5 min) → load (6 min) →
//!   transit (30 min) → confirm (1 min), one instance per package
//! - **Warehouse** / **DeliveryAddress** locations (H3-indexed)
//!
//! # Example
//!
//! ```rust,no_run
//! use sim_logistics::LogisticsSimulation;
//!
//! let mut sim = LogisticsSimulation::new();
//! sim.run_for_hours(24)?;
//!
//! let metrics = sim.get_metrics();
//! println!("Packages delivered: {}", metrics.packages_delivered);
//! println!("Driver utilization: {:.1}%", metrics.driver_utilization_percent);
//! # Ok::<(), sim_logistics::Error>(())
//! ```
//!
//! # Framework Validation
//!
//! This crate exercises the framework from a second domain (agents, stations,
//! workflows, demand generation) without domain code in sim-core.

pub mod agents;
pub mod events;
pub mod locations;
pub mod resources;
pub mod simulation;
pub mod workflows;

pub use agents::{Package, PackagePriority, PackageStage};
pub use events::{
    PackageDeliveredEvent, PackageInTransitEvent, PackageLoadedEvent, PackageReceivedEvent,
    PackageSortedEvent,
};
pub use locations::{DeliveryAddress, Warehouse};
pub use resources::{Station, StationKind};
pub use simulation::{
    LogisticsMetrics, LogisticsRunner, LogisticsSimulation, LogisticsSimulationHooks,
    SHIPMENT_ENGINE,
};
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
