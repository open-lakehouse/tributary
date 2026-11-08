//! Poisson demand generator with exponential inter-arrival times

use super::DemandGenerator;
use crate::context::SimulationContext;
use crate::context::derive_seed;
use crate::population::Demand;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand_distr::{Distribution, Exp};
use std::collections::HashMap;

/// Poisson demand generator
///
/// Generates demands using a Poisson process with exponential inter-arrival times.
/// The rate is specified in events per hour and is scaled based on the simulation time step.
///
/// # Example
///
/// ```rust,ignore
/// use sim_core::demand::PoissonDemandGenerator;
///
/// // Generate 10 orders per hour on average
/// let generator = PoissonDemandGenerator::new(10.0, "food_order");
/// ```
pub struct PoissonDemandGenerator {
    /// Rate in events per hour
    pub rate: f64,
    /// Type of demand to generate
    pub demand_type: String,
    /// Base seed; each step's draws come from `derive_seed(seed, "poisson/{step}")`
    seed: u64,
}

impl PoissonDemandGenerator {
    /// Create a new Poisson demand generator
    ///
    /// The base seed is drawn from OS entropy, so runs are not reproducible;
    /// use [`with_seed`](Self::with_seed) for that.
    ///
    /// # Arguments
    ///
    /// * `rate` - Average number of events per hour
    /// * `demand_type` - Type identifier for generated demands
    ///
    /// # Panics
    ///
    /// Panics if rate is negative
    pub fn new(rate: f64, demand_type: impl Into<String>) -> Self {
        Self::with_seed(rate, demand_type, rand::random())
    }

    /// Create a generator with a specific seed for reproducibility
    ///
    /// Each step's count depends only on `seed` and the step number, not on
    /// earlier calls, so a run restored from a snapshot draws the same
    /// demand as an uninterrupted one.
    ///
    /// # Arguments
    ///
    /// * `rate` - Average number of events per hour
    /// * `demand_type` - Type identifier for generated demands
    /// * `seed` - Seed for the random number generator
    pub fn with_seed(rate: f64, demand_type: impl Into<String>, seed: u64) -> Self {
        assert!(rate >= 0.0, "Rate must be non-negative");
        Self {
            rate,
            demand_type: demand_type.into(),
            seed,
        }
    }

    /// Sample from Poisson distribution using exponential inter-arrivals
    ///
    /// This uses the standard method: sample exponential inter-arrival times
    /// until their sum exceeds the time window.
    fn sample_poisson(&self, exp: Exp<f64>, step: usize) -> usize {
        let mut rng = StdRng::seed_from_u64(derive_seed(self.seed, &format!("poisson/{step}")));
        let mut count = 0;
        let mut time_sum = 0.0;

        // Keep sampling until we exceed the time window (1.0 unit)
        while time_sum < 1.0 {
            time_sum += exp.sample(&mut rng);
            if time_sum < 1.0 {
                count += 1;
            }
        }

        count
    }
}

