//! Time-of-day demand generator with hourly multipliers

use super::DemandGenerator;
use crate::context::SimulationContext;
use crate::population::Demand;
use chrono::Timelike;

/// Time-of-day demand generator
///
/// Wraps a base demand generator and applies hourly multipliers to scale demand
/// based on the time of day. This allows modeling realistic demand patterns with
/// peak and off-peak hours.
///
/// # Example
///
/// ```rust,ignore
/// use sim_core::demand::{PoissonDemandGenerator, TimeOfDayDemandGenerator};
///
/// // Base generator: 10 orders/hour
/// let base = Box::new(PoissonDemandGenerator::new(10.0, "order"));
///
/// // Peak hours (12pm and 6pm) have 2x multiplier
/// let hourly_multipliers = [
///     0.5, 0.5, 0.5, 0.5, 0.5, 0.5,  // 12am-6am (low)
///     0.8, 1.0, 1.2, 1.5, 1.8, 2.0,  // 6am-12pm (ramp to lunch)
///     2.5, 1.8, 1.5, 1.2, 1.5, 2.0,  // 12pm-6pm (lunch peak, ramp to dinner)
///     2.5, 2.0, 1.5, 1.2, 0.8, 0.6,  // 6pm-12am (dinner peak, wind down)
/// ];
///
/// let generator = TimeOfDayDemandGenerator::new(base, hourly_multipliers);
/// ```
pub struct TimeOfDayDemandGenerator {
    /// Base demand generator to wrap
    pub base_generator: Box<dyn DemandGenerator>,
    /// Multipliers for each hour of the day (0-23)
    pub hourly_multipliers: [f64; 24],
}

impl TimeOfDayDemandGenerator {
    /// Create a new time-of-day demand generator
    ///
    /// # Arguments
    ///
    /// * `base_generator` - Base generator to apply multipliers to
    /// * `hourly_multipliers` - Array of 24 multipliers (one per hour)
    ///
    /// # Panics
    ///
    /// Panics if any multiplier is negative
    pub fn new(base_generator: Box<dyn DemandGenerator>, hourly_multipliers: [f64; 24]) -> Self {
        // Validate multipliers are non-negative
        for (i, &multiplier) in hourly_multipliers.iter().enumerate() {
            assert!(
                multiplier >= 0.0,
                "Multiplier at hour {} must be non-negative, got {}",
                i,
                multiplier
            );
        }

        Self {
            base_generator,
            hourly_multipliers,
        }
    }

    /// Create a generator with uniform multipliers (no time-of-day effect)
    pub fn uniform(base_generator: Box<dyn DemandGenerator>) -> Self {
        Self::new(base_generator, [1.0; 24])
    }
}

