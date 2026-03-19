//! Integration tests that validate end-to-end simulation correctness through
//! structural invariants on the event stream.
//!
//! These tests use deterministic scenarios (order_probability = 1.0) and assert
//! that event lifecycles are complete, counts are consistent, timestamps are
//! causally ordered, and capacity constraints are respected. They are designed
//! to catch regressions where functionality is silently disabled.

use geo::Point;
use sim_core::PopulationHandler;
use sim_events::EventRecord;
use sim_kitchen::{
    Customer, CustomerStatus, Inventory, InventoryManager, KitchenSimulation,
    KitchenSimulationHooks,
};
use std::collections::{HashMap, HashSet};

// ===== Event Type Constants =====

const ORDER_CREATED: &str = "io.kitchen.order.created";
const ORDER_ASSIGNED: &str = "io.kitchen.order.assigned";
const ORDER_COMPLETED: &str = "io.kitchen.order.completed";
const CUSTOMER_EATING: &str = "io.kitchen.customer.eating";
const CUSTOMER_DONE: &str = "io.kitchen.customer.done";
const INVENTORY_CONSUMED: &str = "io.kitchen.inventory.consumed";
const INVENTORY_LOW: &str = "io.kitchen.inventory.low";
const SUPPLY_ORDER_CREATED: &str = "io.kitchen.supply.order.created";

// ===== EventInvariantChecker =====

struct EventInvariantChecker {
    events: Vec<EventRecord>,
}

impl EventInvariantChecker {
    fn new(events: Vec<EventRecord>) -> Self {
        Self { events }
    }

    fn count(&self, event_type: &str) -> usize {
        self.events
            .iter()
            .filter(|e| e.event_type == event_type)
            .count()
    }

    fn events_of_type(&self, event_type: &str) -> Vec<&EventRecord> {
        self.events
            .iter()
            .filter(|e| e.event_type == event_type)
            .collect()
    }

