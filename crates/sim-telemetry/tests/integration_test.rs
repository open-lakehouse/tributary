//! Integration tests for metrics generation and export formats.

use chrono::Utc;
use serde_json::json;
use sim_telemetry::core::{TelemetryGenerator, TelemetryRecord};
use sim_telemetry::formats::{JsonLogExporter, OtlpExporter, PrometheusExporter};
use sim_telemetry::generators::{EventToLogGenerator, EventToMetricGenerator, EventToTraceGenerator};
use std::collections::HashMap;

#[test]
fn test_end_to_end_metric_generation_and_export() {
    // Create a metric generator
    let mut metric_gen = EventToMetricGenerator::new();

    // Generate metrics from events
    let events = vec![
        json!({
            "type": "http_request",
            "time": "2026-03-16T10:00:00Z",
            "data": {
                "latency_ms": 120.5,
                "status_code": 200
            }
        }),
        json!({
            "type": "http_request",
            "time": "2026-03-16T10:00:01Z",
            "data": {
                "latency_ms": 95.3,
                "status_code": 200
            }
        }),
        json!({
            "type": "http_request",
            "time": "2026-03-16T10:00:02Z",
            "data": {
                "latency_ms": 150.8,
                "status_code": 500
            }
        }),
    ];

    let mut all_metrics = Vec::new();
    for event in events {
        let metrics = metric_gen.generate(&event).unwrap();
        all_metrics.extend(metrics);
    }

    // Verify we have the expected metrics
    assert!(!all_metrics.is_empty());

    // Check counter increments
    assert_eq!(metric_gen.get_counter("events.http_request"), Some(3));

    // Check histogram has all latency values
    let histogram = metric_gen.get_histogram("http_request.latency_ms.histogram").unwrap();
    assert_eq!(histogram.len(), 3);
    assert_eq!(histogram[0], 120.5);
    assert_eq!(histogram[1], 95.3);
    assert_eq!(histogram[2], 150.8);

    // Export to Prometheus format
    let prometheus_output = PrometheusExporter::export(&all_metrics).unwrap();
    assert!(prometheus_output.contains("events_http_request"));
    assert!(prometheus_output.contains("# TYPE"));

    // Export to OTLP format
    let otlp_metrics = OtlpExporter::export_metrics(&all_metrics).unwrap();
    assert!(otlp_metrics.get("resourceMetrics").is_some());
}

#[test]
fn test_mixed_telemetry_export_formats() {
    // Generate different types of telemetry
    let mut log_gen = EventToLogGenerator::new();
    let mut trace_gen = EventToTraceGenerator::new();
    let mut metric_gen = EventToMetricGenerator::new();

    let event = json!({
        "id": "evt-001",
        "type": "transaction_completed",
        "source": "urn:sim:service:api",
        "time": "2026-03-16T10:00:00Z",
        "data": {
            "user_id": "user-123",
            "amount": 99.99,
            "duration_ms": 250.0
        }
    });

    let logs = log_gen.generate(&event).unwrap();
    let traces = trace_gen.generate(&event).unwrap();
    let metrics = metric_gen.generate(&event).unwrap();

    // Combine all telemetry
    let mut all_records = Vec::new();
    all_records.extend(logs);
    all_records.extend(traces);
    all_records.extend(metrics);

    // Export logs to JSON
    let json_logs = JsonLogExporter::export_to_string(&all_records).unwrap();
    assert!(json_logs.contains("transaction_completed"));

    // Export logs to OTLP
    let otlp_logs = OtlpExporter::export_logs(&all_records).unwrap();
    let log_records = otlp_logs["resourceLogs"][0]["scopeLogs"][0]["logRecords"]
        .as_array()
        .unwrap();
    assert!(!log_records.is_empty());

    // Export traces to OTLP
    let otlp_traces = OtlpExporter::export_traces(&all_records).unwrap();
    let spans = otlp_traces["resourceSpans"][0]["scopeSpans"][0]["spans"]
        .as_array()
        .unwrap();
    assert!(!spans.is_empty());

    // Export metrics to Prometheus
    let prometheus = PrometheusExporter::export(&all_records).unwrap();
    assert!(prometheus.contains("events_transaction_completed"));
}

#[test]
fn test_otlp_format_compliance() {
    // Create sample telemetry records
    let mut attributes = HashMap::new();
    attributes.insert("service.name".to_string(), "test-service".to_string());

    let records = vec![
        TelemetryRecord::Log {
            timestamp: Utc::now(),
            level: sim_telemetry::LogLevel::Info,
            message: "Test log message".to_string(),
            attributes: attributes.clone(),
        },
        TelemetryRecord::Trace {
            trace_id: "trace-abc123".to_string(),
            span_id: "span-001".to_string(),
            parent_span_id: None,
            name: "test.operation".to_string(),
            start_time: Utc::now(),
            end_time: Utc::now(),
            attributes: attributes.clone(),
        },
        TelemetryRecord::Metric {
            timestamp: Utc::now(),
            name: "test.metric".to_string(),
            value: 42.0,
            metric_type: sim_telemetry::MetricType::Counter,
            tags: HashMap::new(),
        },
    ];

    // Export to OTLP and verify structure
    let otlp_logs = OtlpExporter::export_logs(&records).unwrap();
    assert!(otlp_logs.is_object());
    assert!(otlp_logs["resourceLogs"].is_array());
    assert!(otlp_logs["resourceLogs"][0]["scopeLogs"].is_array());
    assert!(otlp_logs["resourceLogs"][0]["scopeLogs"][0]["logRecords"].is_array());

    let otlp_traces = OtlpExporter::export_traces(&records).unwrap();
    assert!(otlp_traces.is_object());
    assert!(otlp_traces["resourceSpans"].is_array());

    let otlp_metrics = OtlpExporter::export_metrics(&records).unwrap();
    assert!(otlp_metrics.is_object());
    assert!(otlp_metrics["resourceMetrics"].is_array());
}

