# Telemetry

**Last Updated:** March 16, 2026

## Overview

The telemetry framework generates observability data (logs, traces, metrics) from simulation events. It provides OTLP-compliant export for integration with OpenTelemetry collectors and other observability platforms.

## Objective

Learn how to:
- Generate logs, traces, and metrics from simulation events
- Export to OTLP, JSON, and Prometheus formats
- Configure telemetry generators
- Integrate with SimulationRunner
- Minimize performance overhead

## Prerequisites

- [Events](events.md) understanding
- [Getting Started](../GETTING_STARTED.md)
- Basic observability concepts (logs, traces, metrics)

## TelemetryCollector

The TelemetryCollector aggregates telemetry from multiple generators:

```rust
use sim_telemetry::{TelemetryCollector, EventToLogGenerator};

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

collector.process_event(&event)?;

// Retrieve telemetry
let records = collector.flush();
println!("Generated {} telemetry records", records.len());
```

## Telemetry Types

### TelemetryRecord Enum

```rust
pub enum TelemetryRecord {
    Log(LogRecord),
    Trace(TraceRecord),
    Metric(MetricRecord),
}
```

### LogRecord

Structured log entries with severity levels:

```rust
pub struct LogRecord {
    pub timestamp: DateTime<Utc>,
    pub severity: LogSeverity,  // Debug, Info, Warn, Error, Fatal
    pub body: String,
    pub attributes: HashMap<String, String>,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
}
```

### TraceRecord

Distributed trace spans with parent-child relationships:

```rust
pub struct TraceRecord {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub attributes: HashMap<String, String>,
}
```

### MetricRecord

Counters, gauges, and histograms:

```rust
pub struct MetricRecord {
    pub name: String,
    pub metric_type: MetricType,  // Counter, Gauge, Histogram
    pub value: f64,
    pub timestamp: DateTime<Utc>,
    pub tags: HashMap<String, String>,
}
```

## Built-In Generators

### EventToLogGenerator

Converts events to structured logs:

```rust
use sim_telemetry::EventToLogGenerator;

let mut generator = EventToLogGenerator::new();
let records = generator.generate(&event)?;

// Generates logs with automatic severity:
// - "error" events → Error
// - "warn" events → Warn
// - Default → Info
```

### EventToTraceGenerator

Generates distributed traces:

```rust
use sim_telemetry::EventToTraceGenerator;

let mut generator = EventToTraceGenerator::new();
let records = generator.generate(&event)?;

// Automatic trace correlation:
// - workflow_id → trace_id
// - step_index → span hierarchy
// - agent_id → parent span
```

### EventToMetricGenerator

Generates metrics from events:

```rust
use sim_telemetry::EventToMetricGenerator;

let mut generator = EventToMetricGenerator::new();
let records = generator.generate(&event)?;

// Auto-generates:
// - Event counters (events_total{type="..."})
// - Duration histograms (duration_seconds{...})
// - State gauges (agents_active{status="..."})
```

## Export Formats

### OTLP Export

For OpenTelemetry Collector:

```rust
use sim_telemetry::formats::otlp::OtlpExporter;

let exporter = OtlpExporter::new("http://localhost:4317");

// Export logs
exporter.export_logs(&records)?;

// Export traces
exporter.export_traces(&records)?;

// Export metrics
exporter.export_metrics(&records)?;
```

### JSON Export

For local analysis:

```rust
let json = serde_json::to_string_pretty(&records)?;
std::fs::write("telemetry.json", json)?;
```

### Prometheus Export

For Prometheus scraping:

```rust
use sim_telemetry::formats::prometheus::PrometheusExporter;

let exporter = PrometheusExporter::new();
let prometheus_text = exporter.export_metrics(&records)?;

// Output format:
// # TYPE events_total counter
// events_total{type="agent.spawned"} 42
// # TYPE duration_seconds histogram
// duration_seconds_bucket{le="0.1"} 10
```

## Integration with SimulationRunner

### Telemetry Hooks

```rust
use sim_core::runner::telemetry::SimulationTelemetry;

let mut runner = SimulationRunner::new(config);

// Attach telemetry
let telemetry = SimulationTelemetry::new();
runner.set_hooks(Box::new(telemetry));

// Run simulation with automatic telemetry collection
runner.run_for_steps(100)?;
```

### Custom Telemetry Hooks

```rust
pub struct CustomTelemetryHooks {
    pub collector: TelemetryCollector,
    pub exporter: OtlpExporter,
}

impl SimulationHooks for CustomTelemetryHooks {
    fn on_agents_ticked(
        &mut self,
        ctx: &SimulationContext,
        results: &[AgentTickResult],
    ) -> Result<()> {
        // Convert agent ticks to events
        for result in results {
            let event = self.agent_tick_to_event(result, ctx);
            self.collector.process_event(&event)?;
        }
        Ok(())
    }

    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        // Export every 100 ticks
        if ctx.step() % 100 == 0 {
            let records = self.collector.flush();
            self.exporter.export_batch(&records)?;
        }
        Ok(())
    }

    fn on_end(&mut self, _ctx: &SimulationContext) -> Result<()> {
        // Final flush
        let records = self.collector.flush();
        self.exporter.export_batch(&records)?;
        Ok(())
    }
}
```

## Example Telemetry

### Log Example

```json
{
  "timestamp": "2026-03-16T10:00:00Z",
  "severity": "INFO",
  "body": "Agent spawned",
  "attributes": {
    "agent_id": "019cf220-...",
    "agent_type": "customer",
    "location": "downtown"
  }
}
```

