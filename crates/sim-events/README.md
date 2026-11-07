# sim-events

CloudEvents 1.0 event records for simulations: an in-memory dispatcher with per-event
processors, a CloudEvents JSON view, and Parquet persistence.

Domain events implement `EventPayload`. `event_type`, `source` and `to_json` are required;
`subject` defaults to `None` and `data_content_type` to `application/json`.

```rust
use chrono::{TimeZone, Utc};
use sim_events::{Event, EventDispatcher, EventPayload};
use serde_json::{Value, json};

#[derive(Debug)]
struct OrderCreated {
    order_id: String,
}

impl EventPayload for OrderCreated {
    fn event_type(&self) -> &str {
        "io.example.order.created"
    }
    fn source(&self) -> String {
        format!("/orders/{}", self.order_id)
    }
    fn to_json(&self) -> Value {
        json!({ "order_id": self.order_id })
    }
}

let mut dispatcher = EventDispatcher::new();
let time = Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap();
dispatcher.emit(Event::new(Box::new(OrderCreated { order_id: "o-1".into() }), time));
assert_eq!(dispatcher.count_by_type("io.example.order.created"), 1);
```

`Event::new(payload, time)` takes the simulation time at which the event occurred. The time
is required. Each event gets a UUID v7 `id` and `specversion` `"1.0"`.
`Event::to_cloudevents_json()` returns the CloudEvents object.

## Dispatcher

`EventDispatcher::emit` routes the event to each registered processor whose `handles`
returns true, then buffers it. Other methods: `emit_batch`, `mark_tick_start` and
`current_tick_events` (events since the last mark), `buffered_events`, `buffered_count`,
`event_counts`, `count_by_type`, `clear`, and `to_records` (for persistence).

`EventProcessor` has `handles(&self, event_type: &str) -> bool` and
`process(&self, event: &Event)`. `process` must not block.
`ChannelSenderProcessor::new(sender, filter)` sends each matching event's CloudEvents JSON
through a `tokio` unbounded channel. `filter` is an event type prefix; `None` sends all.

## Parquet

`ParquetEventWriter` writes one row per event: `id`, `source`, `spec_version`,
`event_type`, `time` (RFC 3339), nullable `subject` and `data_content_type`, and `data`
(JSON text). It uses Snappy compression.

```rust
use sim_events::{ParquetEventReader, ParquetEventWriter};
use std::path::PathBuf;

let mut writer = ParquetEventWriter::new(PathBuf::from("events.parquet"))?;
writer.write_events(&dispatcher.to_records())?; // may be called repeatedly
writer.close()?;
let records = ParquetEventReader::read_file("events.parquet")?; // Vec<EventRecord>
```

`new` replaces an existing file. `close` finishes it. There is no JSON Lines writer; use
`to_cloudevents_json()` or `ChannelSenderProcessor` for JSON output.

Run `cargo doc -p sim-events --open` for the API reference.
