//! Workflow definitions for the kitchen simulation
//!
//! - [`FoodPrepWorkflow`]: prep → cook → package, all in one kitchen

pub mod food_prep;

pub use food_prep::FoodPrepWorkflow;
