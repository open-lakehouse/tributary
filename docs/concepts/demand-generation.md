# Demand Generation

**Last Updated:** March 16, 2026

## Overview

The demand generation framework provides probabilistic and time-based patterns for creating simulation demands. It supports Poisson processes, time-of-day patterns, spatial clustering, and composition of multiple generators.

## Objective

Learn how to:
- Use built-in demand generators (Poisson, TimeOfDay, Spatial, Composite)
- Configure statistical properties
- Integrate with SimulationRunner
- Create custom demand generators

## Prerequisites

- [Getting Started](../GETTING_STARTED.md)
- [Agents](agents.md) understanding
- Basic probability concepts

## DemandGenerator Trait

```rust
pub trait DemandGenerator: Send + Sync {
    fn should_generate(&mut self, current_time: DateTime<Utc>) -> bool;
    fn generate_demands(&mut self, current_time: DateTime<Utc>) -> Vec<Demand>;
    fn reset(&mut self);
}

pub struct Demand {
    pub demand_type: String,
    pub location: Option<Point<f64>>,
    pub agent_id: Option<AgentId>,
    pub created_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}
```

## Built-In Generators

### 1. PoissonDemandGenerator

Generates demands using Poisson distribution for realistic random arrivals:

```rust
use sim_core::demand::PoissonDemandGenerator;

// 10 demands per hour average
let mut generator = PoissonDemandGenerator::new(10.0);

// Check if should generate
if generator.should_generate(current_time) {
    let demands = generator.generate_demands(current_time);
    println!("Generated {} demands", demands.len());
}
```

**Statistical Properties:**
- Inter-arrival times follow exponential distribution
- Mean rate λ (lambda) configurable
- Validated with chi-squared tests

**Use Cases:**
- Customer order arrivals
- Service requests
- Random event generation

### 2. TimeOfDayDemandGenerator

Time-based demand patterns with configurable peaks:

```rust
use sim_core::demand::{TimeOfDayDemandGenerator, PeakPeriod};

let mut generator = TimeOfDayDemandGenerator::with_peaks(vec![
    PeakPeriod {
        start_hour: 7,
        end_hour: 9,
        rate_multiplier: 3.0,  // 3x base rate
    },
    PeakPeriod {
        start_hour: 12,
        end_hour: 14,
        rate_multiplier: 2.5,  // 2.5x base rate
    },
    PeakPeriod {
        start_hour: 18,
        end_hour: 20,
        rate_multiplier: 4.0,  // 4x base rate
    },
]);

// Or use meal-time presets
let generator = TimeOfDayDemandGenerator::with_meal_peaks(10.0); // Base rate
```

**Features:**
- Multiple peak periods
- Rate multipliers per period
- Smooth transitions
- Rush hour modeling

**Use Cases:**
- Restaurant meal times
- Commuter patterns
- Business hours

### 3. SpatialDemandGenerator

Location-based demand with H3 spatial clustering:

```rust
use sim_core::demand::{SpatialDemandGenerator, ZoneDemand};
use h3o::CellIndex;

let zones = vec![
    ZoneDemand {
        h3_cell: downtown_cell,
        density_multiplier: 3.0,  // High-density area
        capacity: Some(100),
    },
    ZoneDemand {
        h3_cell: suburb_cell,
        density_multiplier: 1.0,  // Normal density
        capacity: Some(50),
    },
];

let mut generator = SpatialDemandGenerator::with_zones(zones, 10.0);

let demands = generator.generate_demands(current_time);
// Demands have location field set to zone centers
```

**Features:**
- H3 hexagonal clustering
- Per-zone density multipliers
- Optional capacity limits
- Geographic hot-spots

**Use Cases:**
- Urban demand modeling
- Delivery zones
- Service areas

### 4. CompositeDemandGenerator

Combine multiple generators for complex patterns:

```rust
use sim_core::demand::{CompositeDemandGenerator, CompositionStrategy};

let mut generator = CompositeDemandGenerator::new(
    vec![
        Box::new(TimeOfDayDemandGenerator::with_meal_peaks(10.0)),
        Box::new(SpatialDemandGenerator::with_zones(zones, 5.0)),
    ],
    CompositionStrategy::All,  // Combine all outputs
);

let demands = generator.generate_demands(current_time);
```

**Strategies:**
- `All`: Combine demands from all generators
- `First`: Use first generator that produces demands
- `Weighted`: Random selection based on weights

**Use Cases:**
- Complex scenarios (time + location)
- Multi-factor demand patterns
- Layered generation logic

## Integration with SimulationRunner

```rust
use sim_core::{SimulationRunner, SimulationConfig};
use sim_core::demand::PoissonDemandGenerator;

let config = SimulationConfig::default();
let mut runner = SimulationRunner::new(config);

// Add demand generator
let poisson_gen = Box::new(PoissonDemandGenerator::new(10.0));
runner.add_demand_generator(poisson_gen);

// Run simulation - demands generated each tick
runner.run_for_steps(100)?;
```

### Demand-to-Workflow Pipeline

```rust
impl SimulationHooks for MyHooks {
    fn on_demands_generated(
        &mut self,
        ctx: &SimulationContext,
        demands: &[Demand],
    ) -> Result<()> {
        for demand in demands {
            if demand.demand_type == "food_order" {
                // Start workflow
                let workflow = Box::new(FoodOrderWorkflow::new(
                    AgentId::new(),
                    vec!["burger".to_string()],
                ));

                self.workflow_registry.start_workflow("food_order", workflow)?;
            }
        }
        Ok(())
    }
}
```

## Creating Custom Generators

```rust
use sim_core::demand::{DemandGenerator, Demand};
use chrono::{DateTime, Utc, Timelike};

pub struct BusinessHoursGenerator {
    base_rate: f64,
    open_hour: u32,
    close_hour: u32,
}

impl DemandGenerator for BusinessHoursGenerator {
    fn should_generate(&mut self, current_time: DateTime<Utc>) -> bool {
        let hour = current_time.hour();
        hour >= self.open_hour && hour < self.close_hour
    }

    fn generate_demands(&mut self, current_time: DateTime<Utc>) -> Vec<Demand> {
        if !self.should_generate(current_time) {
            return vec![];
        }

        // Generate based on base_rate
        let mut rng = rand::thread_rng();
        if rng.gen::<f64>() < (self.base_rate / 60.0) {  // Per minute
            vec![Demand {
                demand_type: "service_request".to_string(),
                location: None,
                agent_id: None,
                created_at: current_time,
                metadata: Default::default(),
            }]
        } else {
            vec![]
        }
    }

    fn reset(&mut self) {
        // No state to reset
    }
}
```

## Statistical Validation

Demand generators include statistical tests:

```rust
#[test]
fn test_poisson_distribution() {
    let mut generator = PoissonDemandGenerator::new(10.0);
    let mut demand_counts = vec![0; 20];

    for _ in 0..1000 {
        let demands = generator.generate_demands(Utc::now());
        demand_counts[demands.len()] += 1;
    }

    // Chi-squared test validates Poisson distribution
    let chi_squared = calculate_chi_squared(&demand_counts, 10.0);
    assert!(chi_squared < critical_value);
}
```

## Performance

Phase 10 performance metrics:

- **Per-tick overhead**: 0.75ms (85% better than 5ms target)
- **No regression**: Maintains simulation performance
- **Scalable**: Handles 10,000+ demands/hour

## Examples from Implementation

### sim-kitchen Integration

```rust
// In simulation setup
let mut runner = build_kitchen_simulation(/* ... */);

// Add time-based demand with meal peaks
let tod_gen = TimeOfDayDemandGenerator::with_meal_peaks(15.0);
runner.add_demand_generator(Box::new(tod_gen));

// Add spatial demand for different zones
let spatial_gen = SpatialDemandGenerator::with_zones(downtown_zones, 10.0);
runner.add_demand_generator(Box::new(spatial_gen));

runner.run_for_duration(Duration::hours(24))?;
```

