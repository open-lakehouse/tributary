//! Event dispatcher for collecting and emitting events

use crate::core::{Event, EventRecord};
use std::collections::HashMap;

/// Trait for processing events as they are emitted.
/// Processors must not block — use channels for expensive work.
pub trait EventProcessor: Send + Sync {
    /// Return true if this processor handles the given event type.
    fn handles(&self, event_type: &str) -> bool;
    /// Process the event synchronously. Must not block.
    fn process(&self, event: &Event);
}

/// A processor that forwards matching events through an unbounded mpsc channel.
pub struct ChannelSenderProcessor {
    sender: tokio::sync::mpsc::UnboundedSender<serde_json::Value>,
    filter: Option<String>, // None = send all
}

impl ChannelSenderProcessor {
    pub fn new(
        sender: tokio::sync::mpsc::UnboundedSender<serde_json::Value>,
        filter: Option<String>,
    ) -> Self {
        Self { sender, filter }
    }
}

impl EventProcessor for ChannelSenderProcessor {
    fn handles(&self, event_type: &str) -> bool {
        match &self.filter {
            None => true,
            Some(prefix) => event_type.starts_with(prefix.as_str()),
        }
    }

    fn process(&self, event: &Event) {
        let json = event.to_cloudevents_json();
        let _ = self.sender.send(json); // ignore send errors (receiver dropped)
    }
}

/// Collects and dispatches events
///
/// The dispatcher buffers events in memory and routes them to registered
/// `EventProcessor` implementations. To persist, write `to_records()` with a
/// `ParquetEventWriter`.
///
/// # Example
///
/// ```rust,ignore
/// use sim_events::{EventDispatcher, Event};
///
/// let mut dispatcher = EventDispatcher::new();
/// dispatcher.emit(Event::new(Box::new(payload), ctx.current_time()));
///
/// // Persist at the end of a run
/// let mut writer = ParquetEventWriter::new("events.parquet".into())?;
/// writer.write_events(&dispatcher.to_records())?;
/// writer.close()?;
/// ```
pub struct EventDispatcher {
    /// Buffered events
    buffer: Vec<Event>,
    /// Event counts by type
    counts: HashMap<String, usize>,
    /// Registered event processors
    processors: Vec<Box<dyn EventProcessor>>,
    /// Index into `buffer` marking the start of the current tick
    tick_start_index: usize,
}