    fn extract_order_id(record: &EventRecord) -> Option<String> {
        record
            .data
            .get("order_id")
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    fn extract_customer_id(record: &EventRecord) -> Option<String> {
        record
            .data
            .get("customer_id")
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    #[allow(dead_code)]
    fn extract_kitchen_id(record: &EventRecord) -> Option<String> {
        record
            .data
            .get("kitchen_id")
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    /// Invariant 1: For every order.created, there should be a corresponding
    /// order.assigned and order.completed (for completed orders), and for every
    /// order.completed, a matching customer.eating and customer.done.
    fn assert_order_lifecycle_complete(&self) {
        let created_orders: HashSet<String> = self
            .events_of_type(ORDER_CREATED)
            .iter()
            .filter_map(|e| Self::extract_order_id(e))
            .collect();

        let assigned_orders: HashSet<String> = self
            .events_of_type(ORDER_ASSIGNED)
            .iter()
            .filter_map(|e| Self::extract_order_id(e))
            .collect();

        let completed_orders: HashSet<String> = self
            .events_of_type(ORDER_COMPLETED)
            .iter()
            .filter_map(|e| Self::extract_order_id(e))
            .collect();

        let eating_orders: HashSet<String> = self
            .events_of_type(CUSTOMER_EATING)
            .iter()
            .filter_map(|e| Self::extract_order_id(e))
            .collect();

        // Every assigned order must have been created
        for order_id in &assigned_orders {
            assert!(
                created_orders.contains(order_id),
                "Order {} was assigned but never created",
                order_id
            );
        }

        // Every completed order must have been assigned
        for order_id in &completed_orders {
            assert!(
                assigned_orders.contains(order_id),
                "Order {} was completed but never assigned",
                order_id
            );
        }

        // Every order that triggered customer eating must have been completed
        for order_id in &eating_orders {
            assert!(
                completed_orders.contains(order_id),
                "Customer started eating order {} but it was never completed",
                order_id
            );
        }

        // Every completed order should have a corresponding customer.eating
        for order_id in &completed_orders {
            assert!(
                eating_orders.contains(order_id),
                "Order {} was completed but customer never started eating",
                order_id
            );
        }
    }

    /// Invariant 2: Event counts must be consistent across the pipeline.
    fn assert_event_counts_consistent(&self) {
        let created = self.count(ORDER_CREATED);
        let assigned = self.count(ORDER_ASSIGNED);
        let completed = self.count(ORDER_COMPLETED);
        let eating = self.count(CUSTOMER_EATING);
        let done = self.count(CUSTOMER_DONE);

        assert!(
            assigned <= created,
            "More orders assigned ({}) than created ({})",
            assigned,
            created
        );

        assert_eq!(
            completed, assigned,
            "Completed orders ({}) != assigned orders ({}); assigned orders must eventually complete",
            completed, assigned
        );

        assert_eq!(
            eating, completed,
            "Customer eating events ({}) != completed orders ({})",
            eating, completed
        );

        assert_eq!(
            done, eating,
            "Customer done events ({}) != customer eating events ({})",
            done, eating
        );
    }

    /// Invariant 3: For each order, events must appear in causal order within
    /// the event stream: created before assigned before completed. For each
    /// customer, eating must appear before done.
    ///
    /// Note: Event::new() uses wallclock Utc::now() for the `time` field, so
    /// we validate emission order (buffer position) rather than timestamps.
    fn assert_causal_ordering(&self) {
        // Build per-order emission positions
        let mut order_positions: HashMap<String, Vec<(&str, usize)>> = HashMap::new();

        for (idx, record) in self.events.iter().enumerate() {
            let etype = record.event_type.as_str();
            if matches!(etype, ORDER_CREATED | ORDER_ASSIGNED | ORDER_COMPLETED) {
                if let Some(order_id) = Self::extract_order_id(record) {
                    order_positions
                        .entry(order_id)
                        .or_default()
                        .push((etype, idx));
                }
            }
        }

        let type_order = [ORDER_CREATED, ORDER_ASSIGNED, ORDER_COMPLETED];
        for (order_id, mut positions) in order_positions {
            positions.sort_by_key(|(t, _)| type_order.iter().position(|&x| x == *t).unwrap_or(99));

            for window in positions.windows(2) {
                let (type_a, pos_a) = &window[0];
                let (type_b, pos_b) = &window[1];
                assert!(
                    pos_b > pos_a,
                    "Order {}: {} (position {}) should appear before {} (position {})",
                    order_id,
                    type_a,
                    pos_a,
                    type_b,
                    pos_b,
                );
            }
        }

        // Build per-customer eating→done positions
        let mut customer_eating_pos: HashMap<String, usize> = HashMap::new();
        for (idx, record) in self.events.iter().enumerate() {
            if record.event_type == CUSTOMER_EATING {
                if let Some(cid) = Self::extract_customer_id(record) {
                    customer_eating_pos.insert(cid, idx);
                }
            }
        }

        for (idx, record) in self.events.iter().enumerate() {
            if record.event_type == CUSTOMER_DONE {
                if let Some(cid) = Self::extract_customer_id(record) {
                    if let Some(&eating_pos) = customer_eating_pos.get(&cid) {
                        assert!(
                            idx > eating_pos,
                            "Customer {}: done (position {}) before eating (position {})",
                            cid,
                            idx,
                            eating_pos,
                        );
                    }
                }
            }
        }
    }

    /// Invariant 4: No two events should share the same (event_type, order_id)
    /// or (event_type, customer_id) where that would indicate a duplicate.
    fn assert_no_duplicate_events(&self) {
        // Order events keyed by order_id
        for event_type in [ORDER_CREATED, ORDER_ASSIGNED, ORDER_COMPLETED] {
            let mut seen: HashSet<String> = HashSet::new();
            for record in self.events_of_type(event_type) {
                if let Some(order_id) = Self::extract_order_id(record) {
                    assert!(
                        seen.insert(order_id.clone()),
                        "Duplicate {} event for order {}",
                        event_type,
                        order_id,
                    );
                }
            }
        }

        // Customer events keyed by customer_id
        for event_type in [CUSTOMER_EATING, CUSTOMER_DONE] {
            let mut seen: HashSet<String> = HashSet::new();
            for record in self.events_of_type(event_type) {
                if let Some(customer_id) = Self::extract_customer_id(record) {
                    assert!(
                        seen.insert(customer_id.clone()),
                        "Duplicate {} event for customer {}",
                        event_type,
                        customer_id,
                    );
                }
            }
        }
    }

    /// Invariant 5: At no point should the number of concurrently assigned
    /// (but not yet completed) orders exceed the per-kitchen capacity.
    ///
    /// Uses emission order: each assigned event increments load, each
    /// completed event decrements it. Peak load must stay within capacity.
    fn assert_resource_capacity_respected(&self, kitchen_capacity: usize) {
        let mut current_load: i32 = 0;
        let mut peak_load: i32 = 0;

        for record in &self.events {
            match record.event_type.as_str() {
                ORDER_ASSIGNED => current_load += 1,
                ORDER_COMPLETED => current_load -= 1,
                _ => {}
            }
            if current_load > peak_load {
                peak_load = current_load;
            }
        }

        assert!(
            peak_load <= kitchen_capacity as i32,
            "Peak concurrent load ({}) exceeded kitchen capacity ({})",
            peak_load,
            kitchen_capacity,
        );
    }

    /// Run all invariant checks (except capacity, which needs a parameter).
    fn assert_all_invariants(&self) {
        self.assert_order_lifecycle_complete();
        self.assert_event_counts_consistent();
        self.assert_causal_ordering();
        self.assert_no_duplicate_events();
    }
}

// ===== Helpers =====

/// Create a KitchenSimulation with deterministic customers (order_probability = 1.0).
///
/// Lifecycle timing with 30s steps and 1-minute eating:
///   Tick 1: Idle → Ordering (prob=1.0 is preserved through Arrow round-trip)
///   Tick 2: Ordering → WaitingForFood + demand → OrderCreated
///   After tick 2: allocated → OrderAssigned
///   Tick 12: 5 min elapsed → OrderCompleted
///   After tick 12: deliver → CustomerEating (finish_time = current + 1min)
///   Tick ~15: Eating → Done → CustomerDone
///
/// Total ticks needed per customer: ~15. We use 20 for safety.
fn setup_deterministic_sim(
    num_customers: usize,
    num_kitchens: usize,
    kitchen_capacity: usize,
) -> KitchenSimulation {
    let mut sim = KitchenSimulation::new(num_kitchens, kitchen_capacity, false);

    let mut population = PopulationHandler::new();
    for i in 0..num_customers {
        let customer = Customer::with_params(
            Point::new(-122.4 + (i as f64 * 0.001), 37.8),
            1.0,
            chrono::Duration::minutes(1),
        );
        population.add_agent(customer);
    }
    sim.runner_mut().set_population_handler(population);

    sim
}

/// Number of steps needed for a single customer to complete the full lifecycle.
const STEPS_FOR_FULL_LIFECYCLE: usize = 20;

// ===== Tests =====

/// Single customer goes through the entire lifecycle. Verifies all 5 event
/// types are emitted exactly once and in causal order.
#[test]
fn test_single_order_full_lifecycle() {
    let mut sim = setup_deterministic_sim(1, 1, 5);

    sim.run_for_steps(STEPS_FOR_FULL_LIFECYCLE)
        .expect("simulation should complete");

    let checker = EventInvariantChecker::new(sim.collect_all_event_records());

    assert_eq!(
        checker.count(ORDER_CREATED),
        1,
        "should have exactly 1 order created"
    );
    assert_eq!(
        checker.count(ORDER_ASSIGNED),
        1,
        "should have exactly 1 order assigned"
    );
    assert_eq!(
        checker.count(ORDER_COMPLETED),
        1,
        "should have exactly 1 order completed"
    );
    assert_eq!(
        checker.count(CUSTOMER_EATING),
        1,
        "should have exactly 1 customer eating"
    );
    assert_eq!(
        checker.count(CUSTOMER_DONE),
        1,
        "should have exactly 1 customer done"
    );

    checker.assert_all_invariants();

    // Verify the customer actually ended up in Done state
    let pop = sim.runner().population().unwrap();
    for customer in pop.iter() {
        assert!(
            matches!(customer.status, CustomerStatus::Done),
            "Customer should be Done, got {:?}",
            customer.status,
        );
    }
}

/// Five customers all complete the lifecycle. Validates invariants hold
/// with concurrent orders across multiple kitchens.
#[test]
fn test_all_orders_complete_lifecycle() {
    let mut sim = setup_deterministic_sim(5, 3, 10);

    sim.run_for_steps(STEPS_FOR_FULL_LIFECYCLE)
        .expect("simulation should complete");

    let checker = EventInvariantChecker::new(sim.collect_all_event_records());

    assert_eq!(checker.count(ORDER_CREATED), 5);
    assert_eq!(checker.count(ORDER_ASSIGNED), 5);
    assert_eq!(checker.count(ORDER_COMPLETED), 5);
    assert_eq!(checker.count(CUSTOMER_EATING), 5);
    assert_eq!(checker.count(CUSTOMER_DONE), 5);

    checker.assert_all_invariants();
}

/// With limited kitchen capacity (1 kitchen, capacity 2), orders must queue.
/// Validates that peak concurrent load never exceeds capacity.
#[test]
fn test_capacity_constrained_orders_queue() {
    let mut sim = setup_deterministic_sim(5, 1, 2);

    // With capacity 2, orders process in batches: first 2 assigned, remaining 3
    // queue. Each batch: ~12 ticks cooking + 2-3 eating. Need enough steps for
    // all 5 orders to complete, but capacity constraints mean batching.
    // ResourceAllocator capacity is based on pending tasks, not completion, so
    // all 5 orders get created but only 2 are assigned at a time.
    sim.run_for_steps(80).expect("simulation should complete");

    let checker = EventInvariantChecker::new(sim.collect_all_event_records());

    // All orders should eventually be created
    assert_eq!(checker.count(ORDER_CREATED), 5);

    // All assigned orders must complete
    let assigned = checker.count(ORDER_ASSIGNED);
    let completed = checker.count(ORDER_COMPLETED);
    assert_eq!(assigned, completed, "every assigned order must complete");

    // Capacity constraint: never more than 2 orders in-flight at once
    checker.assert_resource_capacity_respected(2);
    checker.assert_causal_ordering();
    checker.assert_no_duplicate_events();
}

/// Verifies that the count of order.completed (from hooks dispatcher) matches
/// customer.eating (from simulation dispatcher) exactly.
#[test]
fn test_event_counts_match_across_dispatchers() {
    let mut sim = setup_deterministic_sim(3, 2, 10);

    sim.run_for_steps(STEPS_FOR_FULL_LIFECYCLE)
        .expect("simulation should complete");

    // Collect from hooks dispatcher
    let hooks_events: Vec<EventRecord> = sim
        .runner()
        .hooks_as::<KitchenSimulationHooks>()
        .expect("hooks should be KitchenSimulationHooks")
        .dispatcher()
        .to_records();

    let hooks_completed = hooks_events
        .iter()
        .filter(|e| e.event_type == ORDER_COMPLETED)
        .count();

    // Collect from simulation dispatcher
    let all_events = sim.collect_all_event_records();
    let sim_eating = all_events
        .iter()
        .filter(|e| e.event_type == CUSTOMER_EATING)
        .count();

    assert_eq!(
        hooks_completed, sim_eating,
        "order.completed count ({}) from hooks must equal customer.eating count ({}) from simulation",
        hooks_completed, sim_eating,
    );

    assert!(
        hooks_completed > 0,
        "should have at least one completed order"
    );
}

/// Zero customers means zero events. Catches accidental event emission.
#[test]
fn test_no_events_lost_on_empty_simulation() {
    let mut sim = setup_deterministic_sim(0, 2, 5);

    sim.run_for_steps(10).expect("simulation should complete");

    let checker = EventInvariantChecker::new(sim.collect_all_event_records());

    assert_eq!(checker.count(ORDER_CREATED), 0);
    assert_eq!(checker.count(ORDER_ASSIGNED), 0);
    assert_eq!(checker.count(ORDER_COMPLETED), 0);
    assert_eq!(checker.count(CUSTOMER_EATING), 0);
    assert_eq!(checker.count(CUSTOMER_DONE), 0);
    assert_eq!(checker.count(INVENTORY_CONSUMED), 0);
    assert_eq!(checker.count(INVENTORY_LOW), 0);
    assert_eq!(checker.count(SUPPLY_ORDER_CREATED), 0);
}

/// Kitchen with low initial inventory triggers supply chain events when
/// orders consume the remaining stock.
#[test]
fn test_inventory_lifecycle_triggers_supply_events() {
    use sim_core::{Resource, ResourceAllocator, SimulationConfig, SimulationRunner};
    use sim_kitchen::{locations::DistributionSite, simulation::FoodPrepWorkflow};

    // Build the simulation manually so we know the kitchen ID
    let config = SimulationConfig {
        name: "inventory-test".to_string(),
        time_step: chrono::Duration::seconds(30),
        verbose: false,
        ..Default::default()
    };
    let mut runner: SimulationRunner<
        Customer,
        sim_kitchen::Kitchen,
        sim_kitchen::Order,
        FoodPrepWorkflow,
    > = SimulationRunner::new(config);

    let mut allocator = ResourceAllocator::new();
    let kitchen = sim_kitchen::Kitchen::new("Kitchen-1".to_string(), 5);
    let kitchen_id = kitchen.id().as_uuid();
    allocator.add_resource(kitchen).unwrap();
    runner.set_resource_allocator(allocator);

    // Set up hooks with low inventory
    let mut hooks = KitchenSimulationHooks::new();

    let mut manager = InventoryManager::new();
    // Very low stock: a single order consumes flour=2, beef=1, lettuce=1
    manager.add_item(Inventory::new("flour".to_string(), 3, 100, 30, 50));
    manager.add_item(Inventory::new("beef".to_string(), 2, 100, 30, 50));
    manager.add_item(Inventory::new("lettuce".to_string(), 2, 100, 30, 50));
    hooks.inventory_managers.insert(kitchen_id, manager);

    // Add a distribution site so supply orders can be created
    hooks.distribution_sites.push(DistributionSite::new(
        "Test Warehouse".to_string(),
        Point::new(-122.42, 37.78),
        vec![
            "flour".to_string(),
            "beef".to_string(),
            "lettuce".to_string(),
        ],
    ));

    runner.set_hooks(Box::new(hooks));

    // Add a single customer that will order immediately
    let mut population = PopulationHandler::new();
    let customer =
        Customer::with_params(Point::new(-122.4, 37.8), 1.0, chrono::Duration::minutes(1));
    population.add_agent(customer);
    runner.set_population_handler(population);

    // Replicate the run_for_steps logic since KitchenSimulation::new creates
    // its own runner and we need to inject the inventory manager above.
    use sim_core::resources::LeastLoaded;

    for _ in 0..5 {
        runner.tick().unwrap();

        if let Some(hooks) = runner.hooks_as_mut::<KitchenSimulationHooks>() {
            let new_orders = hooks.drain_pending_orders();
            if !new_orders.is_empty() {
                if let Some(allocator) = runner.resources_mut() {
                    for order in new_orders {
                        let _ = allocator.add_task(order);
                    }
                }
            }
        }

        let _ = runner.allocate_resources(&LeastLoaded);
    }

    // Collect events from hooks dispatcher
    let events: Vec<EventRecord> = runner
        .hooks_as::<KitchenSimulationHooks>()
        .expect("hooks should be KitchenSimulationHooks")
        .dispatcher()
        .to_records();

    let checker = EventInvariantChecker::new(events);

    // Order should have been created and assigned
    assert!(checker.count(ORDER_CREATED) >= 1, "order should be created");
    assert!(
        checker.count(ORDER_ASSIGNED) >= 1,
        "order should be assigned"
    );

    // Inventory should have been consumed
    assert!(
        checker.count(INVENTORY_CONSUMED) >= 1,
        "inventory should be consumed on assignment"
    );

    // With stock at 3 flour and consumption of 2, remaining is 1 which is below
    // reorder_point of 30, so low inventory should trigger
    assert!(
        checker.count(INVENTORY_LOW) >= 1,
        "low inventory event should fire when stock drops below reorder point"
    );

    // Supply order should be created since we have a distribution site
    assert!(
        checker.count(SUPPLY_ORDER_CREATED) >= 1,
        "supply order should be created when inventory is low and a distribution site is available"
    );
}

/// Running the same deterministic scenario twice produces identical event counts.
/// With order_probability = 1.0 the RNG branch in Customer::tick is never taken,
/// so results must be fully reproducible.
#[test]
fn test_deterministic_ordering_no_rng_flakiness() {
    fn run_scenario() -> HashMap<String, usize> {
        let mut sim = setup_deterministic_sim(3, 2, 10);
        sim.run_for_steps(STEPS_FOR_FULL_LIFECYCLE)
            .expect("simulation should complete");

        let events = sim.collect_all_event_records();
        let mut counts: HashMap<String, usize> = HashMap::new();
        for e in &events {
            *counts.entry(e.event_type.clone()).or_insert(0) += 1;
        }
        counts
    }

    let run_1 = run_scenario();
    let run_2 = run_scenario();

    assert_eq!(
        run_1, run_2,
        "Two runs with order_probability=1.0 should produce identical event counts.\nRun 1: {:?}\nRun 2: {:?}",
        run_1, run_2,
    );

    // Sanity: there should actually be events
    assert!(
        run_1.values().sum::<usize>() > 0,
        "Runs should produce events"
    );
}
