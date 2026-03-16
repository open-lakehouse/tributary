//! Generic Agent-Based Simulation Framework
//!
//! `sim-core` provides the core abstractions and runtime for building agent-based simulations.
//! It is designed to be domain-agnostic, allowing you to model various scenarios including
//! logistics, retail, manufacturing, services, and more.
//!
//! # Core Concepts
//!
//! - **Agents**: Entities that exist in the simulation world and have behavior
//! - **Resources**: Capacity-constrained processors that handle tasks
//! - **Workflows**: Multi-stage processing pipelines
//! - **Context**: Simulation time management and configuration
//!
//! # Example
//!
//! ```rust,ignore
//! use sim_core::{Agent, SimulationContext, PopulationHandler};
//!
//! // Define your agent type
//! struct MyAgent { /* ... */ }
//! impl Agent for MyAgent { /* ... */ }
//!
//! // Create simulation
//! let ctx = SimulationContext::new(/* ... */);
//! let mut population = PopulationHandler::<MyAgent>::new();
//!
//! // Run simulation
//! population.tick_agents(&ctx)?;
//! ```

pub mod context;
pub mod demand;
pub mod population;
pub mod resources;
pub mod runner;
pub mod snapshot;
pub mod utils;
pub mod workflows;

// Re-export commonly used types
pub use context::{SimulationConfig, SimulationContext};
pub use demand::{
    CompositeDemandGenerator, DemandGenerator, PoissonDemandGenerator, SpatialDemandGenerator,
    TimeOfDayDemandGenerator,
};
pub use population::{Agent, AgentStatus, Demand, PopulationHandler};
pub use resources::{AllocationStrategy, Resource, ResourceAllocator, Task};
pub use runner::{NoOpHooks, SimulationHooks, SimulationRunner, SimulationStats};
pub use snapshot::{SnapshotBackend, SnapshotData, SnapshotManager, SnapshotMetadata};
pub use utils::idents::{AgentId, LocationId, ResourceId, TaskId, WorkflowId};
pub use workflows::{Workflow, WorkflowEngine, WorkflowStep};

/// Result type used throughout the crate
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for the simulation framework
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("DataFusion error: {0}")]
    DataFusion(#[from] datafusion::error::DataFusionError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("Resource error: {0}")]
    Resource(String),

    #[error("Workflow error: {0}")]
    Workflow(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("{0}")]
    Custom(String),
}
