use super::*;
use sim_events::EventRecord;

const RECEIVED: &str = "io.logistics.package.received";
const SORTED: &str = "io.logistics.package.sorted";
const LOADED: &str = "io.logistics.package.loaded";
const IN_TRANSIT: &str = "io.logistics.package.in_transit";
const DELIVERED: &str = "io.logistics.package.delivered";

fn time(e: &EventRecord) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&e.time)
        .unwrap()
        .with_timezone(&Utc)
}

#[test]
fn packages_are_delivered_end_to_end() {
    // Regression: only sorting stations were registered, so every shipment
    // waited forever for a "package_receiver" and nothing was delivered.
    let mut sim = LogisticsSimulation::new_seeded(Some(7));
    sim.run_for_hours(4).unwrap();
    let m = sim.get_metrics();
    assert!(m.packages_created > 0);
    assert!(m.packages_delivered > 0, "nothing delivered: {m:?}");
    assert!(m.packages_delivered <= m.packages_created);
    assert!(m.drivers_used > 0);
    // Pipeline is 42 min of work after creation, plus up to a step of waiting.
    assert!(m.average_delivery_time_secs >= 42.0 * 60.0);
    assert!(m.driver_utilization_percent > 0.0 && m.driver_utilization_percent <= 100.0);
}

#[test]
fn one_package_emits_every_stage_in_order() {
    let mut sim = LogisticsSimulation::new_seeded(Some(1));
    sim.runner_mut()
        .set_demand_generator(Box::new(PoissonDemandGenerator::with_seed(0.0, "none", 1)));
    sim.create_package(0, 0, 2.0, PackagePriority::Express)
        .unwrap();
    for _ in 0..100 {
        sim.step().unwrap();
    }
    let events = sim.event_records();
    let types: Vec<&str> = events.iter().map(|e| e.event_type.as_str()).collect();
    assert_eq!(types, [RECEIVED, SORTED, LOADED, IN_TRANSIT, DELIVERED]);
    assert!(events.iter().all(|e| e.data["package_id"] == "PKG-000001"));
    assert_eq!(events[0].data["warehouse_id"], "Warehouse North");

    let driver = &events[3].data["driver_id"];
    assert!(driver.as_str().unwrap().starts_with("Driver-"));
    assert_eq!(&events[4].data["driver_id"], driver);
    assert_eq!(&events[2].data["driver_id"], driver);
    assert!(
        events[2].data["loading_dock_id"]
            .as_str()
            .unwrap()
            .starts_with("Dock-")
    );
    assert!(
        events[1].data["sorting_station_id"]
            .as_str()
            .unwrap()
            .starts_with("Sorting-")
    );

    // Transit takes 30 minutes and confirmation one more.
    assert_eq!(time(&events[4]) - time(&events[3]), Duration::minutes(31));

    let population = sim.runner().population().unwrap();
    let pkg = population.iter().next().unwrap();
    assert_eq!(pkg.stage, PackageStage::Delivered);
    assert_eq!(pkg.priority, PackagePriority::Express);
    assert_eq!(sim.get_metrics().packages_delivered, 1);
}

#[test]
fn drivers_carry_one_package_at_a_time() {
    let mut sim = LogisticsSimulation::new_seeded(Some(3));
    for i in 0..60 {
        sim.create_package(i % 2, i, 1.0, PackagePriority::Standard)
            .unwrap();
    }
    sim.run_for_hours(3).unwrap();
    let events = sim.event_records();
    // A driver is free again when transit ends (30 min); confirmation is done
    // by a separate station. So one driver's pickups are >= 30 min apart.
    let mut last_pickup: HashMap<String, DateTime<Utc>> = HashMap::new();
    let mut pickups = 0;
    for e in events.iter().filter(|e| e.event_type == IN_TRANSIT) {
        let driver = e.data["driver_id"].as_str().unwrap().to_string();
        if let Some(prev) = last_pickup.insert(driver, time(e)) {
            assert!(time(e) - prev >= Duration::minutes(30));
        }
        pickups += 1;
    }
    assert!(
        pickups >= 60,
        "the 60 manual packages (plus demand) were picked up"
    );
    // 60 packages and 20 drivers: the fleet runs at full capacity for a while.
    assert_eq!(sim.get_metrics().drivers_used, 20);
}

#[test]
fn seeded_runs_are_reproducible() {
    let run = || {
        let mut sim = LogisticsSimulation::new_seeded(Some(11));
        let start: DateTime<Utc> = "2026-01-05T08:00:00Z".parse().unwrap();
        sim.runner_mut().context_mut().set_current_time(start);
        sim.run_for_hours(3).unwrap();
        sim.event_records()
            .into_iter()
            .map(|e| (e.event_type, e.subject, e.time, e.data))
            .collect::<Vec<_>>()
    };
    let a = run();
    assert!(!a.is_empty());
    assert_eq!(a, run());
}

#[test]
fn event_times_are_simulation_time() {
    let mut sim = LogisticsSimulation::new_seeded(Some(5));
    let start: DateTime<Utc> = "2020-06-01T00:00:00Z".parse().unwrap();
    sim.runner_mut().context_mut().set_current_time(start);
    sim.run_for_hours(2).unwrap();
    let events = sim.event_records();
    assert!(!events.is_empty());
    assert!(
        events
            .iter()
            .all(|e| time(e) >= start && time(e) <= start + Duration::hours(2))
    );
}

#[test]
fn invalid_indices_are_rejected() {
    let mut sim = LogisticsSimulation::new();
    assert!(
        sim.create_package(9, 0, 1.0, PackagePriority::Standard)
            .is_err()
    );
    assert!(
        sim.create_package(0, 5000, 1.0, PackagePriority::Standard)
            .is_err()
    );
}
