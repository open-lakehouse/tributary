//! Determinism: a fixed seed reproduces the event stream exactly
//!
//! Two runs with the same seed and start time must emit the same events in
//! the same order, with identical type, subject, time and payload. Only the
//! CloudEvents `id` (a fresh UUID v7 per event) may differ.

use chrono::{DateTime, Utc};
use sim_events::EventRecord;
use sim_kitchen::KitchenSimulation;

const STEPS: usize = 240;

fn run_seeded(seed: u64) -> Vec<EventRecord> {
    let mut sim = KitchenSimulation::new_seeded(3, 5, false, Some(seed));
    let start: DateTime<Utc> = "2026-01-05T11:00:00Z".parse().unwrap();
    sim.runner_mut().context_mut().set_current_time(start);
    sim.add_customers(20);
    sim.run_for_steps(STEPS).expect("simulation should run");
    sim.event_records()
}

fn logical(events: &[EventRecord]) -> Vec<(String, Option<String>, String, serde_json::Value)> {
    events
        .iter()
        .map(|e| {
            (
                e.event_type.clone(),
                e.subject.clone(),
                e.time.clone(),
                e.data.clone(),
            )
        })
        .collect()
}

#[test]
fn same_seed_reproduces_the_event_stream_in_order() {
    let a = run_seeded(42);
    let b = run_seeded(42);
    assert!(
        a.len() > 100,
        "the run should do real work, got {} events",
        a.len()
    );
    assert_eq!(logical(&a), logical(&b));
}

#[test]
fn different_seeds_diverge() {
    assert_ne!(logical(&run_seeded(1)), logical(&run_seeded(999)));
}