**File:** `crates/sim-kitchen/tests/demand_integration_test.rs`

## Common Patterns

### Pattern 1: Layered Demand

```rust
// Base Poisson + time-of-day peaks + spatial clustering
let composite = CompositeDemandGenerator::new(
    vec![
        Box::new(PoissonDemandGenerator::new(5.0)),
        Box::new(TimeOfDayDemandGenerator::with_meal_peaks(10.0)),
        Box::new(SpatialDemandGenerator::with_zones(zones, 3.0)),
    ],
    CompositionStrategy::All,
);
```

### Pattern 2: Conditional Generation

```rust
impl DemandGenerator for ConditionalGenerator {
    fn should_generate(&mut self, current_time: DateTime<Utc>) -> bool {
        let hour = current_time.hour();
        let day_of_week = current_time.weekday();

        // Only weekdays during business hours
        day_of_week.num_days_from_monday() < 5 &&
        hour >= 9 && hour < 17
    }
}
```

### Pattern 3: Rate Adaptation

```rust
pub struct AdaptiveGenerator {
    base_rate: f64,
    current_rate: f64,
    load_threshold: f64,
}

impl DemandGenerator for AdaptiveGenerator {
    fn generate_demands(&mut self, current_time: DateTime<Utc>) -> Vec<Demand> {
        // Adjust rate based on system load
        let system_load = self.check_system_load();

        self.current_rate = if system_load > self.load_threshold {
            self.base_rate * 0.5  // Reduce demand
        } else {
            self.base_rate
        };

        // Generate with adjusted rate
        // ...
    }
}
```

## Common Pitfalls

### Pitfall 1: Not Seeding RNG

```rust
// ❌ Wrong - same sequence every run
let mut generator = PoissonDemandGenerator::new(10.0);

// ✅ Correct - use thread_rng() or seed explicitly
let mut generator = PoissonDemandGenerator::with_seed(10.0, 42);
```

### Pitfall 2: Rate Units Confusion

```rust
// ❌ Wrong - rate is per-hour, but generating per-second
let generator = PoissonDemandGenerator::new(10.0);  // 10/hour
// But called every second! Way too many demands

// ✅ Correct - adjust rate for tick frequency
let tick_duration = Duration::seconds(30);
let hourly_rate = 10.0;
let tick_rate = hourly_rate * (tick_duration.num_seconds() as f64 / 3600.0);
let generator = PoissonDemandGenerator::new(tick_rate);
```

### Pitfall 3: Forgetting Location

```rust
// ❌ Wrong - spatial demands without location
demands.push(Demand {
    demand_type: "food_order".to_string(),
    location: None,  // No location despite being spatial!
    // ...
});

// ✅ Correct - include location for spatial demands
demands.push(Demand {
    demand_type: "food_order".to_string(),
    location: Some(zone_center),
    // ...
});
```

## Testing

```rust
#[test]
fn test_time_of_day_peaks() {
    let mut generator = TimeOfDayDemandGenerator::with_meal_peaks(10.0);

    // Test lunch hour (high demand)
    let lunch_time = Utc::now().with_hour(12).unwrap();
    let lunch_demands = generator.generate_demands(lunch_time);

    // Test off-peak (low demand)
    let off_peak = Utc::now().with_hour(15).unwrap();
    let off_peak_demands = generator.generate_demands(off_peak);

    assert!(lunch_demands.len() > off_peak_demands.len());
}
```

## Related Documentation

- [Agents](agents.md) - Demand consumption
- [Workflows](workflows.md) - Demand-triggered workflows
- [Resources](resources.md) - Processing demands as tasks

## Summary

Demand generation framework features:

- 4 composable generators (Poisson, TimeOfDay, Spatial, Composite)
- Statistical validation with chi-squared tests
- <1ms per-tick overhead
- Full SimulationRunner integration
- Custom generator support via trait

---

**Last Updated:** March 16, 2026