### Trace Example

```json
{
  "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736",
  "span_id": "00f067aa0ba902b7",
  "parent_span_id": "00f067aa0ba902b6",
  "name": "workflow.step.execute",
  "start_time": "2026-03-16T10:00:00Z",
  "end_time": "2026-03-16T10:00:05Z",
  "attributes": {
    "workflow_type": "food_order",
    "step_index": "1",
    "resource_type": "kitchen"
  }
}
```

### Metric Example

```
# TYPE events_total counter
events_total{type="agent.spawned"} 42
events_total{type="workflow.started"} 38

# TYPE duration_seconds histogram
duration_seconds_bucket{le="0.1"} 10
duration_seconds_bucket{le="0.5"} 25
duration_seconds_bucket{le="1.0"} 38
duration_seconds_sum 32.5
duration_seconds_count 38
```

## Performance

Phase 8 achieved:

- **Telemetry overhead**: <1% (5x better than <5% target)
- **Log generation**: 4-36x faster than required
- **Trace generation**: Sub-millisecond
- **Metric aggregation**: Sub-millisecond

### Performance Tips

```rust
// ✅ Good - batch processing
let events = vec![event1, event2, event3];
for event in events {
    collector.process_event(&event)?;
}

// Flush periodically
if collector.record_count() > 1000 {
    let records = collector.flush();
    exporter.export_batch(&records)?;
}

// ❌ Bad - excessive flushing
collector.process_event(&event)?;
collector.flush();  // Too frequent!
```

## Custom Generators

Implement `TelemetryGenerator` for custom logic:

```rust
use sim_telemetry::{TelemetryGenerator, TelemetryRecord};
use serde_json::Value;

pub struct CustomMetricGenerator {
    metric_name: String,
}

impl TelemetryGenerator for CustomMetricGenerator {
    fn generate(&mut self, event: &Value) -> Result<Vec<TelemetryRecord>, TelemetryError> {
        let event_type = event["type"].as_str().unwrap_or("");

        if event_type.starts_with("io.kitchen.order") {
            let metric = TelemetryRecord::Metric(MetricRecord {
                name: self.metric_name.clone(),
                metric_type: MetricType::Counter,
                value: 1.0,
                timestamp: Utc::now(),
                tags: HashMap::from([
                    ("event_type".to_string(), event_type.to_string()),
                ]),
            });

            Ok(vec![metric])
        } else {
            Ok(vec![])
        }
    }
}
```

## Common Pitfalls

### Pitfall 1: Not Flushing Records

```rust
// ❌ Wrong - records never exported
impl SimulationHooks for MyHooks {
    fn on_end(&mut self, ctx: &SimulationContext) -> Result<()> {
        // Forgot to flush!
        Ok(())
    }
}

// ✅ Correct - flush and export
impl SimulationHooks for MyHooks {
    fn on_end(&mut self, ctx: &SimulationContext) -> Result<()> {
        let records = self.collector.flush();
        self.exporter.export_batch(&records)?;
        Ok(())
    }
}
```

### Pitfall 2: Excessive Telemetry

```rust
// ❌ Wrong - too much telemetry
fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
    // Don't emit telemetry every tick for every agent!
    self.emit_log("Agent ticked")?;
    self.emit_metric("tick_count", 1.0)?;
    // ...
}

// ✅ Correct - sample or aggregate
fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
    // Sample: only emit for significant state changes
    if self.status_changed() {
        self.emit_log("Status changed")?;
    }
    // ...
}
```

### Pitfall 3: Missing Trace Context

```rust
// ❌ Wrong - broken trace hierarchy
TraceRecord {
    trace_id: "...",
    span_id: "...",
    parent_span_id: None,  // No parent!
    // ...
}

// ✅ Correct - maintain hierarchy
TraceRecord {
    trace_id: workflow.trace_id,
    span_id: generate_span_id(),
    parent_span_id: Some(workflow.root_span_id),
    // ...
}
```

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_generation() {
        let mut generator = EventToLogGenerator::new();

        let event = json!({
            "type": "agent.spawned",
            "time": "2026-03-16T10:00:00Z",
            "data": {"agent_id": "001"}
        });

        let records = generator.generate(&event).unwrap();
        assert_eq!(records.len(), 1);

        match &records[0] {
            TelemetryRecord::Log(log) => {
                assert_eq!(log.severity, LogSeverity::Info);
                assert!(log.body.contains("spawned"));
            }
            _ => panic!("Expected log record"),
        }
    }

    #[test]
    fn test_collector() {
        let mut collector = TelemetryCollector::new();
        collector.add_generator(Box::new(EventToLogGenerator::new()));

        let event = json!({"type": "test", "time": "2026-03-16T10:00:00Z"});
        collector.process_event(&event).unwrap();

        let records = collector.flush();
        assert!(!records.is_empty());
    }
}
```

## Related Documentation

- [Events](events.md) - Event generation and storage
- [Agents](agents.md) - Agent state telemetry
- [Workflows](workflows.md) - Workflow trace generation

## Summary

Telemetry framework provides:

- Logs, traces, and metrics from simulation events
- OTLP, JSON, and Prometheus export formats
- <1% performance overhead
- Custom generator support via trait
- Full OpenTelemetry integration
- 104 tests validating all components

---

**Last Updated:** March 16, 2026
