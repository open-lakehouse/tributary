//! Agents in the logistics simulation
//!
//! - [`Package`]: a passive agent whose stage follows its shipment workflow

pub mod package;

pub use package::{Package, PackagePriority, PackageStage};
