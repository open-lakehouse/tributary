//! Location management for supply chain simulations
//!
//! This module provides location types for modeling supply chain operations,
//! including distribution sites that serve as sources of inventory.

mod distribution_site;

pub use distribution_site::{DistributionSite, OrderFulfillment};
