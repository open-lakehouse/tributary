//! Recipe types and registry for the ghost kitchen simulation

pub mod registry;
pub mod types;

pub use registry::RecipeRegistry;
pub use types::{Recipe, RecipeStep, StationKind};
