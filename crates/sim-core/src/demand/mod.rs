//! Demand generation
//!
//! This module provides traits and models for generating demand in simulations.
//! It includes probabilistic demand generators with time-based and spatial patterns.

mod composite;
mod poisson;
mod spatial;
mod time_of_day;

pub use composite::CompositeDemandGenerator;
pub use poisson::PoissonDemandGenerator;
pub use spatial::SpatialDemandGenerator;
pub use time_of_day::TimeOfDayDemandGenerator;

use crate::context::SimulationContext;
use crate::population::Demand;

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
///     fn generate(&self, ctx: &SimulationContext) -> Vec<Demand> {
///         let num_orders = sample_poisson(self.lambda, ctx.time_step());
///         // Create demands...
///         demands
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
    fn generate(&self, ctx: &SimulationContext) -> Vec<Demand>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::SimulationConfig;
    use geo::Point;
    use std::collections::HashMap;

    // Test that DemandGenerator trait is object-safe
    #[test]
    fn test_demand_generator_trait_object_safety() {
        struct TestGenerator;
        impl DemandGenerator for TestGenerator {
            fn generate(&self, _ctx: &SimulationContext) -> Vec<Demand> {
                vec![]
            }
        }

        let _generator: Box<dyn DemandGenerator> = Box::new(TestGenerator);
    }

    // Test basic demand creation
    #[test]
    fn test_basic_demand_creation() {
        let demand = Demand {
            demand_type: "test_order".to_string(),
            location: Some(Point::new(0.0, 0.0)),
            metadata: HashMap::new(),
        };

        assert_eq!(demand.demand_type, "test_order");
        assert!(demand.location.is_some());
        assert_eq!(demand.metadata.len(), 0);
    }

    // Test demand with metadata
    #[test]
    fn test_demand_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "customer_id".to_string(),
            serde_json::Value::String("cust_123".to_string()),
        );
        metadata.insert("priority".to_string(), serde_json::Value::Number(5.into()));

        let demand = Demand {
            demand_type: "priority_order".to_string(),
            location: None,
            metadata,
        };

        assert_eq!(demand.metadata.len(), 2);
        assert_eq!(
            demand.metadata.get("customer_id").unwrap(),
            &serde_json::Value::String("cust_123".to_string())
        );
    }

    // Test that generator can be used in simulation context
    #[test]
    fn test_generator_with_context() {
        struct SimpleGenerator;
        impl DemandGenerator for SimpleGenerator {
            fn generate(&self, ctx: &SimulationContext) -> Vec<Demand> {
                vec![Demand {
                    demand_type: format!("order_at_step_{}", ctx.step()),
                    location: None,
                    metadata: HashMap::new(),
                }]
            }
        }

        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);
        let generator = SimpleGenerator;

        let demands = generator.generate(&ctx);
        assert_eq!(demands.len(), 1);
        assert_eq!(demands[0].demand_type, "order_at_step_0");
    }

    // Test empty generator
    #[test]
    fn test_empty_generator() {
        struct EmptyGenerator;
        impl DemandGenerator for EmptyGenerator {
            fn generate(&self, _ctx: &SimulationContext) -> Vec<Demand> {
                vec![]
            }
        }

        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);
        let generator = EmptyGenerator;

        let demands = generator.generate(&ctx);
        assert_eq!(demands.len(), 0);
    }
}
