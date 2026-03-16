//! Ghost Kitchen Simulation
//!
//! This crate demonstrates how to build a domain-specific simulation using the
//! sim-core, sim-geo, and sim-events framework.
//!
//! # Domain Model
//!
//! - **Customers**: Agents that order food, wait, eat, and leave
//! - **Kitchens**: Resources that prepare orders
//! - **Orders**: Tasks assigned to kitchens
//! - **Events**: CloudEvents for all state changes
//!
//! # Example
//!
//! ```rust,ignore
//! use sim_kitchen::{build_simulation, add_customers};
//!
//! let mut runner = build_simulation(3, 5, true);
//! add_customers(&mut runner, 20);
//! runner.run_for_steps(100)?;
//! ```

pub mod agents;
pub mod events;
pub mod resources;
pub mod simulation;

pub use agents::{Customer, CustomerStatus};
pub use events::*;
pub use resources::{Kitchen, Order};
pub use simulation::{
    KitchenSimulationHooks, NoWorkflow, add_customers, build_simulation, process_demands,
};
