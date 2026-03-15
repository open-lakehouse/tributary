//! Agent lifecycle management
//!
//! This module provides the core abstractions for agents in the simulation and a generic
//! population handler for managing agent state and behavior.

mod handler;
mod traits;

pub use handler::PopulationHandler;
pub use traits::{Agent, AgentStatus, AgentTickResult, Demand};
