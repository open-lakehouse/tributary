//! Logistics agent types
//!
//! This module contains agent implementations for the logistics domain:
//! - Package: Passive agent tracking package lifecycle
//! - Driver: Active agent handling package pickup and delivery

pub mod driver;
pub mod package;

pub use driver::{Driver, DriverState};
pub use package::{Package, PackagePriority, PackageState};
