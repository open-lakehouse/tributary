# sim-telemetry API Reference

`sim-telemetry` transforms simulation events into OpenTelemetry-compatible observability data (logs, traces, metrics).

## Table of Contents

- [Core Types](#core-types)
- [Generators](#generators)
- [Telemetry Collector](#telemetry-collector)
- [Format Exporters](#format-exporters)

---

## Core Types

### TelemetryRecord

Union type for all telemetry data.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TelemetryRecord {
    Log {
        timestamp: DateTime<Utc>,
        level: LogLevel,
        message: String,
        attributes: HashMap<String, String>,
    },
    Trace {
        trace_id: String,
        span_id: String,
        parent_span_id: Option<String>,
        name: String,
        start_time: DateTime<Utc>,
        end_time: Option<DateTime<Utc>>,
        attributes: HashMap<String, String>,
        status: String,
    },
    Metric {
        timestamp: DateTime<Utc>,
        name: String,
        metric_type: MetricType,
        value: f64,
        attributes: HashMap<String, String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
}
```

---

## Generators

### TelemetryGenerator

Trait for converting events to telemetry.

```rust
pub trait TelemetryGenerator: Send + Sync {
    fn generate(&mut self, event: &serde_json::Value) -> Result<Vec<TelemetryRecord>>;
}
```

### EventToLogGenerator

Converts CloudEvents to structured logs.

```rust
pub struct EventToLogGenerator {
    pub fn new() -> Self;
    pub fn with_probabilities(warn_prob: f64, error_prob: f64) -> Self;
}

impl TelemetryGenerator for EventToLogGenerator {
    fn generate(&mut self, event: &Value) -> Result<Vec<TelemetryRecord>>;
}
```

**Example:**

```rust
use sim_telemetry::{EventToLogGenerator, TelemetryGenerator};

let mut generator = EventToLogGenerator::new();

let event = json!({
    "id": "evt-123",
    "type": "io.kitchen.order.created",
    "source": "/orders/order-123",
    "time": "2026-03-16T10:00:00Z",
    "data": {
        "order_id": "order-123",
        "items": ["burger", "fries"],
        "total": 15.99
    }
});

let records = generator.generate(&event)?;

// Output: Log record with attributes
// - event.id = "evt-123"
// - event.type = "io.kitchen.order.created"
// - data.order_id = "order-123"
// - data.total = "15.99"
```

### EventToTraceGenerator

Converts workflow events to distributed traces.

```rust
pub struct EventToTraceGenerator {
    pub fn new() -> Self;
}

impl TelemetryGenerator for EventToTraceGenerator {
    fn generate(&mut self, event: &Value) -> Result<Vec<TelemetryRecord>>;
}
```

**Example:**

```rust
let mut generator = EventToTraceGenerator::new();

let step_started_event = json!({
    "id": "evt-456",
    "type": "io.simulation.workflow.step_started",
    "source": "/workflows/cooking/instances/inst-789",
    "time": "2026-03-16T10:00:00Z",
    "data": {
        "workflow_id": "cooking",
        "instance_id": "inst-789",
        "step_index": 1,
        "step_type": "grilling"
    }
});

let records = generator.generate(&step_started_event)?;

// Output: Trace span for workflow step
// - trace_id: derived from instance_id
// - span_id: unique per step
// - parent_span_id: previous step's span
```

### EventToMetricGenerator

Converts events to metrics.

```rust
pub struct EventToMetricGenerator {
    pub fn new() -> Self;
}

impl TelemetryGenerator for EventToMetricGenerator {
    fn generate(&mut self, event: &Value) -> Result<Vec<TelemetryRecord>>;
}
```

**Example:**

```rust
let mut generator = EventToMetricGenerator::new();

let completed_event = json!({
    "id": "evt-789",
    "type": "io.kitchen.order.completed",
    "source": "/orders/order-123",
    "time": "2026-03-16T10:15:00Z",
    "data": {
        "order_id": "order-123",
        "duration_seconds": 900
    }
});

let records = generator.generate(&completed_event)?;

// Output: Metrics
// - order_completion_time (histogram) = 900
// - orders_completed_total (counter) = +1
```

---

## Telemetry Collector

Central collector for managing multiple generators.

```rust
pub struct TelemetryCollector {
    pub fn new() -> Self;
    pub fn add_generator(&mut self, name: String, generator: Box<dyn TelemetryGenerator>);
    pub fn process_event(&mut self, event: &Value) -> Result<Vec<TelemetryRecord>>;
    pub fn drain(&mut self) -> Vec<TelemetryRecord>;
}
```

**Example:**

```rust
use sim_telemetry::{TelemetryCollector, EventToLogGenerator, EventToMetricGenerator};

let mut collector = TelemetryCollector::new();

// Add generators
collector.add_generator("logs".to_string(), Box::new(EventToLogGenerator::new()));
collector.add_generator("metrics".to_string(), Box::new(EventToMetricGenerator::new()));

// Process events
for event in events {
    let records = collector.process_event(&event)?;
    // Export records...
}

// Drain buffer
let all_records = collector.drain();
```

---

## Format Exporters

### JSON Logs

```rust
use sim_telemetry::formats::json_logs::export_json_logs;

export_json_logs(&telemetry_records, "./telemetry/logs.jsonl")?;
```

**Output Format (JSONL):**

```json
{"timestamp":"2026-03-16T10:00:00Z","level":"Info","message":"io.kitchen.order.created","attributes":{"event.id":"evt-123","data.order_id":"order-123"}}
{"timestamp":"2026-03-16T10:15:00Z","level":"Info","message":"io.kitchen.order.completed","attributes":{"event.id":"evt-789","data.duration_seconds":"900"}}
```

### OTLP (OpenTelemetry Protocol)

```rust
use sim_telemetry::formats::otlp::export_otlp;

export_otlp(&telemetry_records, "./telemetry/traces.json")?;
```

**Output Format:**

Standard OTLP JSON format compatible with OpenTelemetry collectors.

### Prometheus

```rust
use sim_telemetry::formats::prometheus::export_prometheus;

export_prometheus(&telemetry_records, "./telemetry/metrics.prom")?;
```

**Output Format:**

```
# HELP orders_completed_total Total orders completed
# TYPE orders_completed_total counter
orders_completed_total 150

# HELP order_completion_time Order completion time in seconds
# TYPE order_completion_time histogram
order_completion_time_bucket{le="300"} 45
order_completion_time_bucket{le="600"} 89
order_completion_time_bucket{le="900"} 120
order_completion_time_bucket{le="+Inf"} 150
order_completion_time_sum 78300
order_completion_time_count 150
```

---

## Integration with Simulation

### Complete Pipeline

```rust
use sim_core::{SimulationRunner, SimulationHooks};
use sim_events::{EventDispatcher, ParquetEventWriter};
use sim_telemetry::{TelemetryCollector, EventToLogGenerator, EventToMetricGenerator};

struct TelemetryHooks {
    event_dispatcher: EventDispatcher,
    event_writer: ParquetEventWriter,
    telemetry_collector: TelemetryCollector,
}

impl TelemetryHooks {
    fn new(output_dir: &str) -> Result<Self> {
        let mut collector = TelemetryCollector::new();
        collector.add_generator("logs".to_string(), Box::new(EventToLogGenerator::new()));
        collector.add_generator("metrics".to_string(), Box::new(EventToMetricGenerator::new()));

        Ok(Self {
            event_dispatcher: EventDispatcher::new(),
            event_writer: ParquetEventWriter::new(format!("{}/events.parquet", output_dir))?,
            telemetry_collector: collector,
        })
    }
}

impl SimulationHooks for TelemetryHooks {
    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        // Flush every 100 steps
        if ctx.step() % 100 == 0 {
            // Drain events
            let events = self.event_dispatcher.drain();

            // Write events to Parquet
            self.event_writer.write_events(&events)?;

            // Generate telemetry from events
            for event in events {
                let event_json = serde_json::to_value(&event)?;
                self.telemetry_collector.process_event(&event_json)?;
            }

            // Export telemetry
            let telemetry_records = self.telemetry_collector.drain();
            export_json_logs(&telemetry_records, "./telemetry/logs.jsonl")?;
            export_prometheus(&telemetry_records, "./telemetry/metrics.prom")?;
        }
        Ok(())
    }
}
```

---

## Configuration Options

### Generator Configuration

```rust
// Adjust log levels probabilistically
let log_generator = EventToLogGenerator::with_probabilities(
    0.05,  // 5% warnings
    0.01   // 1% errors
);

// Custom metric extraction
struct CustomMetricGenerator {
    // Extract domain-specific metrics
}

impl TelemetryGenerator for CustomMetricGenerator {
    fn generate(&mut self, event: &Value) -> Result<Vec<TelemetryRecord>> {
        let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match event_type {
            "io.kitchen.order.created" => {
                // Extract order value metric
                if let Some(total) = event.get("data")
                    .and_then(|d| d.get("total"))
                    .and_then(|v| v.as_f64())
                {
                    return Ok(vec![TelemetryRecord::Metric {
                        timestamp: Utc::now(),
                        name: "order_value".to_string(),
                        metric_type: MetricType::Gauge,
                        value: total,
                        attributes: HashMap::new(),
                    }]);
                }
            }
            _ => {}
        }

        Ok(vec![])
    }
}
```

---

## Best Practices

### 1. Generator Selection

Choose generators based on your observability needs:

- **Logs**: All state transitions, debugging
- **Traces**: Workflow execution, latency analysis
- **Metrics**: Aggregated statistics, dashboards

### 2. Sampling

For high-volume simulations, sample telemetry:

```rust
struct SamplingTelemetryCollector {
    collector: TelemetryCollector,
    sample_rate: f64,
}

impl SamplingTelemetryCollector {
    fn process_event_sampled(&mut self, event: &Value) -> Result<Vec<TelemetryRecord>> {
        if rand::random::<f64>() < self.sample_rate {
            self.collector.process_event(event)
        } else {
            Ok(vec![])
        }
    }
}
```

### 3. Export Strategy

- **Development**: JSON logs for easy inspection
- **Production**: OTLP for integration with observability platforms
- **Metrics**: Prometheus for monitoring and alerting

### 4. Performance

- Batch telemetry generation
- Flush periodically, not per event
- Use async exporters for large volumes

---

## See Also

- [sim-events API Reference](./sim-events.md) - Event infrastructure
- [sim-core API Reference](./sim-core.md) - Simulation runtime
