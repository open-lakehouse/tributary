use super::*;
use sim_events::EventRecord;

const CREATED: &str = "io.kitchen.order.created";
const ASSIGNED: &str = "io.kitchen.order.assigned";
const COMPLETED: &str = "io.kitchen.order.completed";
const EATING: &str = "io.kitchen.customer.eating";
const DONE: &str = "io.kitchen.customer.done";

/// Customers that always order and eat for one minute
fn sim_with_eager_customers(
    customers: usize,
    kitchens: usize,
    capacity: usize,
) -> KitchenSimulation {
    let mut sim = KitchenSimulation::new(kitchens, capacity, false);
    let mut population = PopulationHandler::new();
    for i in 0..customers {
        population.add_agent(Customer::with_params(
            geo::Point::new(-122.4 + i as f64 * 0.001, 37.8),
            1.0,
            chrono::Duration::minutes(1),
        ));
    }
    sim.runner_mut().set_population_handler(population);
    sim
}

fn data(e: &EventRecord) -> serde_json::Value {
    e.data.clone()
}

fn time(e: &EventRecord) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&e.time)
        .unwrap()
        .with_timezone(&Utc)
}

fn of_type<'a>(events: &'a [EventRecord], ty: &str) -> Vec<&'a EventRecord> {
    events.iter().filter(|e| e.event_type == ty).collect()
}

#[test]
fn one_order_goes_through_the_whole_lifecycle() {
    let mut sim = sim_with_eager_customers(1, 1, 5);
    let start = sim.runner().context().current_time();
    sim.run_for_steps(22).unwrap();
    let events = sim.event_records();

    let types: Vec<&str> = events.iter().map(|e| e.event_type.as_str()).collect();
    assert_eq!(types, [CREATED, ASSIGNED, COMPLETED, EATING, DONE]);

    let created = data(&events[0]);
    let order_id = created["order_id"].as_str().unwrap();
    assert!(events.iter().all(|e| data(e)["order_id"] == order_id));
    assert_eq!(created["total_price"], 12.5);

    // Ordered in step 1, started in step 2, prepared for 8 minutes.
    let step = chrono::Duration::seconds(TIME_STEP_SECS);
    assert_eq!(time(&events[0]), start + step);
    assert_eq!(time(&events[1]), start + step * 2);
    assert_eq!(
        time(&events[2]),
        time(&events[1]) + FoodPrepWorkflow::total_duration()
    );
    assert_eq!(
        data(&events[2])["prep_time_seconds"],
        (time(&events[2]) - time(&events[0])).num_seconds()
    );
    assert_eq!(
        data(&events[2])["kitchen_id"],
        data(&events[1])["kitchen_id"]
    );
    assert_eq!(data(&events[1])["kitchen_name"], "Kitchen-1");

    let done = data(&events[4]);
    assert_eq!(done["total_price"], 12.5);
    assert_eq!(
        done["total_time_seconds"],
        (time(&events[4]) - time(&events[0])).num_seconds()
    );
    // Every event carries simulation time, never the wall clock.
    assert!(
        events
            .iter()
            .all(|e| time(e) < start + chrono::Duration::hours(1))
    );
}

#[test]
fn kitchens_never_exceed_capacity_and_orders_queue() {
    let mut sim = sim_with_eager_customers(6, 1, 2);
    sim.run_for_steps(150).unwrap();
    let events = sim.event_records();

    let mut in_progress = 0i64;
    let mut peak = 0;
    for e in &events {
        match e.event_type.as_str() {
            ASSIGNED => in_progress += 1,
            COMPLETED => in_progress -= 1,
            _ => {}
        }
        peak = peak.max(in_progress);
    }
    assert_eq!(peak, 2, "capacity 2 should be fully used, never exceeded");

    // With 6 customers and 2 slots some orders must wait to start.
    let created: HashMap<_, _> = of_type(&events, CREATED)
        .into_iter()
        .map(|e| (data(e)["order_id"].to_string(), time(e)))
        .collect();
    let waited = of_type(&events, ASSIGNED)
        .into_iter()
        .filter(|e| {
            time(e) - created[&data(e)["order_id"].to_string()]
                > chrono::Duration::seconds(TIME_STEP_SECS)
        })
        .count();
    assert!(waited > 0, "expected some orders to queue");
    assert!(of_type(&events, DONE).len() >= 6);
}

#[test]
fn orders_are_spread_across_kitchens() {
    let mut sim = sim_with_eager_customers(6, 3, 5);
    sim.run_for_steps(5).unwrap();
    let events = sim.event_records();
    let mut per_kitchen: HashMap<String, usize> = HashMap::new();
    for e in of_type(&events, ASSIGNED) {
        *per_kitchen
            .entry(data(e)["kitchen_name"].as_str().unwrap().to_string())
            .or_default() += 1;
    }
    assert_eq!(per_kitchen.len(), 3);
    assert!(per_kitchen.values().all(|&n| n == 2));
}

#[test]
fn every_order_event_matches_one_creation() {
    let mut sim = sim_with_eager_customers(5, 2, 2);
    sim.run_for_steps(120).unwrap();
    let events = sim.event_records();
    let created: Vec<String> = of_type(&events, CREATED)
        .iter()
        .map(|e| data(e)["order_id"].to_string())
        .collect();
    let unique: std::collections::HashSet<_> = created.iter().collect();
    assert_eq!(unique.len(), created.len(), "order ids are unique");
    for ty in [ASSIGNED, COMPLETED, EATING] {
        let ids: Vec<String> = of_type(&events, ty)
            .iter()
            .map(|e| data(e)["order_id"].to_string())
            .collect();
        let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique_ids.len(), ids.len(), "{ty} fires once per order");
        assert!(
            ids.iter().all(|id| unique.contains(id)),
            "{ty} for unknown order"
        );
    }
    // Every completed order was delivered in the same step.
    assert_eq!(
        of_type(&events, COMPLETED).len(),
        of_type(&events, EATING).len()
    );
}

#[test]
fn saving_events_mid_run_keeps_running() {
    // Regression: save_events cleared the dispatcher without resetting the
    // artifact cursor, so the next step panicked on an out-of-range slice.
    let dir = std::env::temp_dir().join(format!("kitchen-save-{}", uuid::Uuid::new_v4()));
    let mut sim = sim_with_eager_customers(3, 1, 5);
    sim.run_for_steps(25).unwrap();
    let written = sim.save_events(dir.join("events.parquet")).unwrap();
    assert!(written > 0);
    assert!(sim.event_records().is_empty());
    sim.run_for_steps(25).unwrap();
    assert!(!sim.event_records().is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn artifacts_are_generated_from_events() {
    let mut sim = sim_with_eager_customers(2, 1, 5);
    // step() (not run_for_steps, which flushes them to storage at the end)
    for _ in 0..25 {
        sim.step().unwrap();
    }
    assert!(sim.hooks().artifact_count() > 0);
}

#[test]
fn ordering_without_kitchens_is_an_error() {
    let mut sim = sim_with_eager_customers(1, 0, 5);
    assert!(sim.run_for_steps(5).is_err());
}

#[test]
fn no_customers_means_no_events() {
    let mut sim = sim_with_eager_customers(0, 2, 5);
    sim.run_for_steps(10).unwrap();
    assert!(sim.event_records().is_empty());
}
