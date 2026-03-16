//! Spatial demand generator with H3-based geographic clustering

use super::DemandGenerator;
use crate::context::SimulationContext;
use crate::population::Demand;
use geo::Point;
use h3o::{CellIndex, LatLng};
use rand::Rng;
use std::collections::HashMap;

/// Spatial demand generator
///
/// Wraps a base demand generator and assigns geographic locations to demands
/// using H3 hexagonal grid cells with probability weights. This enables
/// modeling of spatial demand patterns and clustering.
///
/// # Example
///
/// ```rust,ignore
/// use sim_core::demand::{PoissonDemandGenerator, SpatialDemandGenerator};
/// use h3o::CellIndex;
/// use std::collections::HashMap;
///
/// // Base generator
/// let base = Box::new(PoissonDemandGenerator::new(10.0, "order"));
///
/// // Define spatial distribution
/// let mut h3_probs = HashMap::new();
/// h3_probs.insert(downtown_cell, 0.5);  // 50% downtown
/// h3_probs.insert(suburbs_cell, 0.3);   // 30% suburbs
/// h3_probs.insert(airport_cell, 0.2);   // 20% airport
///
/// let generator = SpatialDemandGenerator::new(base, h3_probs);
/// ```
pub struct SpatialDemandGenerator {
    /// Base demand generator to wrap
    pub base_generator: Box<dyn DemandGenerator>,
    /// Probability weights for H3 cells
    pub h3_probabilities: HashMap<CellIndex, f64>,
}

impl SpatialDemandGenerator {
    /// Create a new spatial demand generator
    ///
    /// # Arguments
    ///
    /// * `base_generator` - Base generator to add spatial distribution to
    /// * `h3_probabilities` - Map of H3 cells to probability weights
    ///
    /// # Panics
    ///
    /// Panics if any probability is negative or if probabilities map is empty
    pub fn new(
        base_generator: Box<dyn DemandGenerator>,
        h3_probabilities: HashMap<CellIndex, f64>,
    ) -> Self {
        assert!(!h3_probabilities.is_empty(), "Must have at least one H3 cell");

        // Validate probabilities are non-negative
        for (cell, &prob) in &h3_probabilities {
            assert!(
                prob >= 0.0,
                "Probability for cell {:?} must be non-negative, got {}",
                cell,
                prob
            );
        }

        Self {
            base_generator,
            h3_probabilities,
        }
    }

    /// Sample an H3 cell using weighted random selection
    fn sample_h3_cell(&self, rng: &mut impl Rng) -> CellIndex {
        // Calculate total weight
        let total: f64 = self.h3_probabilities.values().sum();

        // Handle edge case of all-zero probabilities
        if total == 0.0 {
            return *self.h3_probabilities.keys().next().unwrap();
        }

        // Weighted random selection
        #[allow(deprecated)]
        let mut target = rng.r#gen::<f64>() * total;

        for (cell, prob) in &self.h3_probabilities {
            target -= prob;
            if target <= 0.0 {
                return *cell;
            }
        }

        // Fallback (should rarely happen due to floating point)
        *self.h3_probabilities.keys().next().unwrap()
    }

    /// Convert H3 cell to a geographic point (cell center)
    fn h3_cell_to_point(cell: CellIndex) -> Point<f64> {
        let latlng: LatLng = cell.into();
        Point::new(latlng.lng(), latlng.lat())
    }
}