impl EventDispatcher {
    /// Create a new event dispatcher
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            counts: HashMap::new(),
            processors: Vec::new(),
            tick_start_index: 0,
        }
    }

    /// Register an event processor
    pub fn register_processor(&mut self, processor: Box<dyn EventProcessor>) {
        self.processors.push(processor);
    }

    /// Mark the start of the current simulation tick.
    ///
    /// After this call, `current_tick_events()` returns only events emitted
    /// since the most recent call to `mark_tick_start`.
    pub fn mark_tick_start(&mut self) {
        self.tick_start_index = self.buffer.len();
    }

    /// Return a slice of events emitted since the last `mark_tick_start`.
    pub fn current_tick_events(&self) -> &[Event] {
        &self.buffer[self.tick_start_index..]
    }

    /// Emit a single event
    ///
    /// The event is added to the buffer and routed to matching processors.
    pub fn emit(&mut self, event: Event) {
        *self
            .counts
            .entry(event.event_type().to_string())
            .or_insert(0) += 1;
        // Route to matching processors before buffering
        for processor in &self.processors {
            if processor.handles(event.event_type()) {
                processor.process(&event);
            }
        }
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

    /// Clear the buffer and reset counts
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.counts.clear();
        self.tick_start_index = 0;
    }

    /// Get all buffered events
    pub fn buffered_events(&self) -> &[Event] {
        &self.buffer
    }

    /// Convert buffered events to records
    pub fn to_records(&self) -> Vec<EventRecord> {
        self.buffer.iter().map(|e| e.to_record()).collect()
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
    use chrono::DateTime;
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

        let event1 = Event::new(Box::new(TestPayload { value: 1 }), DateTime::UNIX_EPOCH);
        let event2 = Event::new(Box::new(TestPayload { value: 2 }), DateTime::UNIX_EPOCH);

        dispatcher.emit(event1);
        dispatcher.emit(event2);

        assert_eq!(dispatcher.buffered_count(), 2);
        assert_eq!(dispatcher.count_by_type("test.event"), 2);
    }

    #[test]
    fn test_dispatcher_batch() {
        let mut dispatcher = EventDispatcher::new();

        let events = vec![
            Event::new(Box::new(TestPayload { value: 1 }), DateTime::UNIX_EPOCH),
            Event::new(Box::new(TestPayload { value: 2 }), DateTime::UNIX_EPOCH),
            Event::new(Box::new(TestPayload { value: 3 }), DateTime::UNIX_EPOCH),
        ];

        dispatcher.emit_batch(events);

        assert_eq!(dispatcher.buffered_count(), 3);
    }

    #[test]
    fn test_dispatcher_clear() {
        let mut dispatcher = EventDispatcher::new();

        dispatcher.emit(Event::new(
            Box::new(TestPayload { value: 1 }),
            DateTime::UNIX_EPOCH,
        ));
        assert_eq!(dispatcher.buffered_count(), 1);

        dispatcher.clear();
        assert_eq!(dispatcher.buffered_count(), 0);
    }

    #[test]
    fn test_count_tracking_after_clear() {
        let mut dispatcher = EventDispatcher::new();

        // Emit events
        dispatcher.emit(Event::new(
            Box::new(TestPayload { value: 1 }),
            DateTime::UNIX_EPOCH,
        ));
        dispatcher.emit(Event::new(
            Box::new(TestPayload { value: 2 }),
            DateTime::UNIX_EPOCH,
        ));
        dispatcher.emit(Event::new(
            Box::new(TestPayload { value: 3 }),
            DateTime::UNIX_EPOCH,
        ));

        assert_eq!(dispatcher.buffered_count(), 3);
        assert_eq!(dispatcher.count_by_type("test.event"), 3);

        // Clear should reset both buffer and counts
        dispatcher.clear();
        assert_eq!(dispatcher.buffered_count(), 0);
        assert_eq!(dispatcher.count_by_type("test.event"), 0);

        // Verify counts HashMap is empty
        assert_eq!(dispatcher.event_counts().len(), 0);

        // Emit new events after clear
        dispatcher.emit(Event::new(
            Box::new(TestPayload { value: 4 }),
            DateTime::UNIX_EPOCH,
        ));
        dispatcher.emit(Event::new(
            Box::new(TestPayload { value: 5 }),
            DateTime::UNIX_EPOCH,
        ));

        // Counts should start fresh from 0
        assert_eq!(dispatcher.buffered_count(), 2);
        assert_eq!(dispatcher.count_by_type("test.event"), 2);
    }

    #[test]
    fn test_dispatcher_to_records() {
        let mut dispatcher = EventDispatcher::new();

        dispatcher.emit(Event::new(
            Box::new(TestPayload { value: 42 }),
            DateTime::UNIX_EPOCH,
        ));

        let records = dispatcher.to_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].event_type, "test.event");
        assert_eq!(records[0].data["value"], 42);
    }

    #[test]
    fn test_count_accuracy_long_running() {
        let mut dispatcher = EventDispatcher::new();

        // Simulate long-running simulation with multiple batches
        for _batch in 0..100 {
            // Emit events
            for i in 0..50 {
                dispatcher.emit(Event::new(
                    Box::new(TestPayload { value: i }),
                    DateTime::UNIX_EPOCH,
                ));
            }

            // Verify count before flush
            assert_eq!(dispatcher.buffered_count(), 50);
            assert_eq!(dispatcher.count_by_type("test.event"), 50);

            // Clear after processing (simulating flush)
            dispatcher.clear();

            // Verify everything is reset
            assert_eq!(dispatcher.buffered_count(), 0);
            assert_eq!(dispatcher.count_by_type("test.event"), 0);
            assert_eq!(dispatcher.event_counts().len(), 0);
        }

        // Final verification: no memory leak
        assert_eq!(dispatcher.buffered_count(), 0);
        assert_eq!(dispatcher.event_counts().len(), 0);
    }

    #[test]
    fn test_count_consistency_multiple_clears() {
        let mut dispatcher = EventDispatcher::new();

        // Test multiple clear operations
        for iteration in 0..10 {
            // Emit varying numbers of events
            let event_count = (iteration + 1) * 5;
            for i in 0..event_count {
                dispatcher.emit(Event::new(
                    Box::new(TestPayload { value: i as i32 }),
                    DateTime::UNIX_EPOCH,
                ));
            }

            // Verify counts are accurate
            assert_eq!(dispatcher.buffered_count(), event_count);
            assert_eq!(dispatcher.count_by_type("test.event"), event_count);

            // Clear
            dispatcher.clear();

            // Verify complete reset
            assert_eq!(dispatcher.buffered_count(), 0);
            assert_eq!(dispatcher.count_by_type("test.event"), 0);
            assert_eq!(dispatcher.event_counts().len(), 0);
        }
    }

    #[derive(Debug)]
    struct MultiTypePayload {
        event_type: String,
        value: i32,
    }

    impl EventPayload for MultiTypePayload {
        fn event_type(&self) -> &str {
            &self.event_type
        }

        fn source(&self) -> String {
            "/test".to_string()
        }

        fn to_json(&self) -> serde_json::Value {
            json!({ "value": self.value })
        }
    }

    #[test]
    fn test_count_tracking_multiple_types() {
        let mut dispatcher = EventDispatcher::new();

        // Emit events of different types
        dispatcher.emit(Event::new(
            Box::new(MultiTypePayload {
                event_type: "type.a".to_string(),
                value: 1,
            }),
            DateTime::UNIX_EPOCH,
        ));
        dispatcher.emit(Event::new(
            Box::new(MultiTypePayload {
                event_type: "type.a".to_string(),
                value: 2,
            }),
            DateTime::UNIX_EPOCH,
        ));
        dispatcher.emit(Event::new(
            Box::new(MultiTypePayload {
                event_type: "type.b".to_string(),
                value: 3,
            }),
            DateTime::UNIX_EPOCH,
        ));
        dispatcher.emit(Event::new(
            Box::new(MultiTypePayload {
                event_type: "type.c".to_string(),
                value: 4,
            }),
            DateTime::UNIX_EPOCH,
        ));

        // Verify counts by type
        assert_eq!(dispatcher.buffered_count(), 4);
        assert_eq!(dispatcher.count_by_type("type.a"), 2);
        assert_eq!(dispatcher.count_by_type("type.b"), 1);
        assert_eq!(dispatcher.count_by_type("type.c"), 1);
        assert_eq!(dispatcher.event_counts().len(), 3);

        // Clear should reset all types
        dispatcher.clear();

        assert_eq!(dispatcher.buffered_count(), 0);
        assert_eq!(dispatcher.count_by_type("type.a"), 0);
        assert_eq!(dispatcher.count_by_type("type.b"), 0);
        assert_eq!(dispatcher.count_by_type("type.c"), 0);
        assert_eq!(dispatcher.event_counts().len(), 0);
    }

    #[test]
    fn test_processor_registration() {
        use std::sync::{Arc, Mutex};

        struct CountingProcessor {
            count: Arc<Mutex<usize>>,
        }
        impl EventProcessor for CountingProcessor {
            fn handles(&self, _: &str) -> bool {
                true
            }
            fn process(&self, _: &Event) {
                *self.count.lock().unwrap() += 1;
            }
        }

        let count = Arc::new(Mutex::new(0usize));
        let processor = CountingProcessor {
            count: count.clone(),
        };

        let mut dispatcher = EventDispatcher::new();
        dispatcher.register_processor(Box::new(processor));

        dispatcher.emit(Event::new(
            Box::new(TestPayload { value: 1 }),
            DateTime::UNIX_EPOCH,
        ));
        dispatcher.emit(Event::new(
            Box::new(TestPayload { value: 2 }),
            DateTime::UNIX_EPOCH,
        ));

        assert_eq!(*count.lock().unwrap(), 2);
    }

    #[test]
    fn test_processor_handles_filter() {
        use std::sync::{Arc, Mutex};

        struct FilteredProcessor {
            prefix: String,
            count: Arc<Mutex<usize>>,
        }
        impl EventProcessor for FilteredProcessor {
            fn handles(&self, event_type: &str) -> bool {
                event_type.starts_with(&self.prefix)
            }
            fn process(&self, _: &Event) {
                *self.count.lock().unwrap() += 1;
            }
        }

        let count = Arc::new(Mutex::new(0usize));
        let processor = FilteredProcessor {
            prefix: "kitchen.".to_string(),
            count: count.clone(),
        };

        let mut dispatcher = EventDispatcher::new();
        dispatcher.register_processor(Box::new(processor));

        // This should NOT trigger (wrong prefix) — TestPayload emits "test.event"
        dispatcher.emit(Event::new(
            Box::new(TestPayload { value: 1 }),
            DateTime::UNIX_EPOCH,
        ));
        assert_eq!(*count.lock().unwrap(), 0);
    }

    #[test]
    fn test_mark_tick_start() {
        let mut dispatcher = EventDispatcher::new();

        dispatcher.emit(Event::new(
            Box::new(TestPayload { value: 1 }),
            DateTime::UNIX_EPOCH,
        ));
        dispatcher.emit(Event::new(
            Box::new(TestPayload { value: 2 }),
            DateTime::UNIX_EPOCH,
        ));
        dispatcher.emit(Event::new(
            Box::new(TestPayload { value: 3 }),
            DateTime::UNIX_EPOCH,
        ));

        dispatcher.mark_tick_start();

        dispatcher.emit(Event::new(
            Box::new(TestPayload { value: 4 }),
            DateTime::UNIX_EPOCH,
        ));
        dispatcher.emit(Event::new(
            Box::new(TestPayload { value: 5 }),
            DateTime::UNIX_EPOCH,
        ));

        let tick_events = dispatcher.current_tick_events();
        assert_eq!(tick_events.len(), 2);
    }
}
