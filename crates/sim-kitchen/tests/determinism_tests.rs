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

/// Every artifact file of a seeded run, with the event references removed
///
/// `source_event_id` points at a CloudEvents `id`, which is random; every
/// other field (session and message IDs included) must reproduce.
fn artifacts_seeded(seed: u64) -> std::collections::BTreeMap<String, Vec<serde_json::Value>> {
    static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("kitchen-determinism-{}-{run}", std::process::id()));
    let mut sim = KitchenSimulation::new_seeded(3, 5, false, Some(seed)).with_artifact_dir(&dir);
    let start: DateTime<Utc> = "2026-01-05T11:00:00Z".parse().unwrap();
    sim.runner_mut().context_mut().set_current_time(start);
    sim.add_customers(20);
    sim.run_for_steps(STEPS).expect("simulation should run");

    let mut files = std::collections::BTreeMap::new();
    let mut pending = vec![dir.clone()];
    while let Some(d) = pending.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let records = std::fs::read_to_string(&path)
                .unwrap()
                .lines()
                .map(|l| {
                    let mut v: serde_json::Value = serde_json::from_str(l).unwrap();
                    if let Some(o) = v.as_object_mut() {
                        o.remove("source_event_id");
                    }
                    v
                })
                .collect();
            let rel = path.strip_prefix(&dir).unwrap().display().to_string();
            files.insert(rel, records);
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
    files
}

#[test]
fn same_seed_reproduces_the_artifacts() {
    let a = artifacts_seeded(42);
    assert!(a.len() > 5, "expected several artifact files, got {a:?}");
    assert_eq!(a, artifacts_seeded(42));
}