impl DemandGenerator for TimeOfDayDemandGenerator {
    fn generate(&self, ctx: &SimulationContext) -> Vec<Demand> {
        // Get current hour of day (0-23)
        let hour = ctx.current_time().hour() as usize;
        let multiplier = self.hourly_multipliers[hour];

        // Generate base demands
        let base_demands = self.base_generator.generate(ctx);
        let base_count = base_demands.len();

        // Handle edge case: no base demands
        if base_count == 0 {
            return Vec::new();
        }

        // Calculate scaled count
        let scaled_count = (base_count as f64 * multiplier).round() as usize;

        // Scale demands based on multiplier
        if scaled_count <= base_count {
            // Scaling down: take subset
            base_demands.into_iter().take(scaled_count).collect()
        } else {
            // Scaling up: cycle through demands by cloning to reach target count
            let mut result = Vec::with_capacity(scaled_count);

            for i in 0..scaled_count {
                // Clone demands in round-robin fashion
                let source_index = i % base_count;
                result.push(base_demands[source_index].clone());
            }

            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::SimulationConfig;
    use crate::demand::PoissonDemandGenerator;
    use chrono::{TimeZone, Utc};

    // Helper generator that always produces a fixed count
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
                    metadata: std::collections::HashMap::new(),
                })
                .collect()
        }
    }

    #[test]
    fn test_time_of_day_creation() {
        let base = Box::new(FixedCountGenerator {
            count: 10,
            demand_type: "order".to_string(),
        });
        let multipliers = [1.0; 24];
        let generator = TimeOfDayDemandGenerator::new(base, multipliers);

        assert_eq!(generator.hourly_multipliers.len(), 24);
    }

    #[test]
    fn test_uniform_multipliers() {
        let base = Box::new(FixedCountGenerator {
            count: 10,
            demand_type: "order".to_string(),
        });
        let generator = TimeOfDayDemandGenerator::uniform(base);

        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let demands = generator.generate(&ctx);
        assert_eq!(demands.len(), 10); // No scaling with uniform multipliers
    }

    #[test]
    fn test_peak_hour_scaling() {
        let base = Box::new(FixedCountGenerator {
            count: 10,
            demand_type: "order".to_string(),
        });

        // Set peak at noon (hour 12) with 2x multiplier
        let mut multipliers = [1.0; 24];
        multipliers[12] = 2.0;

        let generator = TimeOfDayDemandGenerator::new(base, multipliers);

        // Create context at noon
        let config = SimulationConfig::default();
        let start_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let ctx = SimulationContext::new(start_time, config);

        let demands = generator.generate(&ctx);
        assert_eq!(demands.len(), 20); // 10 * 2.0 = 20
    }

    #[test]
    fn test_off_peak_reduction() {
        let base = Box::new(FixedCountGenerator {
            count: 10,
            demand_type: "order".to_string(),
        });

        // Set off-peak at 3am (hour 3) with 0.5x multiplier
        let mut multipliers = [1.0; 24];
        multipliers[3] = 0.5;

        let generator = TimeOfDayDemandGenerator::new(base, multipliers);

        // Create context at 3am
        let config = SimulationConfig::default();
        let start_time = Utc.with_ymd_and_hms(2024, 1, 1, 3, 0, 0).unwrap();
        let ctx = SimulationContext::new(start_time, config);

        let demands = generator.generate(&ctx);
        assert_eq!(demands.len(), 5); // 10 * 0.5 = 5
    }

    #[test]
    fn test_midnight_transition() {
        let base = Box::new(FixedCountGenerator {
            count: 10,
            demand_type: "order".to_string(),
        });

        let mut multipliers = [1.0; 24];
        multipliers[0] = 0.3; // Midnight
        multipliers[23] = 0.4; // 11pm

        let generator = TimeOfDayDemandGenerator::new(base, multipliers);

        // Test at midnight
        let config = SimulationConfig::default();
        let start_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let ctx = SimulationContext::new(start_time, config);

        let demands = generator.generate(&ctx);
        assert_eq!(demands.len(), 3); // 10 * 0.3 = 3
    }

    #[test]
    fn test_composition_with_poisson() {
        // Test that TimeOfDay can wrap Poisson generator
        // Use high rate to ensure we get demands (lambda = rate * time_step_hours)
        // With rate=1000 and time_step=60s, lambda = 1000 * (60/3600) ≈ 16.67
        // This gives P(X=0) ≈ 5.7e-8, virtually guaranteed to generate demands
        let base = Box::new(PoissonDemandGenerator::with_seed(1000.0, "order", 42));

        let mut multipliers = [1.0; 24];
        multipliers[12] = 3.0; // Triple at noon

        let generator = TimeOfDayDemandGenerator::new(base, multipliers);

        // Test at noon
        let config = SimulationConfig::default();
        let start_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let ctx = SimulationContext::new(start_time, config);

        let demands = generator.generate(&ctx);
        // Should have roughly 3x the base rate, and definitely > 0 with high rate
        assert!(demands.len() > 0);
    }

    #[test]
    fn test_zero_multiplier() {
        let base = Box::new(FixedCountGenerator {
            count: 100,
            demand_type: "order".to_string(),
        });

        let mut multipliers = [1.0; 24];
        multipliers[4] = 0.0; // No demand at 4am

        let generator = TimeOfDayDemandGenerator::new(base, multipliers);

        // Test at 4am
        let config = SimulationConfig::default();
        let start_time = Utc.with_ymd_and_hms(2024, 1, 1, 4, 0, 0).unwrap();
        let ctx = SimulationContext::new(start_time, config);

        let demands = generator.generate(&ctx);
        assert_eq!(demands.len(), 0);
    }

    #[test]
    fn test_realistic_daily_pattern() {
        // Test a realistic pattern with lunch and dinner peaks
        let base = Box::new(FixedCountGenerator {
            count: 10,
            demand_type: "order".to_string(),
        });

        let hourly_multipliers = [
            0.3, 0.2, 0.2, 0.2, 0.3, 0.4, // 12am-6am (low)
            0.6, 0.8, 1.0, 1.2, 1.5, 2.0, // 6am-12pm (ramp to lunch)
            2.5, 1.8, 1.2, 1.0, 1.2, 1.8, // 12pm-6pm (lunch peak, ramp to dinner)
            2.5, 2.0, 1.5, 1.0, 0.7, 0.5, // 6pm-12am (dinner peak, wind down)
        ];

        let generator = TimeOfDayDemandGenerator::new(base, hourly_multipliers);

        let config = SimulationConfig::default();

        // Test lunch peak (12pm)
        let lunch_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let ctx_lunch = SimulationContext::new(lunch_time, config.clone());
        let lunch_demands = generator.generate(&ctx_lunch);
        assert_eq!(lunch_demands.len(), 25); // 10 * 2.5

        // Test dinner peak (6pm)
        let dinner_time = Utc.with_ymd_and_hms(2024, 1, 1, 18, 0, 0).unwrap();
        let ctx_dinner = SimulationContext::new(dinner_time, config.clone());
        let dinner_demands = generator.generate(&ctx_dinner);
        assert_eq!(dinner_demands.len(), 25); // 10 * 2.5

        // Test off-peak (3am)
        let offpeak_time = Utc.with_ymd_and_hms(2024, 1, 1, 3, 0, 0).unwrap();
        let ctx_offpeak = SimulationContext::new(offpeak_time, config);
        let offpeak_demands = generator.generate(&ctx_offpeak);
        assert_eq!(offpeak_demands.len(), 2); // 10 * 0.2
    }

    #[test]
    #[should_panic(expected = "Multiplier at hour 5 must be non-negative")]
    fn test_negative_multiplier_panics() {
        let base = Box::new(FixedCountGenerator {
            count: 10,
            demand_type: "order".to_string(),
        });

        let mut multipliers = [1.0; 24];
        multipliers[5] = -0.5; // Invalid

        TimeOfDayDemandGenerator::new(base, multipliers);
    }
}
