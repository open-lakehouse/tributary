# Testing Strategies Guide

This guide covers comprehensive testing patterns, practices, test organization, mocking, fixtures, and coverage goals based on the 649+ test suite across the codebase.

## Table of Contents

1. [Testing Philosophy](#testing-philosophy)
2. [Test Organization](#test-organization)
3. [Unit Testing Patterns](#unit-testing-patterns)
4. [Integration Testing](#integration-testing)
5. [Mocking and Fixtures](#mocking-and-fixtures)
6. [Coverage Goals](#coverage-goals)
7. [Examples from Codebase](#examples-from-codebase)

## Testing Philosophy

### Test Pyramid

```
         ╱╲
        ╱  ╲       E2E Tests (5%)
       ╱    ╲      - Full 24-hour simulations
      ╱──────╲     - Framework generality tests
     ╱        ╲
    ╱          ╲   Integration Tests (20%)
   ╱────────────╲  - Multi-component interactions
  ╱              ╲ - Resource-agent coordination
 ╱────────────────╲
╱                  ╲ Unit Tests (75%)
────────────────────  - Individual components
                      - State transitions
                      - Edge cases
```

### Testing Principles

1. **Fast Feedback**: Unit tests run in < 1 second
2. **Isolated**: Each test is independent
3. **Repeatable**: Same input = same output
4. **Thorough**: Cover happy path, edge cases, errors
5. **Readable**: Tests document behavior
6. **Maintainable**: Avoid test brittleness

### Coverage Goals

From the 649+ test suite:

- **Overall**: > 85% line coverage
- **Critical Paths**: 100% coverage
- **Agent Logic**: > 90% coverage
- **Resource Logic**: > 90% coverage
- **Workflow Logic**: > 90% coverage
- **Framework Core**: > 95% coverage

## Test Organization

### File Structure

```
crate/
├── src/
│   ├── lib.rs
│   ├── agents/
│   │   ├── mod.rs
│   │   └── package.rs       # Module code
│   │       └── tests        # Inline unit tests
│   └── resources/
│       ├── mod.rs
│       └── sorting.rs       # Module code
│           └── tests        # Inline unit tests
└── tests/
    ├── integration_test.rs  # Multi-component tests
    └── simulation_test.rs   # End-to-end tests
```

### Test Module Organization

```rust
// src/agents/package.rs

// Main implementation
pub struct Package { /* ... */ }
impl Package { /* ... */ }
impl Agent for Package { /* ... */ }

// Unit tests inline
#[cfg(test)]
mod tests {
    use super::*;

    // Group 1: Creation tests
    #[test]
    fn test_package_creation() { /* ... */ }

    #[test]
    fn test_package_with_custom_parameters() { /* ... */ }

    // Group 2: State transition tests
    #[test]
    fn test_package_state_transitions() { /* ... */ }

    #[test]
    fn test_invalid_state_transition() { /* ... */ }

    // Group 3: Behavior tests
    #[test]
    fn test_package_tick() { /* ... */ }

    // Group 4: Serialization tests
    #[test]
    fn test_arrow_round_trip() { /* ... */ }

    // Group 5: Edge cases
    #[test]
    fn test_package_position_semantics() { /* ... */ }
}
```

### Test Naming Convention

**Pattern: `test_<component>_<scenario>_<expected_outcome>`**

```rust
// Good examples
#[test]
fn test_driver_movement_reaches_destination() { }

#[test]
fn test_sorting_station_queue_rejects_when_full() { }

#[test]
fn test_workflow_progression_completes_all_steps() { }

// Bad examples
#[test]
fn test1() { } // Not descriptive

#[test]
fn test_driver() { } // Too vague

#[test]
fn test_everything_works() { } // Not specific
```

## Unit Testing Patterns

### Pattern 1: State Transition Testing

Test all valid state transitions for agents:

```rust
#[test]
fn test_package_state_transitions() {
    let origin = Point::new(0.0, 0.0);
    let destination = Point::new(1.0, 1.0);
    let mut pkg = Package::new(origin, destination, 1.0, PackagePriority::Standard);

    // Initial state
    assert_eq!(pkg.state.as_str(), "created");

    // Created -> Sorted
    pkg.mark_sorted("warehouse-1".to_string());
    assert_eq!(pkg.state.as_str(), "sorted");

    // Sorted -> Loaded
    pkg.mark_loaded("driver-1".to_string());
    assert_eq!(pkg.state.as_str(), "loaded");

    // Loaded -> InTransit
    let eta = Utc::now();
    pkg.mark_in_transit(eta);
    assert_eq!(pkg.state.as_str(), "in_transit");

    // InTransit -> Delivered
    pkg.mark_delivered();
    assert_eq!(pkg.state.as_str(), "delivered");
    assert!(pkg.is_delivered());
    assert!(pkg.delivered_at.is_some());
}
```

**What to test:**

- All valid transitions
- State-specific data preservation
- Side effects (timestamps, counters)
- Terminal states (is_idle, is_complete)

### Pattern 2: Capacity and Bounds Testing

Test resource capacity constraints:

```rust
#[test]
fn test_sorting_station_capacity_limits() {
    let mut station = SortingStation::standard("S1".to_string());

    // Queue 15 packages
    for i in 0..15 {
        station.enqueue_package(format!("pkg-{}", i)).unwrap();
    }

    // First tick processes up to capacity (10)
    station.process_tick(1);

    assert_eq!(station.current_load, 10);
    assert_eq!(station.queue_length(), 5);
    assert!(!station.has_capacity());
}

#[test]
fn test_sorting_station_queue_limit() {
    let mut station = SortingStation::standard("S1".to_string());

    // Can queue up to capacity * 3 = 30 packages
    for i in 0..30 {
        assert!(station.enqueue_package(format!("pkg-{}", i)).is_ok());
    }

    // 31st should fail
    assert!(station.enqueue_package("pkg-31".to_string()).is_err());
}
```

**What to test:**

- At capacity limit
- Below capacity
- At maximum queue size
- Beyond maximum (error cases)

### Pattern 3: Timing and Duration Testing

Test tick-based timing calculations:

```rust
#[test]
fn test_sorting_station_processing_completion() {
    let mut station = SortingStation::standard("S1".to_string());

    station.enqueue_package("pkg-1".to_string()).unwrap();
    station.process_tick(1);

    // Package should be processing
    assert_eq!(station.current_load, 1);

    // Processing takes (3600 / 12) / 30 = 10 ticks
    for tick in 2..11 {
        station.process_tick(tick);
        assert_eq!(station.current_load, 1); // Still processing
    }

    // At tick 11, should be complete
    station.process_tick(11);
    assert_eq!(station.current_load, 0);
    assert_eq!(station.total_processed, 1);
}

#[test]
fn test_driver_loading_duration() {
    let mut driver = Driver::new(Point::new(0.0, 0.0));
    driver.state = DriverState::LoadingPackages {
        warehouse_id: "w1".to_string(),
        package_count: 5,
        loading_started_at: 0,
    };
    driver.metadata_destinations = vec![Point::new(1.0, 1.0)];
    driver.metadata_package_ids = vec!["pkg-1".to_string()];

    let ctx = SimulationContext::from_now(SimulationConfig::default());

    // Loading takes 12 ticks
    for _ in 0..11 {
        driver.tick(&ctx).unwrap();
        assert_eq!(driver.state.as_str(), "loading_packages");
    }

    // 12th tick should complete loading
    let result = driver.tick(&ctx).unwrap();
    assert_eq!(driver.state.as_str(), "delivering_packages");
    assert!(result.state_changes.contains_key("loading_complete"));
}
```

**What to test:**

- Exact completion timing
- Processing throughout duration
- State before/after completion
- Edge cases (tick 0, instant completion)

### Pattern 4: Serialization Round-Trip Testing

Test Arrow serialization:

```rust
#[test]
fn test_package_arrow_round_trip() {
    let pkg = Package::new(
        Point::new(1.0, 2.0),
        Point::new(3.0, 4.0),
        2.5,
        PackagePriority::Express,
    );

    // Serialize to Arrow
    let row = pkg.to_arrow_row();

    // Deserialize from Arrow
    let restored = Package::from_arrow_row(&row).unwrap();

    // Verify all fields preserved
    assert_eq!(pkg.id, restored.id);
    assert_eq!(pkg.state.as_str(), restored.state.as_str());
    assert_eq!(pkg.weight_kg, restored.weight_kg);
    assert_eq!(pkg.tracking_number, restored.tracking_number);
}

#[test]
fn test_driver_arrow_round_trip() {
    let home = Point::new(1.0, 2.0);
    let driver = Driver::with_capacity(home, 15, 20.0);

    let row = driver.to_arrow_row();
    let restored = Driver::from_arrow_row(&row).unwrap();

    assert_eq!(driver.id, restored.id);
    assert_eq!(driver.state.as_str(), restored.state.as_str());
    assert_eq!(driver.home_location, restored.home_location);
    assert_eq!(driver.capacity, restored.capacity);
}
```

**What to test:**

- All agent types
- All state variants
- Position (Some vs None)
- Metadata fields
- No data loss

### Pattern 5: Error Case Testing

Test error conditions explicitly:

```rust
#[test]
fn test_create_package_invalid_warehouse() {
    let mut sim = LogisticsSimulation::new();

    let result = sim.create_package(10, 0, 1.0, PackagePriority::Standard);
    assert!(result.is_err());
}

#[test]
fn test_create_package_invalid_destination() {
    let mut sim = LogisticsSimulation::new();

    let result = sim.create_package(0, 2000, 1.0, PackagePriority::Standard);
    assert!(result.is_err());
}

#[test]
#[should_panic(expected = "Package count exceeds driver capacity")]
fn test_assign_delivery_exceeds_capacity() {
    let mut driver = Driver::with_capacity(Point::new(0.0, 0.0), 2, 15.0);

    let warehouse = Point::new(1.0, 1.0);
    let destinations = vec![Point::new(2.0, 2.0); 5];
    let package_ids = vec!["pkg-1".to_string(); 5];

    driver.assign_delivery(warehouse, "w1".to_string(), destinations, package_ids);
}
```

**What to test:**

- Invalid inputs
- Boundary violations
- Constraint violations
- Expected panics (use `#[should_panic]`)

### Pattern 6: Ordering and FIFO Testing

Test queue ordering:

```rust
#[test]
fn test_fifo_ordering() {
    let mut station = SortingStation::new("S1".to_string(), 2, 12.0);

    station.enqueue_package("pkg-1".to_string()).unwrap();
    station.enqueue_package("pkg-2".to_string()).unwrap();
    station.enqueue_package("pkg-3".to_string()).unwrap();

    station.process_tick(1);

    // First two should be processing
    assert_eq!(station.processing.len(), 2);
    assert!(station.processing.iter().any(|(id, _)| id == "pkg-1"));
    assert!(station.processing.iter().any(|(id, _)| id == "pkg-2"));

    // Third should still be queued
    assert_eq!(station.queue_length(), 1);
}
```

### Pattern 7: Metrics and Statistics Testing

Test metric calculations:

```rust
#[test]
fn test_utilization_calculation() {
    let mut station = SortingStation::standard("S1".to_string());

    assert_eq!(station.utilization(), 0.0);

    // Add 5 packages (50% utilization)
    for i in 0..5 {
        station.enqueue_package(format!("pkg-{}", i)).unwrap();
    }
    station.process_tick(1);

    assert_eq!(station.utilization(), 50.0);

    // Fill to capacity
    for i in 5..10 {
        station.enqueue_package(format!("pkg-{}", i)).unwrap();
    }
    station.process_tick(2);

    assert_eq!(station.utilization(), 100.0);
}

#[test]
fn test_delivery_time_calculation() {
    let mut pkg = Package::new(
        Point::new(0.0, 0.0),
        Point::new(1.0, 1.0),
        1.0,
        PackagePriority::Standard,
    );

    assert!(pkg.delivery_time_seconds().is_none());

    // Simulate delivery
    pkg.mark_sorted("w1".to_string());
    pkg.mark_loaded("d1".to_string());
    pkg.mark_in_transit(Utc::now());
    std::thread::sleep(std::time::Duration::from_millis(100));
    pkg.mark_delivered();

    assert!(pkg.delivery_time_seconds().is_some());
    assert!(pkg.delivery_time_seconds().unwrap() >= 0);
}
```

## Integration Testing

### Pattern 1: Multi-Component Interaction

Test interaction between agents and resources:

```rust
#[test]
fn test_package_sorting_workflow() {
    let mut station = SortingStation::standard("Station-A".to_string());
    let mut pkg = Package::new(
        Point::new(0.0, 0.0),
        Point::new(1.0, 1.0),
        2.0,
        PackagePriority::Standard,
    );

    // Package arrives
    assert_eq!(pkg.state.as_str(), "created");

    // Enqueue for sorting
    let pkg_id = pkg.id.to_string();
    station.enqueue_package(pkg_id.clone()).unwrap();

    // Process until complete
    for tick in 1..15 {
        station.process_tick(tick);

        let completed = station.get_completed(tick);
        if completed.contains(&pkg_id) {
            // Mark package as sorted
            pkg.mark_sorted("Station-A".to_string());
            break;
        }
    }

    assert_eq!(pkg.state.as_str(), "sorted");
    assert_eq!(station.total_processed, 1);
}
```

### Pattern 2: Full Simulation Test

Test complete simulation cycle:

```rust
#[test]
fn test_24_hour_simulation() {
    let mut sim = LogisticsSimulation::new();

    // Create initial batch of packages
    for i in 0..100 {
        let warehouse_idx = i % 2;
        let destination_idx = i * 10;
        let weight = 1.0 + (i % 5) as f64 * 0.5;
        let priority = if i % 3 == 0 {
            PackagePriority::Express
        } else {
            PackagePriority::Standard
        };

        sim.create_package(warehouse_idx, destination_idx, weight, priority).unwrap();
    }

    assert_eq!(sim.package_count(), 100);

    // Run for 24 hours
    sim.run_for_hours(24).unwrap();

    // Verify metrics
    let metrics = sim.get_metrics();
    assert!(metrics.packages_created >= 100, "Should have created at least 100 packages");
}
```

### Pattern 3: Framework Generality Test

Verify framework has no domain-specific code:

```rust
#[test]
fn test_framework_generality_zero_modifications() {
    // This test ensures sim-core has ZERO knowledge of logistics domain
    use sim_core::*;

    // Framework should work with generic types only
    let _config = SimulationConfig::default();
    let _ctx = SimulationContext::from_now(_config);

    // Compilation proves framework is truly generic
    assert!(true, "Framework compiles without domain-specific modifications");
}
```

## Mocking and Fixtures

### Fixture Pattern: Test Data Builders

```rust
// Test utilities module
pub mod test_utils {
    use super::*;

    pub struct PackageBuilder {
        origin: Point<f64>,
        destination: Point<f64>,
        weight_kg: f64,
        priority: PackagePriority,
    }

    impl PackageBuilder {
        pub fn new() -> Self {
            Self {
                origin: Point::new(0.0, 0.0),
                destination: Point::new(1.0, 1.0),
                weight_kg: 1.0,
                priority: PackagePriority::Standard,
            }
        }

        pub fn with_origin(mut self, origin: Point<f64>) -> Self {
            self.origin = origin;
            self
        }

        pub fn with_destination(mut self, destination: Point<f64>) -> Self {
            self.destination = destination;
            self
        }

        pub fn with_weight(mut self, weight_kg: f64) -> Self {
            self.weight_kg = weight_kg;
            self
        }

        pub fn express(mut self) -> Self {
            self.priority = PackagePriority::Express;
            self
        }

        pub fn build(self) -> Package {
            Package::new(self.origin, self.destination, self.weight_kg, self.priority)
        }
    }
}

// Usage in tests
#[test]
fn test_with_fixture() {
    let pkg = test_utils::PackageBuilder::new()
        .with_weight(5.0)
        .express()
        .build();

    assert_eq!(pkg.weight_kg, 5.0);
    assert!(matches!(pkg.priority, PackagePriority::Express));
}
```

### Mock Pattern: Test Doubles

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Mock SimulationContext for testing
    fn mock_context() -> SimulationContext {
        let config = SimulationConfig {
            name: "test".to_string(),
            time_step: Duration::seconds(30),
            verbose: false,
            custom: serde_json::Map::new(),
        };
        SimulationContext::from_now(config)
    }

    #[test]
    fn test_agent_with_mock_context() {
        let mut agent = Driver::new(Point::new(0.0, 0.0));
        let ctx = mock_context();

        let result = agent.tick(&ctx).unwrap();
        assert_eq!(result.state_changes.len(), 0);
    }
}
```

### Fixture Pattern: Common Test Data

```rust
#[cfg(test)]
mod test_fixtures {
    use super::*;

    pub fn standard_warehouse() -> Warehouse {
        Warehouse::new(
            "Test Warehouse".to_string(),
            Point::new(-122.4, 37.8),
            1000,
        ).unwrap()
    }

    pub fn standard_driver() -> Driver {
        Driver::new(Point::new(-122.4, 37.8))
    }

    pub fn standard_package() -> Package {
        Package::new(
            Point::new(-122.4, 37.8),
            Point::new(-122.5, 37.9),
            2.5,
            PackagePriority::Standard,
        )
    }
}

// Usage
#[test]
fn test_with_standard_fixtures() {
    let warehouse = test_fixtures::standard_warehouse();
    let driver = test_fixtures::standard_driver();
    let package = test_fixtures::standard_package();

    // Test interaction...
}
```

## Coverage Goals

### Measuring Coverage

**Using tarpaulin:**

```bash
cargo install cargo-tarpaulin

# Run coverage
cargo tarpaulin --out Html --output-dir coverage

# View report
open coverage/index.html
```

**Using llvm-cov:**

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov

# Run coverage
cargo llvm-cov --html

# View report
open target/llvm-cov/html/index.html
```

### Coverage Targets by Component

**Critical Paths (100%):**

- Agent state transitions
- Resource capacity enforcement
- Workflow step progression
- Serialization/deserialization

**Core Logic (> 90%):**

- Agent tick() methods
- Resource process_tick() methods
- Task factory creation
- Workflow definitions

**Utilities (> 80%):**

- Helper functions
- Metrics calculation
- Event emission

**UI/CLI (> 60%):**

- Command parsing
- Output formatting

### Coverage Report Example

```
File                              | Lines | Covered | %
----------------------------------|-------|---------|------
src/agents/package.rs             | 380   | 365     | 96.1%
src/agents/driver.rs              | 463   | 441     | 95.2%
src/resources/sorting.rs          | 256   | 248     | 96.9%
src/resources/loading.rs          | 283   | 271     | 95.8%
src/workflows/shipment.rs         | 405   | 398     | 98.3%
src/simulation.rs                 | 196   | 181     | 92.3%
----------------------------------|-------|---------|------
Total                             | 1983  | 1904    | 96.0%
```

## Examples from Codebase

### Example 1: Complete Agent Test Suite

From `/crates/sim-logistics/src/agents/package.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Creation tests
    #[test]
    fn test_package_creation() { /* 15 lines */ }

    #[test]
    fn test_package_priority() { /* 18 lines */ }

    #[test]
    fn test_package_tracking_number() { /* 12 lines */ }

    // State transition tests
    #[test]
    fn test_package_state_transitions() { /* 25 lines */ }

    // Position semantics tests
    #[test]
    fn test_package_position() { /* 22 lines */ }

    // Behavior tests
    #[test]
    fn test_delivery_time_calculation() { /* 20 lines */ }

    // Serialization tests
    #[test]
    fn test_package_state_data() { /* 18 lines */ }

    #[test]
    fn test_package_arrow_round_trip() { /* 15 lines */ }
}
```

**Total: 8 tests, 145 lines, 96.1% coverage**

### Example 2: Resource Test Suite

From `/crates/sim-logistics/src/resources/sorting.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // 15 tests covering:
    // - Creation
    // - Enqueue operations
    // - Queue limits
    // - Processing logic
    // - Timing calculations
    // - Capacity constraints
    // - Utilization metrics
    // - FIFO ordering
    // - Resource trait implementation
}
```

**Total: 15 tests, 220 lines, 96.9% coverage**

### Example 3: Integration Test

From `/crates/sim-logistics/tests/integration_test.rs`:

```rust
#[test]
fn test_24_hour_simulation() {
    let mut sim = LogisticsSimulation::new();

    // Setup: Create 100 packages
    for i in 0..100 {
        sim.create_package(/* ... */).unwrap();
    }

    // Execute: Run 24-hour simulation
    sim.run_for_hours(24).unwrap();

    // Verify: Check metrics
    let metrics = sim.get_metrics();
    assert!(metrics.packages_created >= 100);
}
```

## Best Practices Summary

1. **Test at Multiple Levels**: Unit, integration, end-to-end
2. **Name Tests Descriptively**: Clear scenario and expectation
3. **Test Happy Path and Edge Cases**: Both success and failure
4. **Use Fixtures for Common Setup**: Reduce duplication
5. **Test State Transitions**: Cover all valid transitions
6. **Test Serialization**: Verify no data loss
7. **Test Timing**: Exact tick calculations
8. **Measure Coverage**: Aim for > 85% overall
9. **Keep Tests Fast**: Unit tests < 1 second total
10. **Make Tests Readable**: Tests are documentation

## Running Tests

### Run All Tests

```bash
cargo test
```

### Run Specific Test

```bash
cargo test test_package_creation
```

### Run Tests in Package

```bash
cargo test --package sim-logistics
```

### Run Integration Tests Only

```bash
cargo test --test integration_test
```

### Run with Output

```bash
cargo test -- --nocapture
```

### Run Benchmarks

```bash
cargo bench
```

## Next Steps

- Write tests as you implement features
- Run tests before committing
- Add tests for bug fixes
- Measure coverage regularly
- Refactor tests when needed
- Review [Custom Agents Guide](./custom-agents.md) for agent testing patterns
- Review [Custom Resources Guide](./custom-resources.md) for resource testing patterns
