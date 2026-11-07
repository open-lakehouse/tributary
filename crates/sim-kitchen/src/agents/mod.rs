//! Agent implementations for the ghost kitchen simulation
//!
//! - [`Customer`]: places orders, waits for food, eats, and orders again

pub mod customer;

pub use customer::{Customer, CustomerStatus};
