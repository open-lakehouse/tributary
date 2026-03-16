# sim-telemetry

OpenTelemetry-compatible telemetry generation for simulations.

## Overview

`sim-telemetry` provides tools for generating observability data (logs, traces, metrics) from simulation events. It bridges the gap between simulation frameworks and modern observability platforms.

## Features

- **Multi-Signal Telemetry**: Generate logs, traces, and metrics from simulation events
- **CloudEvents Compatible**: Based on CloudEvents specification for event interchange
- **Multiple Export Formats**: OTLP, JSON, and Prometheus formats
- **Minimal Overhead**: <5% performance impact on simulation runtime
- **Extensible**: Custom generators via `TelemetryGenerator` trait

## Architecture

```
Simulation Event → TelemetryCollector → [Generators] → Telemetry Records → Export
```

### Core Components

1. **TelemetryRecord**: Enum representing logs, traces, or metrics
2. **TelemetryGenerator**: Trait for converting events to telemetry
3. **TelemetryCollector**: Aggregates records from multiple generators
4. **Export Formats**: OTLP, JSON, and Prometheus exporters

## Installation

Add to `Cargo.toml`:

```toml
[dependencies]
sim-telemetry = { path = "../sim-telemetry" }
```

## Quick Start

### Basic Usage

```rust
use sim_telemetry::{TelemetryCollector, EventToLogGenerator};
use serde_json::json;

// Create collector with generators
let mut collector = TelemetryCollector::new();
collector.add_generator(Box::new(EventToLogGenerator::new()));

// Process CloudEvents
let event = json!({
    "specversion": "1.0",
    "id": "evt-001",
    "source": "urn:sim:agent:agent-001",
    "type": "com.simulation.agent_ticked",
    "time": "2026-03-16T10:00:00Z",
    "data": {"agent_id": "agent-001", "tick": 42}
});

collector.process_event(&event).unwrap();

// Retrieve telemetry
let records = collector.flush();
println!("Collected {} records", records.len());
```

### SimulationRunner Integration

```rust
use sim_core::{SimulationRunner, SimulationConfig};
use sim_core::runner::telemetry::SimulationTelemetry;

let config = SimulationConfig::default();
let mut runner = SimulationRunner::new(config);

// Attach telemetry hooks
let telemetry = SimulationTelemetry::new();
runner.set_hooks(Box::new(telemetry));

// Run simulation with automatic telemetry collection
runner.run_for_steps(100)?;

// Access collected telemetry
// (requires downcasting hooks or using TelemetryHooks trait)
```

## Generators

### EventToLogGenerator

Generates structured logs from events:

```rust
use sim_telemetry::EventToLogGenerator;

let mut generator = EventToLogGenerator::new();
let records = generator.generate(&event)?;
```

**Output**: Logs with severity levels (Info, Warn, Error, etc.)

### EventToMetricGenerator

Generates metrics (counters, gauges, histograms):

```rust
use sim_telemetry::EventToMetricGenerator;

let mut generator = EventToMetricGenerator::new();
let records = generator.generate(&event)?;
```

**Output**: Metrics with tags for aggregation

### EventToTraceGenerator

Generates distributed traces:

```rust
use sim_telemetry::EventToTraceGenerator;

let mut generator = EventToTraceGenerator::new();
let records = generator.generate(&event)?;
```

**Output**: Trace spans with relationships

## Export Formats

### JSON Export

Simple serialization for storage or analysis:

```rust
let json = serde_json::to_string_pretty(&records)?;
std::fs::write("telemetry.json", json)?;
```

### OTLP Export

For OpenTelemetry Collector:

```rust
use sim_telemetry::formats::otlp::OtlpExporter;

let exporter = OtlpExporter::new("http://localhost:4317");
exporter.export_logs(&records)?;
exporter.export_traces(&records)?;
exporter.export_metrics(&records)?;
```

### Prometheus Export

For Prometheus scraping:

```rust
use sim_telemetry::formats::prometheus::PrometheusExporter;

let exporter = PrometheusExporter::new();
let prometheus_text = exporter.export_metrics(&records)?;
```

## Examples

### Complete Workflow

See `examples/telemetry_demo.rs` for end-to-end demonstration:

```bash
cargo run -p sim-telemetry --example telemetry_demo
```

### Generate Logs Only

```bash
cargo run -p sim-telemetry --example generate_logs
```

### Metrics Demo

```bash
cargo run -p sim-telemetry --example metrics_demo
```

### Trace Demo

```bash
cargo run -p sim-telemetry --example trace_demo
```

## Performance

Telemetry overhead is designed to be minimal:

- **Event Processing**: <0.1ms per event
- **Memory Overhead**: ~200 bytes per record
- **Total Impact**: <5% of simulation runtime

### Benchmarks

Run performance tests:

```bash
cargo bench -p sim-telemetry
```

## Integration Patterns

### Pattern 1: Inline Collection

Collect telemetry during simulation:

```rust
let mut telemetry = SimulationTelemetry::new();
runner.set_hooks(Box::new(telemetry));
runner.run_for_steps(1000)?;
```

### Pattern 2: Post-Processing

Process events after simulation:

```rust
let events = simulation.export_events();
for event in events {
    collector.process_event(&event)?;
}
```

### Pattern 3: Streaming Export

Export in real-time:

```rust
struct StreamingTelemetry {
    collector: TelemetryCollector,
    exporter: OtlpExporter,
}

impl SimulationHooks for StreamingTelemetry {
    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        if ctx.step() % 100 == 0 {
            let records = self.collector.flush();
            self.exporter.export_batch(&records)?;
        }
        Ok(())
    }
}
```

## Custom Generators

Implement `TelemetryGenerator` for custom telemetry:

```rust
use sim_telemetry::{TelemetryGenerator, TelemetryRecord, TelemetryError};
use serde_json::Value;

struct CustomGenerator;

impl TelemetryGenerator for CustomGenerator {
    fn generate(&mut self, event: &Value) -> Result<Vec<TelemetryRecord>, TelemetryError> {
        // Custom logic here
        Ok(vec![])
    }
}

collector.add_generator(Box::new(CustomGenerator));
```

## Testing

Run all tests:

```bash
cargo test -p sim-telemetry
```

Run specific tests:

```bash
cargo test -p sim-telemetry --lib core::collector
cargo test -p sim-core runner::telemetry
```

## API Documentation

Generate and view full API documentation:

```bash
cargo doc -p sim-telemetry --open
```

## Troubleshooting

### High Memory Usage

If memory usage is high, flush records more frequently:

```rust
if collector.record_count() > 10000 {
    let records = collector.flush();
    export_batch(&records)?;
}
```

### Missing Telemetry

Ensure generators are added before processing events:

```rust
collector.add_generator(Box::new(EventToLogGenerator::new()));
// Must come after adding generators
collector.process_event(&event)?;
```

### Performance Impact

Profile telemetry overhead:

```bash
cargo flamegraph --example telemetry_demo
```

## Future Enhancements

- [ ] Arrow-based batch processing
- [ ] Automatic sampling strategies
- [ ] Built-in aggregation functions
- [ ] Real-time streaming export
- [ ] Schema validation for events

## License

See workspace LICENSE file.
