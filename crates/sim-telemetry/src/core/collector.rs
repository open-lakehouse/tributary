//! Telemetry collection and aggregation.
//!
//! The `TelemetryCollector` receives events, runs them through multiple generators,
//! and aggregates the resulting telemetry records for export.

use super::{TelemetryError, TelemetryGenerator, TelemetryRecord};
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// Collects and aggregates telemetry records from multiple generators.
///
/// The collector maintains a set of telemetry generators and processes events
/// through them to produce logs, traces, and metrics. Records are stored in
/// memory until flushed.
///
/// # Thread Safety
///
/// The collector uses interior mutability with `Arc<Mutex<>>` to allow safe
/// concurrent access from multiple threads.
///
/// # Example
///
/// ```rust
/// use sim_telemetry::core::TelemetryCollector;
/// use sim_telemetry::generators::{EventToLogGenerator, EventToMetricGenerator};
/// use serde_json::json;
///
/// let mut collector = TelemetryCollector::new();
/// collector.add_generator(Box::new(EventToLogGenerator::new()));
/// collector.add_generator(Box::new(EventToMetricGenerator::new()));
///
/// let event = json!({
///     "id": "evt-001",
///     "type": "com.simulation.agent_ticked",
///     "source": "urn:sim:agent:agent-001",
///     "time": "2026-03-16T10:00:00Z",
///     "data": {"agent_id": "agent-001", "tick": 1}
/// });
///
/// collector.process_event(&event).unwrap();
/// let records = collector.flush();
/// println!("Collected {} telemetry records", records.len());
/// ```
#[derive(Clone)]
pub struct TelemetryCollector {
    generators: Arc<Mutex<Vec<Box<dyn TelemetryGenerator>>>>,
    records: Arc<Mutex<Vec<TelemetryRecord>>>,
}

impl TelemetryCollector {
    /// Create a new empty telemetry collector.
    pub fn new() -> Self {
        Self {
            generators: Arc::new(Mutex::new(Vec::new())),
            records: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add a telemetry generator to the collector.
    ///
    /// Generators are applied in the order they are added.
    ///
    /// # Arguments
    ///
    /// * `generator` - A boxed telemetry generator
    pub fn add_generator(&mut self, generator: Box<dyn TelemetryGenerator>) {
        self.generators.lock().unwrap().push(generator);
    }

    /// Process an event through all registered generators.
    ///
    /// Each generator may produce zero or more telemetry records from the event.
    /// All generated records are collected internally until `flush()` is called.
    ///
    /// # Arguments
    ///
    /// * `event` - A JSON event to process
    ///
    /// # Returns
    ///
    /// Number of telemetry records generated from this event
    ///
    /// # Errors
    ///
    /// Returns error if any generator fails to process the event
    pub fn process_event(&mut self, event: &Value) -> Result<usize, TelemetryError> {
        let mut count = 0;
        let mut generators = self.generators.lock().unwrap();

        for generator in generators.iter_mut() {
            let new_records = generator.generate(event)?;
            count += new_records.len();
            self.records.lock().unwrap().extend(new_records);
        }

        Ok(count)
    }

    /// Flush all collected telemetry records.
    ///
    /// This drains the internal buffer and returns all collected records,
    /// resetting the collector to empty state.
    ///
    /// # Returns
    ///
    /// Vector of all telemetry records collected since the last flush
    pub fn flush(&mut self) -> Vec<TelemetryRecord> {
        let mut records = self.records.lock().unwrap();
        std::mem::take(&mut *records)
    }

    /// Get the current count of collected records without flushing.
    pub fn record_count(&self) -> usize {
        self.records.lock().unwrap().len()
    }

    /// Get the number of registered generators.
    pub fn generator_count(&self) -> usize {
        self.generators.lock().unwrap().len()
    }

    /// Clear all collected records without returning them.
    pub fn clear(&mut self) {
        self.records.lock().unwrap().clear();
    }
}

impl Default for TelemetryCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators::{EventToLogGenerator, EventToMetricGenerator, EventToTraceGenerator};
    use serde_json::json;

    #[test]
    fn test_collector_creation() {
        let collector = TelemetryCollector::new();
        assert_eq!(collector.record_count(), 0);
        assert_eq!(collector.generator_count(), 0);
    }

    #[test]
    fn test_add_generators() {
        let mut collector = TelemetryCollector::new();
        collector.add_generator(Box::new(EventToLogGenerator::new()));
        collector.add_generator(Box::new(EventToMetricGenerator::new()));

        assert_eq!(collector.generator_count(), 2);
    }

    #[test]
    fn test_process_event() {
        let mut collector = TelemetryCollector::new();
        collector.add_generator(Box::new(EventToLogGenerator::new()));

        let event = json!({
            "id": "test-001",
            "type": "com.simulation.test",
            "source": "urn:test",
            "time": "2026-03-16T10:00:00Z",
            "data": {}
        });

        let count = collector.process_event(&event).unwrap();
        assert_eq!(count, 1);
        assert_eq!(collector.record_count(), 1);
    }

    #[test]
    fn test_flush() {
        let mut collector = TelemetryCollector::new();
        collector.add_generator(Box::new(EventToLogGenerator::new()));

        let event = json!({
            "id": "test-001",
            "type": "com.simulation.test",
            "source": "urn:test",
            "time": "2026-03-16T10:00:00Z",
            "data": {}
        });

        collector.process_event(&event).unwrap();
        assert_eq!(collector.record_count(), 1);

        let records = collector.flush();
        assert_eq!(records.len(), 1);
        assert_eq!(collector.record_count(), 0);
    }

    #[test]
    fn test_multiple_generators() {
        let mut collector = TelemetryCollector::new();
        collector.add_generator(Box::new(EventToLogGenerator::new()));
        collector.add_generator(Box::new(EventToMetricGenerator::new()));
        collector.add_generator(Box::new(EventToTraceGenerator::new()));

        let event = json!({
            "id": "test-001",
            "type": "com.simulation.test",
            "source": "urn:test",
            "time": "2026-03-16T10:00:00Z",
            "data": {"value": 42.0}
        });

        let count = collector.process_event(&event).unwrap();
        assert!(count >= 3); // Should generate at least one record per generator
    }

    #[test]
    fn test_clear() {
        let mut collector = TelemetryCollector::new();
        collector.add_generator(Box::new(EventToLogGenerator::new()));

        let event = json!({
            "id": "test-001",
            "type": "com.simulation.test",
            "source": "urn:test",
            "time": "2026-03-16T10:00:00Z",
            "data": {}
        });

        collector.process_event(&event).unwrap();
        assert_eq!(collector.record_count(), 1);

        collector.clear();
        assert_eq!(collector.record_count(), 0);
    }

    #[test]
    fn test_batch_processing() {
        let mut collector = TelemetryCollector::new();
        collector.add_generator(Box::new(EventToLogGenerator::new()));

        for i in 0..10 {
            let event = json!({
                "id": format!("test-{:03}", i),
                "type": "com.simulation.test",
                "source": "urn:test",
                "time": "2026-03-16T10:00:00Z",
                "data": {"index": i}
            });

            collector.process_event(&event).unwrap();
        }

        assert_eq!(collector.record_count(), 10);

        let records = collector.flush();
        assert_eq!(records.len(), 10);
        assert_eq!(collector.record_count(), 0);
    }
}
