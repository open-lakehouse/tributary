//! Event-processor adapter for [`TelemetryCollector`].
//!
//! sim-telemetry does not depend on sim-events, so it defines its own minimal
//! [`EventProcessor`] trait over CloudEvents JSON. It cannot be registered with
//! `sim_events::EventDispatcher` directly; feed it events yourself. Replacing it
//! with `sim_events::EventProcessor` is tracked in
//! <https://github.com/open-lakehouse/tributary/issues/32>.

use crate::core::{TelemetryCollector, TelemetryRecord};
use serde_json::Value;

/// Minimal event-processor interface over CloudEvents JSON.
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
/// Pass each event to [`EventProcessor::process`] to collect telemetry
/// without modifying the simulation hooks.
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
