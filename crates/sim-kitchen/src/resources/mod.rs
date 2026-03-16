//! Resource management for ghost kitchen simulation
//!
//! This module provides resource types for the ghost kitchen domain:
//!
//! - **inventory**: Track ingredients and supplies with consumption/replenishment
//! - **kitchen_order**: Kitchen resources and Order tasks

pub mod inventory;
pub mod kitchen_order;

pub use inventory::{Inventory, InventoryError, InventoryManager};
pub use kitchen_order::{Kitchen, Order};
