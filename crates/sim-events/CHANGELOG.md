# Changelog

All notable changes to `sim-events` will be documented in this file.

## [Unreleased] - 2026-03-21

### Added
- **`EventProcessor` trait**: Pluggable, non-blocking event processors (`handles(&str) -> bool`, `process(&Event)`)
- **`EventDispatcher::register_processor`**: Register processors at startup; `emit()` routes to all matching processors
- **`ChannelSenderProcessor`**: Forwards matching events as CloudEvents JSON on a `tokio::mpsc::UnboundedSender`
- **`ParquetEventProcessor`**: Buffers events with interior mutability and writes to Parquet on `flush()`
- **Tick index**: `mark_tick_start()` / `current_tick_events()` for efficient per-tick event slicing in hooks

### Deprecated
- `EventDispatcher::flush_to_parquet()` — use `ParquetEventProcessor` instead
- `EventDispatcher::flush_to_jsonl()` — use a custom `EventProcessor` instead

---

## [0.0.1] - 2026-03-16

### Added

#### Core Event Infrastructure
- **Event**: CloudEvents 1.0 compliant event wrapper
- **EventPayload trait**: Domain-specific event data abstraction
- **EventRecord**: Serializable event representation
- UUID v7 event IDs (time-ordered)
- ISO 8601 timestamp formatting
- CloudEvents required fields (id, source, specversion, type, time)
- Optional fields support (subject, datacontenttype)

#### Event Dispatcher
- **EventDispatcher**: In-memory event buffering and emission
- Batch event emission
- Event counting by type
- Buffer size tracking
- Event type statistics

#### Storage Formats
- **ParquetEventWriter**: Write events to Parquet columnar format
- JSON Lines export
- Arrow RecordBatch conversion
- Snappy compression
- Schema management

#### CloudEvents 1.0 Compliance
- Standardized event format
- Required field validation
- MIME type support
- Extension attributes

### Dependencies
- `sim-core 0.0.1` - Core types
- `arrow 57.0` - Columnar format
- `parquet 57.0` - Parquet file format
- `chrono 0.4` - Timestamp handling
- `uuid 1.0` - Event identifiers (v7)
- `serde_json 1.0` - JSON serialization

[0.0.1]: https://github.com/yourusername/repo/releases/tag/v0.0.1
