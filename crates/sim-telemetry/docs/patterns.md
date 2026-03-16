# sim-telemetry Usage Patterns

## Pattern 1: Basic Telemetry Collection

```rust
use sim_telemetry::{TelemetryCollector, EventToLogGenerator};
use serde_json::json;

let mut collector = TelemetryCollector::new();
collector.add_generator(Box::new(EventToLogGenerator::new()));

let event = json!({
    "id": "evt-001",
    "type": "com.simulation.agent_ticked",
    "source": "urn:sim:agent:agent-001",
    "time": "2026-03-16T10:00:00Z",
    "data": {"agent_id": "agent-001", "tick": 42}
});

collector.process_event(&event)?;
let records = collector.flush();
println!("Collected {} telemetry records", records.len());
```

## Pattern 2: Multi-Signal Generation

```rust
let mut collector = TelemetryCollector::new();
collector.add_generator(Box::new(EventToLogGenerator::new()));
collector.add_generator(Box::new(EventToMetricGenerator::new()));
collector.add_generator(Box::new(EventToTraceGenerator::new()));

// Process events - generates logs, metrics, and traces
for event in events {
    collector.process_event(&event)?;
}
```

## Pattern 3: Export to OTLP

```rust
use sim_telemetry::formats::otlp::OtlpExporter;

let exporter = OtlpExporter::new("http://localhost:4317");
exporter.export_logs(&records)?;
exporter.export_metrics(&records)?;
exporter.export_traces(&records)?;
```

## Best Practices

1. **Adjust error/warn probabilities** for realistic distributions
2. **Flush periodically** to avoid memory growth
3. **Use OTLP for production**, JSON for development
4. **Monitor overhead** - should be <5% of runtime
