//! Performance benchmarks for sim-telemetry event processing
//!
//! This benchmark suite validates that telemetry overhead meets performance targets:
//! - Log generation: <10µs (target: <1µs)
//! - Trace generation: <10µs (target: <1µs)
//! - Metric generation: <10µs (target: <1µs)
//! - Collector 10K events: <100ms (target: <50ms)
//! - OTLP export 100 traces: <10ms (target: <5ms)
//!
//! Run with: cargo bench -p sim-telemetry --bench telemetry_performance

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use serde_json::json;
use sim_telemetry::core::{TelemetryCollector, TelemetryGenerator};
use sim_telemetry::formats::otlp::OtlpExporter;
use sim_telemetry::generators::{
    EventToLogGenerator, EventToMetricGenerator, EventToTraceGenerator,
};

/// Create a sample CloudEvents-compliant event for benchmarking
fn create_sample_event() -> serde_json::Value {
    json!({
        "specversion": "1.0",
        "id": "test-001",
        "source": "urn:sim:agent:agent-001",
        "type": "com.simulation.agent_ticked",
        "time": "2026-03-16T10:00:00Z",
        "datacontenttype": "application/json",
        "data": {
            "agent_id": "agent-001",
            "tick": 1,
            "state": {
                "position": {"x": 10.0, "y": 20.0},
                "energy": 75.5
            }
        }
    })
}

/// Create multiple sample events for batch testing
fn create_sample_events(count: usize) -> Vec<serde_json::Value> {
    (0..count)
        .map(|i| {
            json!({
                "specversion": "1.0",
                "id": format!("test-{:06}", i),
                "source": format!("urn:sim:agent:agent-{:03}", i % 100),
                "type": "com.simulation.agent_ticked",
                "time": "2026-03-16T10:00:00Z",
                "datacontenttype": "application/json",
                "data": {
                    "agent_id": format!("agent-{:03}", i % 100),
                    "tick": i,
                    "state": {
                        "position": {"x": i as f64, "y": i as f64 * 2.0},
                        "energy": 50.0 + (i % 50) as f64
                    }
                }
            })
        })
        .collect()
}

/// Benchmark: Log generation per event
///
/// Target: <10µs (preferred: <1µs)
///
/// Log generation is the most common telemetry operation and must be extremely fast.
fn benchmark_log_generation(c: &mut Criterion) {
    let mut generator = EventToLogGenerator::default();
    let event = create_sample_event();

    c.bench_function("generate_log", |b| {
        b.iter(|| black_box(generator.generate(black_box(&event))));
    });
}

/// Benchmark: Log generation with different sampling rates
///
/// Tests performance with varying probabilities of debug/trace logging.
fn benchmark_log_generation_sampling(c: &mut Criterion) {
    let event = create_sample_event();

    for (trace_prob, debug_prob) in [(0.0, 0.0), (0.05, 0.10), (0.20, 0.30)] {
        c.bench_with_input(
            BenchmarkId::new(
                "log_sampling",
                format!("{:.2}_{:.2}", trace_prob, debug_prob),
            ),
            &(trace_prob, debug_prob),
            |b, &(t, d)| {
                let mut generator = EventToLogGenerator::with_probabilities(t, d);
                b.iter(|| black_box(generator.generate(black_box(&event))));
            },
        );
    }
}

/// Benchmark: Trace generation per event
///
/// Target: <10µs (preferred: <1µs)
///
/// Trace generation creates span records with timing information.
fn benchmark_trace_generation(c: &mut Criterion) {
    let mut generator = EventToTraceGenerator::default();
    let event = create_sample_event();

    c.bench_function("generate_trace", |b| {
        b.iter(|| black_box(generator.generate(black_box(&event))));
    });
}

/// Benchmark: Metric generation per event
///
/// Target: <10µs (preferred: <1µs)
///
/// Metric generation extracts numeric values from events.
fn benchmark_metric_generation(c: &mut Criterion) {
    let mut generator = EventToMetricGenerator::default();
    let event = create_sample_event();

    c.bench_function("generate_metric", |b| {
        b.iter(|| black_box(generator.generate(black_box(&event))));
    });
}

/// Benchmark: Collector batch processing
///
/// Target for 10K events: <100ms (preferred: <50ms = 5µs per event)
///
/// Tests the overhead of processing many events through the collector pipeline.
fn benchmark_collector_processing(c: &mut Criterion) {
    for event_count in [100, 1000, 10000] {
        c.bench_with_input(
            BenchmarkId::new("collector_batch", event_count),
            &event_count,
            |b, &count| {
                let events = create_sample_events(count);
                b.iter(|| {
                    let mut collector = TelemetryCollector::new();
                    collector.add_generator(Box::new(EventToLogGenerator::default()));
                    collector.add_generator(Box::new(EventToTraceGenerator::default()));
                    collector.add_generator(Box::new(EventToMetricGenerator::default()));

                    for event in &events {
                        let _ = black_box(collector.process_event(black_box(event)));
                    }

                    black_box(collector.flush());
                });
            },
        );
    }
}

