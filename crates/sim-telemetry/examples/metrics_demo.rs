//! Demonstration of metric generation and export formats.
//!
//! Run with: cargo run --example metrics_demo

use serde_json::json;
use sim_telemetry::core::TelemetryGenerator;
use sim_telemetry::formats::{JsonLogExporter, OtlpExporter, PrometheusExporter};
use sim_telemetry::generators::{EventToLogGenerator, EventToMetricGenerator, EventToTraceGenerator};

fn main() {
    println!("=== Telemetry Metrics and Export Formats Demo ===\n");

    // Create generators
    let mut metric_gen = EventToMetricGenerator::new();
    let mut log_gen = EventToLogGenerator::new();
    let mut trace_gen = EventToTraceGenerator::new();

    // Simulate some events
    println!("1. Generating telemetry from events...");
    let events = vec![
        json!({
            "id": "evt-001",
            "type": "http_request",
            "source": "urn:sim:api:gateway",
            "time": "2026-03-16T10:00:00Z",
            "data": {
                "method": "GET",
                "path": "/api/users",
                "status_code": 200,
                "latency_ms": 45.2,
                "response_size": 2048
            }
        }),
        json!({
            "id": "evt-002",
            "type": "http_request",
            "source": "urn:sim:api:gateway",
            "time": "2026-03-16T10:00:01Z",
            "data": {
                "method": "POST",
                "path": "/api/orders",
                "status_code": 201,
                "latency_ms": 120.5,
                "response_size": 512
            }
        }),
        json!({
            "id": "evt-003",
            "type": "http_request",
            "source": "urn:sim:api:gateway",
            "time": "2026-03-16T10:00:02Z",
            "data": {
                "method": "GET",
                "path": "/api/users",
                "status_code": 500,
                "latency_ms": 5000.0,
                "response_size": 128
            }
        }),
    ];

    let mut all_telemetry = Vec::new();

    for event in events {
        // Generate different types of telemetry from the same event
        all_telemetry.extend(log_gen.generate(&event).unwrap());
        all_telemetry.extend(trace_gen.generate(&event).unwrap());
        all_telemetry.extend(metric_gen.generate(&event).unwrap());
    }

    println!("   Generated {} telemetry records", all_telemetry.len());

    // Display metric statistics
    println!("\n2. Metric Statistics:");
    println!("   Event counter: {} requests", metric_gen.get_counter("events.http_request").unwrap());

    if let Some(latencies) = metric_gen.get_histogram("http_request.latency_ms.histogram") {
        let avg_latency: f64 = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let min_latency = latencies.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_latency = latencies.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

        println!("   Latency distribution:");
        println!("     - Min: {:.2}ms", min_latency);
        println!("     - Max: {:.2}ms", max_latency);
        println!("     - Avg: {:.2}ms", avg_latency);
    }

    // Export to Prometheus format
    println!("\n3. Prometheus Export Format:");
    println!("{}", "=".repeat(60));
    let prometheus = PrometheusExporter::export(&all_telemetry).unwrap();
    println!("{}", prometheus);

    // Export to OTLP format (metrics)
    println!("4. OTLP Metrics Export Format:");
    println!("{}", "=".repeat(60));
    let otlp_metrics = OtlpExporter::export_metrics(&all_telemetry).unwrap();
    println!("{}", serde_json::to_string_pretty(&otlp_metrics).unwrap());

    // Export logs to JSON
    println!("\n5. JSON Logs Export Format:");
    println!("{}", "=".repeat(60));
    let json_logs = JsonLogExporter::export_to_string(&all_telemetry).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_logs).unwrap();
    let logs = parsed.as_array().unwrap();
    println!("Exported {} log records", logs.len());
    if let Some(first_log) = logs.first() {
        println!("Sample log record:");
        println!("{}", serde_json::to_string_pretty(first_log).unwrap());
    }

    // Export traces to OTLP
    println!("\n6. OTLP Traces Export Format:");
    println!("{}", "=".repeat(60));
    let otlp_traces = OtlpExporter::export_traces(&all_telemetry).unwrap();
    let spans = &otlp_traces["resourceSpans"][0]["scopeSpans"][0]["spans"];
    let span_count = spans.as_array().unwrap().len();
    println!("Exported {} trace spans", span_count);
    if let Some(first_span) = spans.as_array().unwrap().first() {
        println!("Sample trace span:");
        println!("{}", serde_json::to_string_pretty(first_span).unwrap());
    }

    println!("\n=== Demo Complete ===");
}
