//! Tests for the generic validation framework using synthetic event streams.

use serde_json::json;
use sim_events::EventRecord;
use sim_validate::{DomainProfile, validate_events};

/// Build a kitchen event with the given type and order id.
fn ev(id: &str, event_type: &str, order_id: &str) -> EventRecord {
    EventRecord {
        id: id.to_string(),
        source: "/test".to_string(),
        spec_version: "1.0".to_string(),
        event_type: event_type.to_string(),
        time: "2026-01-01T00:00:00Z".to_string(),
        subject: Some(order_id.to_string()),
        data_content_type: Some("application/json".to_string()),
        data: json!({ "order_id": order_id, "customer_id": "c1" }),
    }
}

/// A well-formed single-order lifecycle.
fn good_stream() -> Vec<EventRecord> {
    vec![
        ev("e1", "io.kitchen.order.created", "o1"),
        ev("e2", "io.kitchen.order.assigned", "o1"),
        ev("e3", "io.kitchen.order.completed", "o1"),
        ev("e4", "io.kitchen.customer.eating", "o1"),
    ]
}

#[test]
fn clean_run_has_no_violations() {
    let report = validate_events(&good_stream(), &DomainProfile::kitchen());
    assert!(
        report.is_ok(),
        "expected clean report, got: {:?}",
        report.violations
    );
    assert_eq!(report.total_events, 4);
}

#[test]
fn assigned_without_created_is_flagged() {
    // o2 is assigned but never created → lifecycle-pair + count-consistency.
    let mut events = good_stream();
    events.push(ev("e5", "io.kitchen.order.assigned", "o2"));

    let report = validate_events(&events, &DomainProfile::kitchen());
    assert!(!report.is_ok());
    assert!(
        report.violations.iter().any(|v| v.rule == "lifecycle-pair"),
        "expected a lifecycle-pair violation, got: {:?}",
        report.violations
    );
}

#[test]
fn out_of_order_emission_is_flagged() {
    // completed emitted before assigned for the same order.
    let events = vec![
        ev("e1", "io.kitchen.order.created", "o1"),
        ev("e2", "io.kitchen.order.completed", "o1"),
        ev("e3", "io.kitchen.order.assigned", "o1"),
    ];
    let report = validate_events(&events, &DomainProfile::kitchen());
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.rule == "causal-ordering"),
        "expected a causal-ordering violation, got: {:?}",
        report.violations
    );
}

#[test]
fn duplicate_completion_is_flagged() {
    let mut events = good_stream();
    events.push(ev("e5", "io.kitchen.order.completed", "o1")); // duplicate completion
    let report = validate_events(&events, &DomainProfile::kitchen());
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.rule == "no-duplicate-terminal"),
        "expected a no-duplicate-terminal violation, got: {:?}",
        report.violations
    );
}

#[test]
fn capacity_breach_is_flagged() {
    // Two orders assigned, none completed, capacity 1 → peak 2 > 1.
    let events = vec![
        ev("e1", "io.kitchen.order.created", "o1"),
        ev("e2", "io.kitchen.order.created", "o2"),
        ev("e3", "io.kitchen.order.assigned", "o1"),
        ev("e4", "io.kitchen.order.assigned", "o2"),
    ];
    let profile = DomainProfile::kitchen().with_capacity(
        "io.kitchen.order.assigned",
        "io.kitchen.order.completed",
        1,
    );
    let report = validate_events(&events, &profile);
    assert!(
        report.violations.iter().any(|v| v.rule == "capacity"),
        "expected a capacity violation, got: {:?}",
        report.violations
    );
}
