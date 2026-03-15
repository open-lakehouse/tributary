//! Agent lifecycle management
//!
//! This module provides the core abstractions for agents in the simulation and a generic
//! population handler for managing agent state and behavior.

mod handler;
mod schema;
mod traits;

#[cfg(test)]
mod arrow_tests;

pub use handler::PopulationHandler;
pub use schema::agent_schema;
pub use traits::{Agent, AgentStatus, AgentTickResult, ArrowAgentRow, Demand};
