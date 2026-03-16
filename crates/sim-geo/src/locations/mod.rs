//! Location management and spatial queries

mod handler;
pub mod schema;
mod traits;

#[cfg(test)]
mod arrow_tests;

pub use handler::LocationHandler;
pub use traits::Location;
