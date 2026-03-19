//! Composable demand generation pipeline

use super::DemandGenerator;
use crate::context::SimulationContext;
use crate::population::Demand;

/// Composite demand generator
///
/// Combines multiple demand generators into a single generator that produces
/// the union of all demands. This enables modeling multiple demand sources
/// simultaneously (e.g., different customer segments, delivery types, etc.).
///
/// # Example
///
/// ```rust,ignore
/// use sim_core::demand::{CompositeDemandGenerator, PoissonDemandGenerator};
///
/// let mut composite = CompositeDemandGenerator::new();
///
/// // Add food orders (10/hour)
/// composite = composite.with_generator(Box::new(
///     PoissonDemandGenerator::new(10.0, "food_order")
/// ));
///
/// // Add grocery orders (5/hour)
/// composite = composite.with_generator(Box::new(
///     PoissonDemandGenerator::new(5.0, "grocery_order")
/// ));
///
/// // Now generates both types of orders
/// ```
pub struct CompositeDemandGenerator {
    /// List of generators to combine
    generators: Vec<Box<dyn DemandGenerator>>,
}

impl CompositeDemandGenerator {
    /// Create a new empty composite generator
    pub fn new() -> Self {
        Self { generators: vec![] }
    }

    /// Add a generator to the pipeline
    ///
    /// Returns self for method chaining.
    pub fn with_generator(mut self, generator: Box<dyn DemandGenerator>) -> Self {
        self.generators.push(generator);
        self
    }

    /// Get the number of generators in the pipeline
    pub fn len(&self) -> usize {
        self.generators.len()
    }

    /// Check if the pipeline is empty
    pub fn is_empty(&self) -> bool {
        self.generators.is_empty()
    }
}

impl Default for CompositeDemandGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl DemandGenerator for CompositeDemandGenerator {
    fn generate(&self, ctx: &SimulationContext) -> Vec<Demand> {
        // Collect demands from all generators
        self.generators
            .iter()
            .flat_map(|g| g.generate(ctx))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::SimulationConfig;
    use std::collections::HashMap;

    // Helper generator that produces a fixed count
    struct FixedCountGenerator {
        count: usize,
        demand_type: String,
    }

    impl DemandGenerator for FixedCountGenerator {
        fn generate(&self, _ctx: &SimulationContext) -> Vec<Demand> {
            (0..self.count)
                .map(|_| Demand {
                    demand_type: self.demand_type.clone(),
                    location: None,
                    metadata: HashMap::new(),
                })
                .collect()
        }
    }

    #[test]
    fn test_composite_creation() {
        let composite = CompositeDemandGenerator::new();
        assert_eq!(composite.len(), 0);
        assert!(composite.is_empty());
    }

    #[test]
    fn test_composite_default() {
        let composite = CompositeDemandGenerator::default();
        assert_eq!(composite.len(), 0);
        assert!(composite.is_empty());
    }

    #[test]
    fn test_empty_pipeline() {
        let composite = CompositeDemandGenerator::new();

        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let demands = composite.generate(&ctx);
        assert_eq!(demands.len(), 0);
    }

    #[test]
    fn test_single_generator() {
        let composite =
            CompositeDemandGenerator::new().with_generator(Box::new(FixedCountGenerator {
                count: 5,
                demand_type: "order".to_string(),
            }));

        assert_eq!(composite.len(), 1);
        assert!(!composite.is_empty());

        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let demands = composite.generate(&ctx);
        assert_eq!(demands.len(), 5);
        assert_eq!(demands[0].demand_type, "order");
    }

    #[test]
    fn test_multiple_generators() {
        let composite = CompositeDemandGenerator::new()
            .with_generator(Box::new(FixedCountGenerator {
                count: 3,
                demand_type: "food_order".to_string(),
            }))
            .with_generator(Box::new(FixedCountGenerator {
                count: 5,
                demand_type: "grocery_order".to_string(),
            }))
            .with_generator(Box::new(FixedCountGenerator {
                count: 2,
                demand_type: "package_delivery".to_string(),
            }));

        assert_eq!(composite.len(), 3);

        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let demands = composite.generate(&ctx);
        assert_eq!(demands.len(), 10); // 3 + 5 + 2

        // Count demands by type
        let food_count = demands
            .iter()
            .filter(|d| d.demand_type == "food_order")
            .count();
        let grocery_count = demands
            .iter()
            .filter(|d| d.demand_type == "grocery_order")
            .count();
        let package_count = demands
            .iter()
            .filter(|d| d.demand_type == "package_delivery")
            .count();

        assert_eq!(food_count, 3);
        assert_eq!(grocery_count, 5);
        assert_eq!(package_count, 2);
    }

    #[test]
    fn test_order_independence() {
        // Verify that demand order is preserved (generators run in order added)
        let composite = CompositeDemandGenerator::new()
            .with_generator(Box::new(FixedCountGenerator {
                count: 2,
                demand_type: "type_a".to_string(),
            }))
            .with_generator(Box::new(FixedCountGenerator {
                count: 2,
                demand_type: "type_b".to_string(),
            }));

        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let demands = composite.generate(&ctx);

        // First 2 should be type_a, next 2 should be type_b
        assert_eq!(demands[0].demand_type, "type_a");
        assert_eq!(demands[1].demand_type, "type_a");
        assert_eq!(demands[2].demand_type, "type_b");
        assert_eq!(demands[3].demand_type, "type_b");
    }

    #[test]
    fn test_with_zero_count_generators() {
        let composite = CompositeDemandGenerator::new()
            .with_generator(Box::new(FixedCountGenerator {
                count: 0,
                demand_type: "none".to_string(),
            }))
            .with_generator(Box::new(FixedCountGenerator {
                count: 3,
                demand_type: "some".to_string(),
            }))
            .with_generator(Box::new(FixedCountGenerator {
                count: 0,
                demand_type: "none2".to_string(),
            }));

        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let demands = composite.generate(&ctx);
        assert_eq!(demands.len(), 3);
        assert!(demands.iter().all(|d| d.demand_type == "some"));
    }
}