#[test]
fn test_prometheus_format_compliance() {
    use sim_telemetry::core::MetricType;

    let mut tags = HashMap::new();
    tags.insert("host".to_string(), "server-01".to_string());
    tags.insert("env".to_string(), "production".to_string());

    let records = vec![
        TelemetryRecord::Metric {
            timestamp: Utc::now(),
            name: "http.requests.total".to_string(),
            value: 1234.0,
            metric_type: MetricType::Counter,
            tags: tags.clone(),
        },
        TelemetryRecord::Metric {
            timestamp: Utc::now(),
            name: "cpu.usage.percent".to_string(),
            value: 75.5,
            metric_type: MetricType::Gauge,
            tags: tags.clone(),
        },
        TelemetryRecord::Metric {
            timestamp: Utc::now(),
            name: "request.duration.seconds".to_string(),
            value: 0.250,
            metric_type: MetricType::Histogram,
            tags: HashMap::new(),
        },
    ];

    let prometheus = PrometheusExporter::export(&records).unwrap();

    // Verify Prometheus format
    assert!(prometheus.contains("# TYPE http_requests_total counter"));
    assert!(prometheus.contains("http_requests_total{"));
    assert!(prometheus.contains("host=\"server-01\""));
    assert!(prometheus.contains("env=\"production\""));

    assert!(prometheus.contains("# TYPE cpu_usage_percent gauge"));
    assert!(prometheus.contains("cpu_usage_percent{"));

    assert!(prometheus.contains("# TYPE request_duration_seconds histogram"));
    assert!(prometheus.contains("request_duration_seconds 0.25"));
}

#[test]
fn test_json_log_export_to_file() {
    use tempfile::tempdir;
    use std::fs;

    let mut log_gen = EventToLogGenerator::new();

    let events = vec![
        json!({
            "id": "evt-001",
            "type": "user_login",
            "source": "urn:sim:auth",
            "time": "2026-03-16T10:00:00Z",
            "data": {"user_id": "user-001"}
        }),
        json!({
            "id": "evt-002",
            "type": "user_logout",
            "source": "urn:sim:auth",
            "time": "2026-03-16T10:05:00Z",
            "data": {"user_id": "user-001"}
        }),
    ];

    let mut logs = Vec::new();
    for event in events {
        logs.extend(log_gen.generate(&event).unwrap());
    }

    // Export to file
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("logs.json");
    JsonLogExporter::export_to_file(&logs, &file_path).unwrap();

    // Verify file was created and contains expected content
    assert!(file_path.exists());
    let content = fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("user_login"));
    assert!(content.contains("user_logout"));
}

#[test]
fn test_metric_aggregation_patterns() {
    let mut metric_gen = EventToMetricGenerator::new();

    // Simulate a series of API requests
    for i in 0..100 {
        let event = json!({
            "type": "api_request",
            "time": "2026-03-16T10:00:00Z",
            "data": {
                "latency": 100.0 + (i as f64 * 0.5),
                "payload_size": 1024 + (i * 10)
            }
        });
        metric_gen.generate(&event).unwrap();
    }

    // Verify counter
    assert_eq!(metric_gen.get_counter("events.api_request"), Some(100));

    // Verify histogram has all values
    let latency_histogram = metric_gen.get_histogram("api_request.latency.histogram").unwrap();
    assert_eq!(latency_histogram.len(), 100);

    let size_histogram = metric_gen.get_histogram("api_request.payload_size.histogram").unwrap();
    assert_eq!(size_histogram.len(), 100);

    // Calculate some statistics
    let avg_latency: f64 = latency_histogram.iter().sum::<f64>() / latency_histogram.len() as f64;
    assert!(avg_latency > 100.0 && avg_latency < 200.0);
}

#[test]
fn test_export_empty_records() {
    let empty_records: Vec<TelemetryRecord> = vec![];

    // All exporters should handle empty records gracefully
    let prometheus = PrometheusExporter::export(&empty_records).unwrap();
    assert_eq!(prometheus, "");

    let json_logs = JsonLogExporter::export_to_string(&empty_records).unwrap();
    assert_eq!(json_logs.trim(), "[]");

    let otlp_logs = OtlpExporter::export_logs(&empty_records).unwrap();
    assert!(otlp_logs["resourceLogs"][0]["scopeLogs"][0]["logRecords"]
        .as_array()
        .unwrap()
        .is_empty());
}
