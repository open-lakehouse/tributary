//! Logistics resource types
//!
//! Resources represent capacity-constrained processors in the logistics system:
//! - SortingStation: Package sorting and routing
//! - LoadingDock: Vehicle loading operations

pub mod loading;
pub mod sorting;

pub use loading::{LoadingDock, VehicleLoadingState};
pub use sorting::SortingStation;
