# sim-telemetry

Turns CloudEvents-shaped JSON events into logs, trace spans and metrics, and exports them
as OTLP JSON, a JSON array of logs, or Prometheus text.

sim-core uses this crate only behind its `telemetry` feature
(`sim_core::runner::telemetry::SimulationTelemetry`). No other workspace crate depends on it.

## Input

Generators take a `serde_json::Value` event.

- `type` is required by all three generators.
- `time` (RFC 3339) is required by the metric and trace generators. The log generator uses
  it when present.
- `source` is required by the trace generator.
- `id` seeds the log level choice in the log generator. A missing `id` is treated as `"default"`.
- `data` is read for metric gauges and log attributes.

A missing required field returns `TelemetryError::InvalidEvent`. An unparsable `time` makes
the log, trace and metric generators use the current wall-clock time instead.

## Generators

| Generator | Output per event |
|---|---|
| `EventToLogGenerator` | One `Log`. The level comes from a hash of the event `id`, not from an RNG. Defaults: 5% Error/Fatal, 10% Trace, the rest Debug/Info/Warn. `with_probabilities(error, trace)` changes this. |
| `EventToTraceGenerator` | One `Trace` span. Events from the same `source` share a `trace_id`. Each event becomes a child of the previous span from that source, except `created` and `initialized` events, which start a new root. |
| `EventToMetricGenerator` | A counter `events.<type>`, and a gauge `<type>.<key>` for each numeric field in `data`. Histograms (`<type>.<key>.histogram`) are kept in state and read with `get_histogram`; they are not returned as records. |

Span durations depend on the event type: 10 ms for `created` and `destroyed`, 500 ms for
`workflow`, 50 ms for `ticked`, 100 ms otherwise.

Each generator implements `TelemetryGenerator::generate(&mut self, &Value)`, which returns
`Vec<TelemetryRecord>`. Implement the trait for custom generators.

## Collecting

```rust
use sim_telemetry::{EventToLogGenerator, EventToMetricGenerator, TelemetryCollector};
use serde_json::json;

let mut collector = TelemetryCollector::new();
collector.add_generator(Box::new(EventToLogGenerator::new()));
collector.add_generator(Box::new(EventToMetricGenerator::new()));

let event = json!({
    "id": "evt-001",
    "type": "com.example.agent_ticked",
    "source": "urn:sim:agent:agent-001",
    "time": "2026-03-16T10:00:00Z",
    "data": { "tick": 1 }
});
let generated = collector.process_event(&event)?; // number of records
let records = collector.flush();                   // drains the buffer
```

Every event needs an RFC 3339 `time` (its simulation time). A missing or unparsable `time`
is an `InvalidEvent` error; generators never fall back to wall-clock time.

This crate does not depend on sim-events (sim-core optionally depends on it, and sim-events
depends on sim-core), so it does not implement `sim_events::EventProcessor`. Feed events to a
`TelemetryCollector` directly, as sim-core's `telemetry` feature does.

## Exporters

| Exporter | Functions | Records used |
|---|---|---|
| `OtlpExporter` | `export_logs`, `export_traces`, `export_metrics` | logs, spans, metrics (OTLP JSON) |
| `JsonLogExporter` | `export_to_file(records, path)`, `export_to_string(records)` | logs (pretty-printed JSON array) |
| `PrometheusExporter` | `export`, `export_without_type` | metrics (text exposition format) |

Examples: `cargo run -p sim-telemetry --example {telemetry_demo,metrics_demo,trace_demo}`;
`generate_logs` reads `test_data/sample_events.json`. Benchmark: `cargo bench -p sim-telemetry`.
API reference: `cargo doc -p sim-telemetry --open`.
