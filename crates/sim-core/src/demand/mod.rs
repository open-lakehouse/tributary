//! Demand generation
//!
//! This module provides traits and models for generating demand in simulations.

use crate::context::SimulationContext;
use crate::population::Demand;
use crate::Result;

/// Generator for creating demands
///
/// Implement this trait to define how demand is generated in your simulation.
///
/// # Example
///
/// ```rust,ignore
/// struct PoissonOrderGenerator {
///     lambda: f64, // Average rate per hour
/// }
///
/// impl DemandGenerator for PoissonOrderGenerator {
///     fn generate(&mut self, ctx: &SimulationContext) -> Result<Vec<Demand>> {
///         let num_orders = sample_poisson(self.lambda, ctx.time_step());
///         // Create demands...
///         Ok(demands)
///     }
/// }
/// ```
pub trait DemandGenerator: Send + Sync {
    /// Generate demands for this simulation step
    ///
    /// # Arguments
    ///
    /// * `ctx` - Simulation context with current time
    ///
    /// # Returns
    ///
    /// Vector of demand requests to be processed
    fn generate(&mut self, ctx: &SimulationContext) -> Result<Vec<Demand>>;
}
