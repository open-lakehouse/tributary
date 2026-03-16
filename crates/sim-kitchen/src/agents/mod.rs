//! Agent implementations for the ghost kitchen simulation
//!
//! This module contains all agent types that participate in the simulation:
//! - Customers: Place orders and consume food
//! - Couriers: Deliver prepared food orders from kitchens to customers
//! - Supply Drivers: Transport inventory from distribution sites to kitchens

pub mod courier;
pub mod customer;
pub mod supply_driver;

pub use courier::{Courier, CourierState};
pub use customer::{Customer, CustomerStatus};
pub use supply_driver::{DriverState, SupplyDriver};
