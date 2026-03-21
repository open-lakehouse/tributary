# sim-events Architecture

## Design Principles

### 1. CloudEvents 1.0 Compliance
All events follow the CloudEvents specification ensuring interoperability with standard tooling.

### 2. Type-Safe Event Payloads
The `EventPayload` trait provides compile-time safety while allowing domain-specific data.

### 3. Efficient Columnar Storage
Parquet format enables efficient storage and fast analytical queries via DataFusion.

### 4. Minimal Runtime Overhead
Events are buffered in memory and flushed in batches to minimize I/O impact.

## Module Organization

```
sim-events/
├── core/              Core event types
│   ├── event.rs       Event wrapper
│   ├── payload.rs     EventPayload trait
│   └── record.rs      EventRecord serialization
│
├── dispatcher/        Event emission and routing
│   └── mod.rs         EventDispatcher + EventProcessor trait
│
└── storage/           Persistence
    └── mod.rs         ParquetEventWriter
```

## CloudEvents Schema

```rust
pub struct Event {
    pub id: String,                        // UUID v7
    pub source: String,                    // URI (e.g., "/agents/123")
    pub specversion: String,               // "1.0"
    pub r#type: String,                    // Event type
    pub time: DateTime<Utc>,               // Timestamp
    pub subject: Option<String>,           // Optional context
    pub datacontenttype: Option<String>,   // MIME type
    pub data: serde_json::Value,           // Payload
}
```

## EventPayload Trait

```rust
pub trait EventPayload: Debug + Send + Sync {
    fn event_type(&self) -> &str;
    fn source(&self) -> String;
    fn subject(&self) -> Option<String> { None }
    fn datacontenttype(&self) -> Option<String> { None }
    fn to_json(&self) -> serde_json::Value;
}
```

**Design Decision**: Trait-based vs struct-based events.

**Trade-off**: Flexibility (any type can be an event) vs simplicity (fixed struct).

## Event Flow

```
Domain Event -> EventPayload impl -> Event wrapper -> EventDispatcher
    -> Route to registered EventProcessors (non-blocking)
    -> Buffer (in-memory)
    -> [Optional] fire on mpsc channel for async consumers
```

## EventProcessor Pipeline

`EventDispatcher` supports pluggable processors registered at startup:

```rust
pub trait EventProcessor: Send + Sync {
    fn handles(&self, event_type: &str) -> bool;
    fn process(&self, event: &Event);
}
```

Built-in processors:
- **`ChannelSenderProcessor`** — forwards matching events as JSON on an `mpsc` channel
- **`ParquetEventProcessor`** — buffers and flushes raw events to Parquet

Downstream processors (in their own crates):
- **`ArtifactCollectorProcessor<P>`** in `sim-artifacts` — routes events to artifact generators
- **`TelemetryCollectorProcessor`** in `sim-telemetry` — routes events to telemetry generators

### Tick Index

`EventDispatcher` tracks a `tick_start_index` so hooks can efficiently slice only the current tick's events:

```rust
dispatcher.mark_tick_start();           // call in on_before_tick
dispatcher.current_tick_events()        // returns &[Event] since last mark
```

## Parquet Schema

Events are stored in columnar format:

```
Schema:
  - id: String (UUID)
  - source: String
  - specversion: String
  - type: String
  - time: Timestamp(Nanosecond, UTC)
  - subject: String (optional)
  - datacontenttype: String (optional)
  - data: String (JSON serialized)
```

**Benefits**:
- Columnar compression
- Fast type-based filtering
- Efficient time-range queries
- DataFusion integration

## Performance Characteristics

- Event creation: <1μs
- Buffer insertion: O(1)
- Parquet flush: ~1ms per 1000 events
- Memory: ~500 bytes per buffered event

## Design Decisions

| Decision | Rationale | Trade-off |
|----------|-----------|-----------|
| CloudEvents 1.0 | Industry standard | Slight overhead vs custom format |
| Parquet storage | Analytical queries | Write amplification |
| In-memory buffering | Minimize I/O | Memory usage |
| UUID v7 | Time-ordered, unique | Slightly larger than sequential IDs |
| `EventProcessor` uses `&self` | Allows multiple borrows; processors use interior mutability | Requires `Mutex`/`Arc` for mutable state |
| Persistence via processors | Decouples I/O from dispatch loop; composable | Callers must register processors explicitly |