/// Benchmark: Collector with single generator
///
/// Measures overhead of just log generation at scale.
fn benchmark_collector_logs_only(c: &mut Criterion) {
    for event_count in [100, 1000, 10000] {
        c.bench_with_input(
            BenchmarkId::new("collector_logs_only", event_count),
            &event_count,
            |b, &count| {
                let events = create_sample_events(count);
                b.iter(|| {
                    let mut collector = TelemetryCollector::new();
                    collector.add_generator(Box::new(EventToLogGenerator::default()));

                    for event in &events {
                        let _ = black_box(collector.process_event(black_box(event)));
                    }

                    black_box(collector.flush());
                });
            },
        );
    }
}

/// Benchmark: OTLP export for traces
///
/// Target for 100 traces: <10ms (preferred: <5ms)
///
/// Tests the overhead of converting telemetry records to OTLP format.
fn benchmark_otlp_export(c: &mut Criterion) {
    let mut generator = EventToTraceGenerator::default();
    let event = create_sample_event();

    // Generate 100 trace records (collect owned TelemetryRecord)
    let records: Vec<sim_telemetry::core::TelemetryRecord> = (0..100)
        .flat_map(|_| generator.generate(&event).unwrap())
        .collect();

    c.bench_function("otlp_export_100_traces", |b| {
        b.iter(|| black_box(OtlpExporter::export_traces(black_box(&records))));
    });
}

/// Benchmark: OTLP export for logs
///
/// Tests log export performance at scale.
fn benchmark_otlp_log_export(c: &mut Criterion) {
    let mut generator = EventToLogGenerator::default();
    let events = create_sample_events(100);

    let records: Vec<sim_telemetry::core::TelemetryRecord> = events
        .iter()
        .flat_map(|e| generator.generate(e).unwrap())
        .collect();

    c.bench_function("otlp_export_100_logs", |b| {
        b.iter(|| black_box(OtlpExporter::export_logs(black_box(&records))));
    });
}

/// Benchmark: OTLP export for metrics
///
/// Tests metric export performance at scale.
fn benchmark_otlp_metric_export(c: &mut Criterion) {
    let mut generator = EventToMetricGenerator::default();
    let events = create_sample_events(100);

    let records: Vec<sim_telemetry::core::TelemetryRecord> = events
        .iter()
        .flat_map(|e| generator.generate(e).unwrap())
        .collect();

    c.bench_function("otlp_export_100_metrics", |b| {
        b.iter(|| black_box(OtlpExporter::export_metrics(black_box(&records))));
    });
}

/// Benchmark: End-to-end telemetry pipeline
///
/// Measures total overhead from event to OTLP export.
fn benchmark_end_to_end_pipeline(c: &mut Criterion) {
    let events = create_sample_events(100);

    c.bench_function("end_to_end_100_events", |b| {
        b.iter(|| {
            let mut collector = TelemetryCollector::new();
            collector.add_generator(Box::new(EventToLogGenerator::default()));
            collector.add_generator(Box::new(EventToTraceGenerator::default()));
            collector.add_generator(Box::new(EventToMetricGenerator::default()));

            for event in &events {
                let _ = collector.process_event(event);
            }

            let records = collector.flush();

            // Separate by type and export (owned records, not references)
            let logs: Vec<_> = records
                .iter()
                .filter(|r| matches!(r, sim_telemetry::core::TelemetryRecord::Log { .. }))
                .cloned()
                .collect();
            let traces: Vec<_> = records
                .iter()
                .filter(|r| matches!(r, sim_telemetry::core::TelemetryRecord::Trace { .. }))
                .cloned()
                .collect();
            let metrics: Vec<_> = records
                .iter()
                .filter(|r| matches!(r, sim_telemetry::core::TelemetryRecord::Metric { .. }))
                .cloned()
                .collect();

            let _ = black_box(OtlpExporter::export_logs(&logs));
            let _ = black_box(OtlpExporter::export_traces(&traces));
            let _ = black_box(OtlpExporter::export_metrics(&metrics));
        });
    });
}

/// Benchmark: Memory allocation patterns
///
/// Tests memory overhead by creating and dropping collectors repeatedly.
fn benchmark_memory_overhead(c: &mut Criterion) {
    c.bench_function("collector_creation_overhead", |b| {
        b.iter(|| {
            let mut collector = TelemetryCollector::new();
            collector.add_generator(Box::new(EventToLogGenerator::default()));
            collector.add_generator(Box::new(EventToTraceGenerator::default()));
            collector.add_generator(Box::new(EventToMetricGenerator::default()));
            black_box(collector);
        });
    });
}

/// Benchmark: JSON serialization overhead
///
/// Measures the cost of serializing telemetry records to JSON.
fn benchmark_json_serialization(c: &mut Criterion) {
    let mut generator = EventToLogGenerator::default();
    let event = create_sample_event();
    let records = generator.generate(&event).unwrap();

    c.bench_function("json_serialize_record", |b| {
        b.iter(|| black_box(serde_json::to_string(black_box(&records[0]))));
    });
}

criterion_group!(
    benches,
    benchmark_log_generation,
    benchmark_log_generation_sampling,
    benchmark_trace_generation,
    benchmark_metric_generation,
    benchmark_collector_processing,
    benchmark_collector_logs_only,
    benchmark_otlp_export,
    benchmark_otlp_log_export,
    benchmark_otlp_metric_export,
    benchmark_end_to_end_pipeline,
    benchmark_memory_overhead,
    benchmark_json_serialization,
);
criterion_main!(benches);
