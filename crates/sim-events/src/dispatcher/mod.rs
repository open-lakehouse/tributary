//! Event dispatcher for collecting and emitting events

use crate::core::{Event, EventRecord};
use crate::storage::ParquetEventWriter;
use std::collections::HashMap;
use std::path::PathBuf;

/// Collects and dispatches events
///
/// The dispatcher buffers events in memory and can write them to storage
/// using various writers (Parquet, JSON, etc.).
///
/// # Example
///
/// ```rust,ignore
/// use sim_events::{EventDispatcher, Event};
///
/// let mut dispatcher = EventDispatcher::new();
///
/// // Emit events
/// dispatcher.emit(event1)?;
/// dispatcher.emit(event2)?;
///
/// // Write to storage
/// dispatcher.flush_to_parquet("events.parquet")?;
/// ```
pub struct EventDispatcher {
    /// Buffered events
    buffer: Vec<Event>,
    /// Event counts by type
    counts: HashMap<String, usize>,
}

impl EventDispatcher {
    /// Create a new event dispatcher
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            counts: HashMap::new(),
        }
    }

    /// Emit a single event
    ///
    /// The event is added to the buffer. Call `flush()` to persist.
    pub fn emit(&mut self, event: Event) {
        *self
            .counts
            .entry(event.event_type().to_string())
            .or_insert(0) += 1;
        self.buffer.push(event);
    }

    /// Emit multiple events
    pub fn emit_batch(&mut self, events: Vec<Event>) {
        for event in events {
            self.emit(event);
        }
    }

    /// Get the number of buffered events
    pub fn buffered_count(&self) -> usize {
        self.buffer.len()
    }

    /// Get event counts by type
    pub fn event_counts(&self) -> &HashMap<String, usize> {
        &self.counts
    }

    /// Get count for a specific event type
    pub fn count_by_type(&self, event_type: &str) -> usize {
        self.counts.get(event_type).copied().unwrap_or(0)
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Get all buffered events
    pub fn buffered_events(&self) -> &[Event] {
        &self.buffer
    }

    /// Convert buffered events to records
    pub fn to_records(&self) -> Vec<EventRecord> {
        self.buffer.iter().map(|e| e.to_record()).collect()
    }

    /// Flush events to Parquet file
    ///
    /// # Arguments
    ///
    /// * `path` - Path to write Parquet file
    ///
    /// # Returns
    ///
    /// Number of events written
    pub fn flush_to_parquet(&mut self, path: impl Into<PathBuf>) -> crate::Result<usize> {
        let records = self.to_records();
        let count = records.len();

        if count > 0 {
            let mut writer = ParquetEventWriter::new(path.into())?;
            writer.write_events(&records)?;
            writer.close()?;
        }

        self.clear();
        Ok(count)
    }

    /// Flush events to JSON Lines file
    ///
    /// Each line is a CloudEvents JSON object.
    pub fn flush_to_jsonl(&mut self, path: impl Into<PathBuf>) -> crate::Result<usize> {
        let path = path.into();
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);

        let count = self.buffer.len();

        for event in &self.buffer {
            let json = event.to_cloudevents_json();
            serde_json::to_writer(&mut writer, &json)?;
            use std::io::Write;
            writeln!(writer)?;
        }

        self.clear();
        Ok(count)
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::EventPayload;
    use serde_json::json;

    #[derive(Debug)]
    struct TestPayload {
        value: i32,
    }

    impl EventPayload for TestPayload {
        fn event_type(&self) -> &str {
            "test.event"
        }

        fn source(&self) -> String {
            "/test".to_string()
        }

        fn to_json(&self) -> serde_json::Value {
            json!({ "value": self.value })
        }
    }

    #[test]
    fn test_dispatcher_emit() {
        let mut dispatcher = EventDispatcher::new();

        let event1 = Event::new(Box::new(TestPayload { value: 1 }));
        let event2 = Event::new(Box::new(TestPayload { value: 2 }));

        dispatcher.emit(event1);
        dispatcher.emit(event2);

        assert_eq!(dispatcher.buffered_count(), 2);
        assert_eq!(dispatcher.count_by_type("test.event"), 2);
    }

    #[test]
    fn test_dispatcher_batch() {
        let mut dispatcher = EventDispatcher::new();

        let events = vec![
            Event::new(Box::new(TestPayload { value: 1 })),
            Event::new(Box::new(TestPayload { value: 2 })),
            Event::new(Box::new(TestPayload { value: 3 })),
        ];

        dispatcher.emit_batch(events);

        assert_eq!(dispatcher.buffered_count(), 3);
    }

    #[test]
    fn test_dispatcher_clear() {
        let mut dispatcher = EventDispatcher::new();

        dispatcher.emit(Event::new(Box::new(TestPayload { value: 1 })));
        assert_eq!(dispatcher.buffered_count(), 1);

        dispatcher.clear();
        assert_eq!(dispatcher.buffered_count(), 0);
    }

    #[test]
    fn test_dispatcher_to_records() {
        let mut dispatcher = EventDispatcher::new();

        dispatcher.emit(Event::new(Box::new(TestPayload { value: 42 })));

        let records = dispatcher.to_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].event_type, "test.event");
        assert_eq!(records[0].data["value"], 42);
    }
}
