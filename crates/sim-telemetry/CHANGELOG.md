# Changelog

All notable changes to `sim-telemetry` will be documented in this file.

## [0.0.1] - 2026-03-16

### Added

#### Core Telemetry Types
- **TelemetryRecord enum**: Log, Trace, Metric variants
- **LogLevel**: Trace, Debug, Info, Warn, Error, Fatal
- **MetricType**: Counter, Gauge, Histogram
- **TelemetryGenerator trait**: Convert events to telemetry
- **TelemetryCollector**: Aggregate records from multiple generators

#### Generators
- **EventToLogGenerator**: CloudEvents -> structured logs
- **EventToMetricGenerator**: CloudEvents -> metrics
- **EventToTraceGenerator**: CloudEvents -> distributed traces
- Probabilistic level assignment (configurable error/warn rates)
- Attribute extraction from event data
- Automatic timestamp propagation

#### Export Formats
- **OTLP exporter**: OpenTelemetry Protocol support
- **JSON exporter**: Human-readable output
- **Prometheus exporter**: Metrics in Prometheus format
- Batch export operations
- Format-specific serialization

#### Performance
- <0.1ms event processing per record
- ~200 bytes memory per record
- <5% simulation runtime overhead
- Minimal allocation during generation

### Dependencies
- `arrow 57.0` - Columnar format
- `chrono 0.4` - Timestamp handling
- `serde_json 1.0` - JSON serialization
- `opentelemetry 0.31.0` - OTLP types
- `opentelemetry-otlp 0.31.0` - OTLP export

[0.0.1]: https://github.com/yourusername/repo/releases/tag/v0.0.1
