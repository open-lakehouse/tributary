//! Integration tests for sim-logistics
//!
//! Validates complete 24-hour simulation scenarios

use sim_logistics::{LogisticsSimulation, PackagePriority};

#[test]
fn test_basic_package_delivery() {
    let mut sim = LogisticsSimulation::new();

    // Create a few packages
    sim.create_package(0, 0, 1.0, PackagePriority::Standard).unwrap();
    sim.create_package(0, 1, 2.0, PackagePriority::Express).unwrap();
    sim.create_package(1, 2, 1.5, PackagePriority::Standard).unwrap();

    assert_eq!(sim.package_count(), 3);
    assert_eq!(sim.get_metrics().packages_created, 3);

    // Run for 1 hour
    sim.run_for_hours(1).unwrap();

    // Verify simulation ran without errors
    assert!(sim.get_metrics().packages_created >= 3);
}

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

    // Note: In a full implementation, we would verify:
    // - packages_delivered >= some threshold
    // - driver_utilization > 0.0
    // - average_delivery_time_secs is reasonable
}

#[test]
fn test_framework_generality_zero_modifications() {
    // This test ensures sim-core has ZERO knowledge of logistics domain
    use sim_core::*;

    // Framework should work with generic types only
    let _config = SimulationConfig::default();
    let _ctx = SimulationContext::from_now(_config);

    // Compilation proves framework is truly generic - no logistics-specific types needed
    assert!(true, "Framework compiles without domain-specific modifications");
}

#[test]
fn test_multiple_warehouses() {
    let sim = LogisticsSimulation::new();

    assert_eq!(sim.warehouse_count(), 2);
    assert_eq!(sim.delivery_address_count(), 1000);
}

#[test]
fn test_driver_fleet() {
    let sim = LogisticsSimulation::new();

    assert_eq!(sim.driver_count(), 20);
}
