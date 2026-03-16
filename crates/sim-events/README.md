# sim-events

CloudEvents-compliant event infrastructure for agent-based simulations.

## Overview

`sim-events` provides a generic, standards-based event system for recording simulation state changes. All events follow the [CloudEvents 1.0 specification](https://cloudevents.io/), ensuring interoperability and clear semantics.

## Features

- **CloudEvents 1.0 Compliance** - Standardized event format
- **Type-Safe Event IDs** - UUID v7 identifiers for time-ordered events
- **Flexible Payloads** - Trait-based system for domain-specific data
- **Multiple Formats** - Export to Parquet or JSON Lines
- **Arrow Integration** - Efficient columnar storage
- **In-Memory Buffering** - Collect events before flushing to disk

## Quick Start

### 1. Define Event Payloads

Implement the `EventPayload` trait for your domain-specific events:

```rust
use sim_events::EventPayload;
use serde_json::json;

#[derive(Debug)]
struct AgentSpawned {
    agent_id: String,
    agent_type: String,
    location: String,
}

impl EventPayload for AgentSpawned {
    fn event_type(&self) -> &str {
        "io.simulation.agent.spawned"
    }

    fn source(&self) -> String {
        format!("/agents/{}", self.agent_id)
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "agent_id": self.agent_id,
            "agent_type": self.agent_type,
            "location": self.location,
        })
    }
}
```

### 2. Emit Events

Use the `EventDispatcher` to collect and emit events:

```rust
use sim_events::{Event, EventDispatcher};

let mut dispatcher = EventDispatcher::new();

// Create and emit events
let event = Event::new(Box::new(AgentSpawned {
    agent_id: "agent-001".to_string(),
    agent_type: "customer".to_string(),
    location: "store-123".to_string(),
}));

dispatcher.emit(event);
```

### 3. Persist to Storage

Write events to Parquet or JSON Lines:

```rust
// Write to Parquet (efficient columnar format)
let count = dispatcher.flush_to_parquet("events.parquet")?;
println!("Wrote {} events", count);

// Or write to JSON Lines (text format)
let count = dispatcher.flush_to_jsonl("events.jsonl")?;
```

## CloudEvents Schema

All events include these CloudEvents fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | String | Yes | Unique event identifier (UUID v7) |
| `source` | String | Yes | URI identifying event source |
| `specversion` | String | Yes | CloudEvents version ("1.0") |
| `type` | String | Yes | Event type identifier |
| `time` | String | Yes | ISO 8601 timestamp |
| `subject` | String | No | Additional context |
| `datacontenttype` | String | No | Content type of data |
| `data` | JSON | Yes | Event payload |

## Event Types

Define event types using reverse-DNS notation:

- `io.simulation.agent.spawned`
- `io.simulation.agent.departed`
- `io.simulation.location.arrived`
- `com.example.order.created`
- `com.warehouse.shipment.dispatched`

## Storage Formats

### Parquet

Parquet provides efficient columnar storage with compression:

```rust
dispatcher.flush_to_parquet("events.parquet")?;
```

**Benefits**:
- Columnar format for analytics
- Snappy compression
- Arrow-compatible
- Query with DataFusion, Polars, DuckDB

### JSON Lines

JSON Lines writes one JSON object per line:

```rust
dispatcher.flush_to_jsonl("events.jsonl")?;
```

**Benefits**:
- Human-readable
- Line-based processing
- Compatible with jq, grep
- Standard CloudEvents format

## Event Dispatcher API

```rust
impl EventDispatcher {
    /// Create new dispatcher
    pub fn new() -> Self;

    /// Emit single event
    pub fn emit(&mut self, event: Event);

    /// Emit multiple events
    pub fn emit_batch(&mut self, events: Vec<Event>);

    /// Get buffer size
    pub fn buffered_count(&self) -> usize;

    /// Get event counts by type
    pub fn event_counts(&self) -> &HashMap<String, usize>;

    /// Get count for specific type
    pub fn count_by_type(&self, event_type: &str) -> usize;

    /// Clear buffer
    pub fn clear(&mut self);

    /// Write to Parquet
    pub fn flush_to_parquet(&mut self, path: impl Into<PathBuf>)
        -> Result<usize>;

    /// Write to JSON Lines
    pub fn flush_to_jsonl(&mut self, path: impl Into<PathBuf>)
        -> Result<usize>;
}
```

## Advanced Usage

### Custom Timestamps

Override event timestamp:

```rust
use chrono::Utc;

let custom_time = Utc::now();
let event = Event::new(Box::new(payload))
    .with_time(custom_time);
```

### Event Tracking

Track event counts during simulation:

```rust
dispatcher.emit(event);

// Check counts
println!("Total events: {}", dispatcher.buffered_count());
println!("Spawns: {}", dispatcher.count_by_type("agent.spawned"));

// Get all counts
for (event_type, count) in dispatcher.event_counts() {
    println!("{}: {}", event_type, count);
}
```

### Reading Events

Query Parquet files with DataFusion:

```rust
use datafusion::prelude::*;

let ctx = SessionContext::new();
let df = ctx.read_parquet("events.parquet", ParquetReadOptions::default()).await?;

// Filter by event type
let spawns = df.filter(col("event_type").eq(lit("agent.spawned")))?;
let results = spawns.collect().await?;
```

## Integration with sim-core

Events use type-safe identifiers from `sim-core`:

```rust
use sim_core::utils::idents::{AgentId, LocationId};
use sim_events::EventPayload;

#[derive(Debug)]
struct LocationArrived {
    agent_id: AgentId,
    location_id: LocationId,
}

impl EventPayload for LocationArrived {
    fn event_type(&self) -> &str {
        "io.simulation.location.arrived"
    }

    fn source(&self) -> String {
        format!("/agents/{}", self.agent_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.location_id.to_string())
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "agent_id": self.agent_id.to_string(),
            "location_id": self.location_id.to_string(),
        })
    }
}
```

## Examples

See the tests in each module for complete examples:

- `src/core/payload.rs` - Custom payload implementations
- `src/core/event.rs` - Event creation and serialization
- `src/dispatcher/mod.rs` - Event emission and buffering
- `src/storage/mod.rs` - Parquet writing

## Dependencies

- `sim-core` - Core simulation types
- `arrow` - Columnar data format
- `parquet` - Parquet file format
- `chrono` - Timestamp handling
- `serde_json` - JSON serialization

## License

Apache-2.0

## Related Crates

- `sim-core` - Core simulation abstractions
- `sim-geo` - Geospatial components
- `sim-telemetry` - Metrics and tracing (planned)