impl DemandGenerator for SpatialDemandGenerator {
    fn generate(&self, ctx: &SimulationContext) -> Vec<Demand> {
        // Generate base demands
        let mut demands = self.base_generator.generate(ctx);

        // Add spatial locations
        let mut rng = rand::thread_rng();

        for demand in &mut demands {
            let cell = self.sample_h3_cell(&mut rng);
            demand.location = Some(Self::h3_cell_to_point(cell));
        }

        demands
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::SimulationConfig;
    use approx::assert_relative_eq;
    use h3o::Resolution;

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

    fn create_test_cell(lat: f64, lng: f64, resolution: Resolution) -> CellIndex {
        let latlng = LatLng::new(lat, lng).expect("Invalid lat/lng");
        latlng.to_cell(resolution)
    }

    #[test]
    fn test_spatial_generator_creation() {
        let base = Box::new(FixedCountGenerator {
            count: 10,
            demand_type: "order".to_string(),
        });

        let mut h3_probs = HashMap::new();
        h3_probs.insert(create_test_cell(37.7749, -122.4194, Resolution::Nine), 1.0);

        let generator = SpatialDemandGenerator::new(base, h3_probs);
        assert_eq!(generator.h3_probabilities.len(), 1);
    }

    #[test]
    #[should_panic(expected = "Must have at least one H3 cell")]
    fn test_empty_h3_map_panics() {
        let base = Box::new(FixedCountGenerator {
            count: 10,
            demand_type: "order".to_string(),
        });

        SpatialDemandGenerator::new(base, HashMap::new());
    }

    #[test]
    #[should_panic(expected = "must be non-negative")]
    fn test_negative_probability_panics() {
        let base = Box::new(FixedCountGenerator {
            count: 10,
            demand_type: "order".to_string(),
        });

        let mut h3_probs = HashMap::new();
        h3_probs.insert(create_test_cell(37.7749, -122.4194, Resolution::Nine), -0.5);

        SpatialDemandGenerator::new(base, h3_probs);
    }

    #[test]
    fn test_demands_get_locations() {
        let base = Box::new(FixedCountGenerator {
            count: 10,
            demand_type: "order".to_string(),
        });

        let mut h3_probs = HashMap::new();
        h3_probs.insert(create_test_cell(37.7749, -122.4194, Resolution::Nine), 1.0);

        let generator = SpatialDemandGenerator::new(base, h3_probs);

        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let demands = generator.generate(&ctx);
        assert_eq!(demands.len(), 10);

        // All demands should have locations
        for demand in &demands {
            assert!(demand.location.is_some());
        }
    }

    #[test]
    fn test_single_cell_all_same_location() {
        let base = Box::new(FixedCountGenerator {
            count: 5,
            demand_type: "order".to_string(),
        });

        let mut h3_probs = HashMap::new();
        let cell = create_test_cell(37.7749, -122.4194, Resolution::Nine);
        h3_probs.insert(cell, 1.0);

        let generator = SpatialDemandGenerator::new(base, h3_probs);

        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let demands = generator.generate(&ctx);

        // All demands should be at the same location (cell center)
        let first_loc = demands[0].location.unwrap();
        for demand in &demands {
            let loc = demand.location.unwrap();
            assert_relative_eq!(loc.x(), first_loc.x(), epsilon = 0.0001);
            assert_relative_eq!(loc.y(), first_loc.y(), epsilon = 0.0001);
        }
    }

    #[test]
    fn test_multiple_cells_distribution() {
        let base = Box::new(FixedCountGenerator {
            count: 1000,
            demand_type: "order".to_string(),
        });

        // Three cells with different probabilities
        let mut h3_probs = HashMap::new();
        let cell1 = create_test_cell(37.7749, -122.4194, Resolution::Nine); // SF
        let cell2 = create_test_cell(37.8044, -122.2712, Resolution::Nine); // Oakland
        let cell3 = create_test_cell(37.4419, -122.1430, Resolution::Nine); // Palo Alto

        h3_probs.insert(cell1, 0.5); // 50%
        h3_probs.insert(cell2, 0.3); // 30%
        h3_probs.insert(cell3, 0.2); // 20%

        let generator = SpatialDemandGenerator::new(base, h3_probs);

        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let demands = generator.generate(&ctx);

        // Count demands per location
        let mut location_counts: HashMap<String, usize> = HashMap::new();
        for demand in &demands {
            let loc = demand.location.unwrap();
            let key = format!("{:.4},{:.4}", loc.x(), loc.y());
            *location_counts.entry(key).or_insert(0) += 1;
        }

        // Should have 3 distinct locations
        assert_eq!(location_counts.len(), 3);

        // Check distribution is roughly correct (with tolerance for randomness)
        let values: Vec<usize> = location_counts.values().copied().collect();
        let total: usize = values.iter().sum();
        assert_eq!(total, 1000);

        // Verify rough distribution (should be approximately 500, 300, 200)
        // Allow ±10% tolerance
        assert!(values.iter().any(|&v| (v as f64 - 500.0).abs() < 100.0)); // ~50%
        assert!(values.iter().any(|&v| (v as f64 - 300.0).abs() < 100.0)); // ~30%
        assert!(values.iter().any(|&v| (v as f64 - 200.0).abs() < 100.0)); // ~20%
    }

    #[test]
    fn test_h3_cell_to_point_conversion() {
        let cell = create_test_cell(37.7749, -122.4194, Resolution::Nine);
        let point = SpatialDemandGenerator::h3_cell_to_point(cell);

        // Point should be close to original coordinates
        assert_relative_eq!(point.y(), 37.7749, epsilon = 0.01);
        assert_relative_eq!(point.x(), -122.4194, epsilon = 0.01);
    }

    #[test]
    fn test_zero_probability_cells() {
        // Test with some zero-probability cells (should still work)
        let base = Box::new(FixedCountGenerator {
            count: 100,
            demand_type: "order".to_string(),
        });

        let mut h3_probs = HashMap::new();
        let cell1 = create_test_cell(37.7749, -122.4194, Resolution::Nine);
        let cell2 = create_test_cell(37.8044, -122.2712, Resolution::Nine);

        h3_probs.insert(cell1, 1.0); // All weight on cell1
        h3_probs.insert(cell2, 0.0); // Zero weight on cell2

        let generator = SpatialDemandGenerator::new(base, h3_probs);

        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let demands = generator.generate(&ctx);

        // All demands should be at cell1
        let cell1_point = SpatialDemandGenerator::h3_cell_to_point(cell1);
        for demand in &demands {
            let loc = demand.location.unwrap();
            assert_relative_eq!(loc.x(), cell1_point.x(), epsilon = 0.0001);
            assert_relative_eq!(loc.y(), cell1_point.y(), epsilon = 0.0001);
        }
    }

    #[test]
    fn test_all_zero_probabilities() {
        // Edge case: all probabilities are zero
        let base = Box::new(FixedCountGenerator {
            count: 10,
            demand_type: "order".to_string(),
        });

        let mut h3_probs = HashMap::new();
        let cell1 = create_test_cell(37.7749, -122.4194, Resolution::Nine);
        let cell2 = create_test_cell(37.8044, -122.2712, Resolution::Nine);

        h3_probs.insert(cell1, 0.0);
        h3_probs.insert(cell2, 0.0);

        let generator = SpatialDemandGenerator::new(base, h3_probs);

        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        // Should not crash, should pick one of the cells
        let demands = generator.generate(&ctx);
        assert_eq!(demands.len(), 10);
        assert!(demands[0].location.is_some());
    }

    #[test]
    fn test_spatial_clustering_metric() {
        // Verify that spatial clustering produces expected variance
        let base = Box::new(FixedCountGenerator {
            count: 100,
            demand_type: "order".to_string(),
        });

        // Two distant cells with equal probability
        let mut h3_probs = HashMap::new();
        let cell1 = create_test_cell(37.7749, -122.4194, Resolution::Nine); // SF
        let cell2 = create_test_cell(40.7128, -74.0060, Resolution::Nine); // NYC

        h3_probs.insert(cell1, 0.5);
        h3_probs.insert(cell2, 0.5);

        let generator = SpatialDemandGenerator::new(base, h3_probs);

        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let demands = generator.generate(&ctx);

        // Calculate distance between all pairs
        let mut distances = Vec::new();
        for i in 0..demands.len() {
            for j in (i + 1)..demands.len() {
                let loc1 = demands[i].location.unwrap();
                let loc2 = demands[j].location.unwrap();

                let dx = loc1.x() - loc2.x();
                let dy = loc1.y() - loc2.y();
                let dist = (dx * dx + dy * dy).sqrt();
                distances.push(dist);
            }
        }

        // Should have two clusters: ~50% at distance 0, ~50% at large distance
        let small_distances = distances.iter().filter(|&&d| d < 1.0).count();
        let large_distances = distances.iter().filter(|&&d| d > 40.0).count();

        // Most distances should be either very small (same cluster) or very large (different clusters)
        assert!(small_distances + large_distances > distances.len() / 2);
    }
}
