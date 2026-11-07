//! Integration tests for sim-logistics

use sim_logistics::{LogisticsSimulation, PackagePriority};

#[test]
fn manual_packages_are_delivered_within_an_hour() {
    let mut sim = LogisticsSimulation::new_seeded(Some(1));
    sim.create_package(0, 0, 1.0, PackagePriority::Standard)
        .unwrap();
    sim.create_package(0, 1, 2.0, PackagePriority::Express)
        .unwrap();
    sim.create_package(1, 2, 1.5, PackagePriority::Standard)
        .unwrap();
    assert_eq!(sim.package_count(), 3);
    assert_eq!(sim.get_metrics().packages_created, 3);

    sim.run_for_hours(1).unwrap();

    // The pipeline takes 42 minutes, so the three manual packages (and any
    // early demand-generated ones) are through.
    let metrics = sim.get_metrics();
    assert!(metrics.packages_delivered >= 3, "{metrics:?}");
}

#[test]
fn day_long_run_delivers_the_backlog() {
    let mut sim = LogisticsSimulation::new_seeded(Some(2));
    for i in 0..100 {
        let priority = if i % 3 == 0 {
            PackagePriority::Express
        } else {
            PackagePriority::Standard
        };
        sim.create_package(i % 2, i * 10, 1.0 + (i % 5) as f64 * 0.5, priority)
            .unwrap();
    }
    assert_eq!(sim.package_count(), 100);

    sim.run_for_hours(24).unwrap();

    let metrics = sim.get_metrics();
    // ~10 demand packages per hour on top of the backlog; everything created
    // more than an hour before the end is delivered.
    assert!(metrics.packages_created >= 300, "{metrics:?}");
    assert!(
        metrics.packages_delivered + 15 >= metrics.packages_created,
        "{metrics:?}"
    );
    assert_eq!(metrics.drivers_used, 20);
    assert!(metrics.driver_utilization_percent > 0.0);
    assert!(metrics.average_delivery_time_secs >= 42.0 * 60.0);
}

#[test]
fn default_layout() {
    let sim = LogisticsSimulation::new();
    assert_eq!(sim.warehouse_count(), 2);
    assert_eq!(sim.delivery_address_count(), 1000);
    assert_eq!(sim.driver_count(), 20);
}