impl DemandGenerator for PoissonDemandGenerator {
    fn generate(&self, ctx: &SimulationContext) -> Vec<Demand> {
        // Convert time step to hours
        let delta_hours = ctx.time_step().num_seconds() as f64 / 3600.0;

        // Scale rate by time step (lambda = rate * delta_t)
        let lambda = self.rate * delta_hours;

        // Handle edge cases
        if lambda <= 0.0 {
            return vec![];
        }

        // For Poisson process, the inter-arrival times are exponentially distributed
        // with rate parameter lambda
        let exp = Exp::new(lambda).expect("Lambda must be positive");

        // Sample number of events from Poisson distribution
        let count = self.sample_poisson(exp, ctx.step());

        // Generate demands
        (0..count)
            .map(|_| Demand {
                demand_type: self.demand_type.clone(),
                location: None,
                metadata: HashMap::new(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::SimulationConfig;
    use approx::assert_relative_eq;

    #[test]
    fn test_poisson_generator_creation() {
        let generator = PoissonDemandGenerator::new(10.0, "order");
        assert_eq!(generator.rate, 10.0);
        assert_eq!(generator.demand_type, "order");
    }

    #[test]
    fn test_poisson_generator_with_seed() {
        let generator = PoissonDemandGenerator::with_seed(5.0, "ticket", 42);
        assert_eq!(generator.rate, 5.0);
        assert_eq!(generator.demand_type, "ticket");
    }

    #[test]
    #[should_panic(expected = "Rate must be non-negative")]
    fn test_negative_rate_panics() {
        PoissonDemandGenerator::new(-1.0, "order");
    }

    #[test]
    fn test_zero_rate_generates_no_demands() {
        let generator = PoissonDemandGenerator::new(0.0, "order");
        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let demands = generator.generate(&ctx);
        assert_eq!(demands.len(), 0);
    }

    #[test]
    fn test_time_step_scaling() {
        // With 60-second time step and 60 events/hour rate,
        // we expect lambda = 1.0 (1 event per minute on average)
        let generator = PoissonDemandGenerator::with_seed(60.0, "order", 12345);

        let config = SimulationConfig {
            time_step: chrono::Duration::seconds(60), // 1 minute
            ..Default::default()
        };
        let mut ctx = SimulationContext::from_now(config);

        // Run multiple steps and check average
        let mut total = 0;
        let iterations = 1000;
        for _ in 0..iterations {
            let demands = generator.generate(&ctx);
            total += demands.len();
            ctx.advance();
        }

        let average = total as f64 / iterations as f64;
        // Should be close to 1.0 (lambda = 60 events/hour * 1 minute = 1)
        assert_relative_eq!(average, 1.0, epsilon = 0.2);
    }

    #[test]
    fn test_reproducibility_with_seed() {
        let generator1 = PoissonDemandGenerator::with_seed(10.0, "order", 999);
        let generator2 = PoissonDemandGenerator::with_seed(10.0, "order", 999);

        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let demands1 = generator1.generate(&ctx);
        let demands2 = generator2.generate(&ctx);

        assert_eq!(demands1.len(), demands2.len());
    }

    #[test]
    fn test_very_high_rate() {
        // Test with very high rate to ensure no overflow. Seeded: the ±100
        // window is ~3σ for λ = 1000, so an entropy-seeded run is flaky.
        let generator = PoissonDemandGenerator::with_seed(1000.0, "order", 1000);

        let config = SimulationConfig {
            time_step: chrono::Duration::seconds(3600), // 1 hour
            ..Default::default()
        };
        let ctx = SimulationContext::from_now(config);

        let demands = generator.generate(&ctx);
        // Should generate around 1000 demands
        assert!(demands.len() > 900 && demands.len() < 1100);
    }

    #[test]
    fn test_generated_demands_have_correct_type() {
        let generator = PoissonDemandGenerator::with_seed(10.0, "food_order", 42);

        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let demands = generator.generate(&ctx);
        for demand in demands {
            assert_eq!(demand.demand_type, "food_order");
            assert!(demand.location.is_none());
            assert_eq!(demand.metadata.len(), 0);
        }
    }

    #[test]
    fn test_rate_distribution() {
        // Verify that over many iterations, the average matches the expected rate
        let rate = 5.0; // 5 events per hour
        let generator = PoissonDemandGenerator::with_seed(rate, "order", 54321);

        let config = SimulationConfig {
            time_step: chrono::Duration::minutes(30), // 0.5 hours
            ..Default::default()
        };
        let mut ctx = SimulationContext::from_now(config);

        let mut total = 0;
        let iterations = 2000;
        for _ in 0..iterations {
            let demands = generator.generate(&ctx);
            total += demands.len();
            ctx.advance();
        }

        let average = total as f64 / iterations as f64;
        // Expected: 5 events/hour * 0.5 hours = 2.5 events per step
        assert_relative_eq!(average, 2.5, epsilon = 0.15);
    }

    #[test]
    fn test_randomness() {
        // Verify that two generators with different seeds produce different results.
        // Use a high rate (600/hr) so lambda=10 per 60s step, giving meaningful variance.
        let generator1 = PoissonDemandGenerator::with_seed(600.0, "order", 1);
        let generator2 = PoissonDemandGenerator::with_seed(600.0, "order", 2);

        let config = SimulationConfig::default();
        let mut ctx = SimulationContext::from_now(config);

        let mut different_count = 0;
        for _ in 0..100 {
            let demands1 = generator1.generate(&ctx);
            let demands2 = generator2.generate(&ctx);
            if demands1.len() != demands2.len() {
                different_count += 1;
            }
            ctx.advance();
        }

        // Should have different results most of the time
        assert!(different_count > 40);
    }

    #[test]
    fn draws_depend_only_on_seed_and_step() {
        // Restore-safety: a generator that skipped steps (or a fresh one after a
        // snapshot restore) draws the same count at a step as one that ran them.
        let config = SimulationConfig::default();
        let mut ctx = SimulationContext::from_now(config);
        let warm = PoissonDemandGenerator::with_seed(600.0, "order", 7);
        let mut counts = Vec::new();
        for _ in 0..20 {
            counts.push(warm.generate(&ctx).len());
            ctx.advance();
        }
        let fresh = PoissonDemandGenerator::with_seed(600.0, "order", 7);
        assert_eq!(fresh.generate(&ctx).len(), warm.generate(&ctx).len());
        // Repeated calls at one step agree, and steps differ.
        assert_eq!(warm.generate(&ctx).len(), warm.generate(&ctx).len());
        assert!(counts.windows(2).any(|w| w[0] != w[1]));
    }
}
