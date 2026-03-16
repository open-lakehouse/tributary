//! Logistics workflow types
//!
//! Workflows define multi-stage processing pipelines for the logistics domain.

pub mod shipment;

pub use shipment::{PackageShipmentWorkflow, ShipmentTask, ShipmentTaskFactory};
