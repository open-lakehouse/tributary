//! EventProcessor adapter for TelemetryCollector.
//!
//! Wraps a TelemetryCollector as an EventProcessor so it can be registered
//! with EventDispatcher and receive events automatically.
//!
//! # Workstream A dependency
//!
//! The `EventProcessor` trait is expected to be defined in `sim-events` as part of
//! Workstream A. Adding `sim-events` as a direct dependency of `sim-telemetry` would
//! create a circular dependency (`sim-core` → `sim-telemetry` → `sim-events` →
//! `sim-core`), so the trait is defined locally here using `serde_json::Value` as
//! the event representation (matching what `TelemetryCollector::process_event` already
//! accepts).
//!
//! Once the dependency graph is restructured to break the cycle (e.g., by moving
//! `EventProcessor` to a small `sim-events-core` crate that has no dependency on
//! `sim-core`), this local definition can be replaced with:
//!
//! ```rust,ignore
//! use sim_events::EventProcessor;
//! ```
//!
//! TODO: implement EventProcessor once sim-events exports it without introducing a
//! circular dependency.

use crate::core::{TelemetryCollector, TelemetryRecord};
use serde_json::Value;

/// Placeholder trait for event processing.
///
/// This mirrors the `EventProcessor` interface expected from Workstream A, using
/// `serde_json::Value` as the event type to avoid a circular dependency on
/// `sim-events`. Replace with `sim_events::EventProcessor` once the cycle is broken.
pub trait EventProcessor {
    /// Returns `true` if this processor should handle events of the given type.
    fn handles(&self, event_type: &str) -> bool;

    /// Process a single event represented as a CloudEvents JSON value.
    ///
    /// Implementations should use interior mutability (e.g. `Arc<Mutex<_>>`) if
    /// they need to mutate state, since dispatchers typically hold `&dyn EventProcessor`.
    fn process(&self, event: &Value);
}

/// Adapts TelemetryCollector to the EventProcessor interface.
///
/// Register this with EventDispatcher to automatically collect telemetry
/// from all events without modifying the simulation hooks.
///
/// # Interior Mutability
///
/// `TelemetryCollector` already uses `Arc<Mutex<>>` internally, so this struct
/// can implement `EventProcessor::process(&self, …)` without requiring `&mut self`.
pub struct TelemetryCollectorProcessor {
    collector: TelemetryCollector,
    /// When `Some(prefix)`, only handle events whose type starts with `prefix`.
    /// When `None`, handle all event types.
    event_type_filter: Option<String>,
}

impl TelemetryCollectorProcessor {
    /// Create a processor that handles all event types.
    pub fn new(collector: TelemetryCollector) -> Self {
        Self {
            collector,
            event_type_filter: None,
        }
    }

    /// Create a processor that only handles events whose type starts with `prefix`.
    pub fn with_filter(collector: TelemetryCollector, prefix: &str) -> Self {
        Self {
            collector,
            event_type_filter: Some(prefix.to_string()),
        }
    }

    /// Flush all collected telemetry records, draining the internal buffer.
    pub fn flush(&mut self) -> Vec<TelemetryRecord> {
        self.collector.flush()
    }

    /// Get the current count of collected records without flushing.
    pub fn record_count(&self) -> usize {
        self.collector.record_count()
    }
}

impl EventProcessor for TelemetryCollectorProcessor {
    /// Returns `true` when no filter is set, or when `event_type` starts with
    /// the configured prefix filter.
    fn handles(&self, event_type: &str) -> bool {
        match &self.event_type_filter {
            None => true,
            Some(prefix) => event_type.starts_with(prefix.as_str()),
        }
    }

    /// Process a CloudEvents JSON event through the collector's generators.
    ///
    /// Uses the collector's interior `Arc<Mutex<>>` so this can take `&self`.
    fn process(&self, event: &Value) {
        // TelemetryCollector uses Arc<Mutex<>> internally, so cloning gives
        // another handle to the same underlying data. Calling process_event
        // on the clone mutates the shared records vec.
        let mut collector_clone = self.collector.clone();
        let _ = collector_clone.process_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processor_new() {
        let collector = TelemetryCollector::new();
        let _processor = TelemetryCollectorProcessor::new(collector);
        // Should not panic
    }

    #[test]
    fn test_processor_with_filter() {
        let collector = TelemetryCollector::new();
        let _processor = TelemetryCollectorProcessor::with_filter(collector, "io.kitchen");
        // Should not panic
    }

    #[test]
    fn test_handles_all_events_when_no_filter() {
        let collector = TelemetryCollector::new();
        let processor = TelemetryCollectorProcessor::new(collector);
        // When no filter, handles all event types
        assert!(processor.handles("io.kitchen.order.created"));
        assert!(processor.handles("io.kitchen.inventory.replenished"));
        assert!(processor.handles("anything.at.all"));
    }

    #[test]
    fn test_handles_with_filter() {
        let collector = TelemetryCollector::new();
        let processor = TelemetryCollectorProcessor::with_filter(collector, "io.kitchen");
        assert!(processor.handles("io.kitchen.order.created"));
        assert!(!processor.handles("io.other.event"));
    }

    #[test]
    fn test_flush_returns_empty_initially() {
        let collector = TelemetryCollector::new();
        let mut processor = TelemetryCollectorProcessor::new(collector);
        let records = processor.flush();
        assert!(records.is_empty());
    }
}
