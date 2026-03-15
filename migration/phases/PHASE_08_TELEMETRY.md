# Phase 8: Telemetry Generation Framework

**Status**: NOT STARTED
**Priority**: 🟡 MEDIUM
**Estimated Effort**: 1 week
**Dependencies**: None (parallel to Phase 5, 6, 7)
**Blocks**: Observability data generation

---

## Objectives

1. Implement TelemetryGenerator trait and TelemetryCollector
2. Generate logs from simulation events
3. Generate distributed traces with span hierarchy
4. Generate metrics (counters, gauges, histograms)
5. Export OTEL-compliant formats

---

## Files to Create

```
crates/sim-telemetry/src/
├── core/
│   ├── mod.rs              - TelemetryRecord enum
│   └── generator.rs        - TelemetryGenerator trait
├── generators/
│   ├── mod.rs
│   ├── logs.rs             - Event → log records
│   ├── traces.rs           - Event → distributed traces
│   └── metrics.rs          - Event → metrics
├── formats/
│   ├── mod.rs
│   ├── otlp.rs             - OpenTelemetry Protocol
│   ├── json_logs.rs        - JSON structured logs
│   └── prometheus.rs       - Prometheus metrics
└── collector.rs            - TelemetryCollector

```

---

## Key Implementation

### TelemetryGenerator Trait
```rust
pub trait TelemetryGenerator: Send + Sync {
    fn generate(
        &self,
        event: &Event,
        ctx: &SimulationContext
    ) -> Result<Vec<TelemetryRecord>>;
}

pub enum TelemetryRecord {
    Log { timestamp, level, message, attributes },
    Trace { trace_id, span_id, parent_span_id, name, start_time, end_time, attributes },
    Metric { timestamp, name, value, metric_type, tags },
}
```

### LogGenerator Example
```rust
pub struct EventToLogGenerator {
    error_probability: f64,  // Inject errors probabilistically
}

impl TelemetryGenerator for EventToLogGenerator {
    fn generate(&self, event: &Event, ctx: &SimulationContext) -> Result<Vec<TelemetryRecord>> {
        let level = if rand::random::<f64>() < self.error_probability {
            LogLevel::Error
        } else {
            LogLevel::Info
        };

        let message = format!("Event: {}", event.event_type);
        let mut attributes = HashMap::new();
        attributes.insert("event_id".to_string(), event.id.to_string());
        attributes.insert("source".to_string(), event.source.clone());

        Ok(vec![TelemetryRecord::Log {
            timestamp: event.time,
            level,
            message,
            attributes,
        }])
    }
}
```

---

## Tasks Summary

1. **Core Types** (1 day) - TelemetryRecord, TelemetryGenerator trait
2. **Log Generator** (1 day) - Event → structured logs, error injection
3. **Trace Generator** (2 days) - Event → spans with parent-child relationships
4. **Metric Generator** (1 day) - Event aggregation to metrics
5. **OTEL Exporters** (1 day) - JSON, OTLP formats
6. **Integration** (1 day) - Connect to SimulationRunner hooks

---

## Acceptance Criteria

- [ ] TelemetryGenerator trait with 3+ implementations
- [ ] Can generate logs, traces, metrics from events
- [ ] OTEL-compliant output formats
- [ ] Integration with SimulationRunner hooks
- [ ] 10+ unit tests
- [ ] Example: generate telemetry from ghost kitchen simulation

---

**Estimated Effort**: 1 week
**Can Start**: Immediately (parallel work)
**Reference**: Original plan Section 3.4
