//! Workflow definitions for the kitchen simulation
//!
//! This module contains workflow implementations for various kitchen operations:
//!
//! - **Supply Chain**: Inventory ordering and delivery from distribution sites
//! - **Courier Delivery**: Food order pickup and customer delivery
//! - **Food Orders**: (Future) Customer order preparation and delivery
//!
//! Each workflow defines a multi-step process with specific resource requirements
//! and task generation logic.

pub mod courier_delivery;
pub mod supply_chain;

pub use courier_delivery::{CourierDeliveryTask, CourierDeliveryTaskFactory, CourierDeliveryWorkflow};
pub use supply_chain::{SupplyChainTaskFactory, SupplyChainWorkflow, SupplyDeliveryTask};
