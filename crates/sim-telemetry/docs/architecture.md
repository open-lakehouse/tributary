# sim-telemetry Architecture

## Design Principles

### 1. OTLP Compliance
Full OpenTelemetry Protocol support for seamless integration with observability platforms.

### 2. Minimal Overhead
Designed for <5% performance impact on simulation runtime through efficient generation.

### 3. Multi-Signal Support
Unified handling of logs, traces, and metrics from a single event stream.

### 4. Extensible Generation
Custom generators via `TelemetryGenerator` trait for domain-specific telemetry.

## Module Organization

```
sim-telemetry/
├── core/                  Core types
│   ├── record.rs          TelemetryRecord enum
│   ├── generator.rs       TelemetryGenerator trait
│   └── collector.rs       TelemetryCollector
│
├── generators/            Builtin generators
│   ├── log.rs            EventToLogGenerator
│   ├── metric.rs         EventToMetricGenerator
│   └── trace.rs          EventToTraceGenerator
│
└── formats/               Export formats
    ├── otlp.rs           OTLP exporter
    ├── json.rs           JSON exporter
    └── prometheus.rs     Prometheus exporter
```

## Telemetry Record Types

### Log Record
```rust
pub enum TelemetryRecord {
    Log {
        timestamp: DateTime<Utc>,
        level: LogLevel,
        message: String,
        attributes: HashMap<String, String>,
    },
    // ...
}
```

### Metric Record
```rust
Metric {
    timestamp: DateTime<Utc>,
    name: String,
    metric_type: MetricType,  // Counter, Gauge, Histogram
    value: f64,
    labels: HashMap<String, String>,
}
```

### Trace Record
```rust
Trace {
    trace_id: String,
    span_id: String,
    parent_span_id: Option<String>,
    name: String,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    attributes: HashMap<String, String>,
}
```

## Generation Pipeline

```
CloudEvent -> TelemetryCollector -> [Generators] -> TelemetryRecords -> Exporter -> OTLP/JSON/Prometheus
```

## Performance Characteristics

- Event processing: <0.1ms per event
- Memory per record: ~200 bytes
- Batch export: ~1ms per 1000 records
- Total overhead: <5% of simulation time

## OTLP Compliance

Full support for OpenTelemetry Protocol:
- Logs: OTLP log data model
- Traces: Span data with parent/child relationships
- Metrics: Counter, Gauge, Histogram types
- Resource attributes
- Instrumentation scope

## Design Decisions

| Decision | Rationale | Trade-off |
|----------|-----------|-----------|
| OTLP as primary format | Industry standard | Complexity vs custom format |
| Probabilistic log levels | Realistic distribution | Less control vs deterministic |
| In-memory collection | Performance | Memory vs streaming |
| Multi-signal from events | Unified pipeline | Generator complexity |
