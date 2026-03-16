//! OpenTelemetry Protocol (OTLP) export format.
//!
//! This module provides exporters for converting telemetry records into
//! OTLP-compatible JSON format for logs, traces, and metrics.

use crate::core::{LogLevel, MetricType, TelemetryRecord};
use serde_json::{json, Value};

/// OTLP exporter for telemetry records.
///
/// Exports telemetry data in OpenTelemetry Protocol format, which is
/// compatible with OpenTelemetry collectors and backends.
pub struct OtlpExporter;

impl OtlpExporter {
    /// Export log records to OTLP JSON format.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_telemetry::formats::OtlpExporter;
    /// use sim_telemetry::core::{TelemetryRecord, LogLevel};
    /// use chrono::Utc;
    /// use std::collections::HashMap;
    ///
    /// let records = vec![
    ///     TelemetryRecord::Log {
    ///         timestamp: Utc::now(),
    ///         level: LogLevel::Info,
    ///         message: "Test message".to_string(),
    ///         attributes: HashMap::new(),
    ///     }
    /// ];
    ///
    /// let otlp_json = OtlpExporter::export_logs(&records).unwrap();
    /// assert!(otlp_json.get("resourceLogs").is_some());
    /// ```
    pub fn export_logs(records: &[TelemetryRecord]) -> Result<Value, Box<dyn std::error::Error>> {
        let logs: Vec<Value> = records
            .iter()
            .filter_map(|r| {
                if let TelemetryRecord::Log {
                    timestamp,
                    level,
                    message,
                    attributes,
                } = r
                {
                    Some(json!({
                        "timeUnixNano": timestamp.timestamp_nanos_opt().unwrap_or(0).to_string(),
                        "severityNumber": severity_number(level),
                        "severityText": format!("{:?}", level),
                        "body": { "stringValue": message },
                        "attributes": attributes.iter().map(|(k, v)| {
                            json!({ "key": k, "value": { "stringValue": v } })
                        }).collect::<Vec<_>>()
                    }))
                } else {
                    None
                }
            })
            .collect();

        Ok(json!({
            "resourceLogs": [{
                "scopeLogs": [{
                    "logRecords": logs
                }]
            }]
        }))
    }

    /// Export trace records to OTLP JSON format.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_telemetry::formats::OtlpExporter;
    /// use sim_telemetry::core::TelemetryRecord;
    /// use chrono::Utc;
    /// use std::collections::HashMap;
    ///
    /// let records = vec![
    ///     TelemetryRecord::Trace {
    ///         trace_id: "abc123".to_string(),
    ///         span_id: "span001".to_string(),
    ///         parent_span_id: None,
    ///         name: "operation".to_string(),
    ///         start_time: Utc::now(),
    ///         end_time: Utc::now(),
    ///         attributes: HashMap::new(),
    ///     }
    /// ];
    ///
    /// let otlp_json = OtlpExporter::export_traces(&records).unwrap();
    /// assert!(otlp_json.get("resourceSpans").is_some());
    /// ```
    pub fn export_traces(
        records: &[TelemetryRecord],
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let spans: Vec<Value> = records
            .iter()
            .filter_map(|r| {
                if let TelemetryRecord::Trace {
                    trace_id,
                    span_id,
                    parent_span_id,
                    name,
                    start_time,
                    end_time,
                    attributes,
                } = r
                {
                    Some(json!({
                        "traceId": hex_encode_trace_id(trace_id),
                        "spanId": hex_encode_span_id(span_id),
                        "parentSpanId": parent_span_id.as_ref().map(|id| hex_encode_span_id(id)),
                        "name": name,
                        "startTimeUnixNano": start_time.timestamp_nanos_opt().unwrap_or(0).to_string(),
                        "endTimeUnixNano": end_time.timestamp_nanos_opt().unwrap_or(0).to_string(),
                        "attributes": attributes.iter().map(|(k, v)| {
                            json!({ "key": k, "value": { "stringValue": v } })
                        }).collect::<Vec<_>>()
                    }))
                } else {
                    None
                }
            })
            .collect();

        Ok(json!({
            "resourceSpans": [{
                "scopeSpans": [{
                    "spans": spans
                }]
            }]
        }))
    }

    /// Export metric records to OTLP JSON format.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_telemetry::formats::OtlpExporter;
    /// use sim_telemetry::core::{TelemetryRecord, MetricType};
    /// use chrono::Utc;
    /// use std::collections::HashMap;
    ///
    /// let records = vec![
    ///     TelemetryRecord::Metric {
    ///         timestamp: Utc::now(),
    ///         name: "cpu.usage".to_string(),
    ///         value: 75.5,
    ///         metric_type: MetricType::Gauge,
    ///         tags: HashMap::new(),
    ///     }
    /// ];
    ///
    /// let otlp_json = OtlpExporter::export_metrics(&records).unwrap();
    /// assert!(otlp_json.get("resourceMetrics").is_some());
    /// ```
    pub fn export_metrics(
        records: &[TelemetryRecord],
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let metrics: Vec<Value> = records
            .iter()
            .filter_map(|r| {
                if let TelemetryRecord::Metric {
                    timestamp,
                    name,
                    value,
                    metric_type,
                    tags,
                } = r
                {
                    let attributes: Vec<Value> = tags
                        .iter()
                        .map(|(k, v)| json!({ "key": k, "value": { "stringValue": v } }))
                        .collect();

                    let data_point = match metric_type {
                        MetricType::Counter => json!({
                            "asInt": *value as i64,
                            "timeUnixNano": timestamp.timestamp_nanos_opt().unwrap_or(0).to_string(),
                            "attributes": attributes
                        }),
                        MetricType::Gauge => json!({
                            "asDouble": value,
                            "timeUnixNano": timestamp.timestamp_nanos_opt().unwrap_or(0).to_string(),
                            "attributes": attributes
                        }),
                        MetricType::Histogram => json!({
                            "timeUnixNano": timestamp.timestamp_nanos_opt().unwrap_or(0).to_string(),
                            "count": 1,
                            "sum": value,
                            "attributes": attributes
                        }),
                    };

                    let metric = match metric_type {
                        MetricType::Counter => json!({
                            "name": name,
                            "sum": {
                                "dataPoints": [data_point],
                                "aggregationTemporality": 2, // CUMULATIVE
                                "isMonotonic": true
                            }
                        }),
                        MetricType::Gauge => json!({
                            "name": name,
                            "gauge": {
                                "dataPoints": [data_point]
                            }
                        }),
                        MetricType::Histogram => json!({
                            "name": name,
                            "histogram": {
                                "dataPoints": [data_point],
                                "aggregationTemporality": 2 // CUMULATIVE
                            }
                        }),
                    };

                    Some(metric)
                } else {
                    None
                }
            })
            .collect();

        Ok(json!({
            "resourceMetrics": [{
                "scopeMetrics": [{
                    "metrics": metrics
                }]
            }]
        }))
    }
}

/// Convert LogLevel to OTLP severity number.
fn severity_number(level: &LogLevel) -> u32 {
    match level {
        LogLevel::Trace => 1,
        LogLevel::Debug => 5,
        LogLevel::Info => 9,
        LogLevel::Warn => 13,
        LogLevel::Error => 17,
        LogLevel::Fatal => 21,
    }
}

/// Convert trace ID to 32-character hex string for OTLP.
fn hex_encode_trace_id(id: &str) -> String {
    // Convert UUID to 32-char hex for OTLP
    let cleaned = id.replace('-', "");
    let mut result = cleaned.chars().take(32).collect::<String>();
    // Pad with zeros if needed
    while result.len() < 32 {
        result.push('0');
    }
    result
}

/// Convert span ID to 16-character hex string for OTLP.
fn hex_encode_span_id(id: &str) -> String {
    // Convert UUID to 16-char hex for OTLP
    let cleaned = id.replace('-', "");
    let mut result = cleaned.chars().take(16).collect::<String>();
    // Pad with zeros if needed
    while result.len() < 16 {
        result.push('0');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::LogLevel;
    use chrono::Utc;
    use std::collections::HashMap;

    #[test]
    fn test_otlp_log_export() {
        let mut attributes = HashMap::new();
        attributes.insert("service".to_string(), "test-service".to_string());

        let records = vec![TelemetryRecord::Log {
            timestamp: Utc::now(),
            level: LogLevel::Info,
            message: "Test log message".to_string(),
            attributes,
        }];

        let result = OtlpExporter::export_logs(&records).unwrap();

        // Check structure
        assert!(result.get("resourceLogs").is_some());
        let resource_logs = result["resourceLogs"].as_array().unwrap();
        assert_eq!(resource_logs.len(), 1);

        let scope_logs = resource_logs[0]["scopeLogs"].as_array().unwrap();
        let log_records = scope_logs[0]["logRecords"].as_array().unwrap();
        assert_eq!(log_records.len(), 1);

        // Check log content
        let log = &log_records[0];
        assert_eq!(log["severityNumber"], 9); // Info = 9
        assert_eq!(log["severityText"], "Info");
        assert_eq!(log["body"]["stringValue"], "Test log message");
    }

    #[test]
    fn test_otlp_trace_export() {
        let mut attributes = HashMap::new();
        attributes.insert("http.method".to_string(), "GET".to_string());

        let records = vec![TelemetryRecord::Trace {
            trace_id: "abc123def456".to_string(),
            span_id: "span001".to_string(),
            parent_span_id: Some("parent001".to_string()),
            name: "http.request".to_string(),
            start_time: Utc::now(),
            end_time: Utc::now(),
            attributes,
        }];

        let result = OtlpExporter::export_traces(&records).unwrap();

        // Check structure
        assert!(result.get("resourceSpans").is_some());
        let resource_spans = result["resourceSpans"].as_array().unwrap();
        assert_eq!(resource_spans.len(), 1);

        let scope_spans = resource_spans[0]["scopeSpans"].as_array().unwrap();
        let spans = scope_spans[0]["spans"].as_array().unwrap();
        assert_eq!(spans.len(), 1);

        // Check span content
        let span = &spans[0];
        assert_eq!(span["name"], "http.request");
        assert!(span["traceId"].as_str().unwrap().len() == 32);
        assert!(span["spanId"].as_str().unwrap().len() == 16);
        assert!(span["parentSpanId"].is_string());
    }

    #[test]
    fn test_otlp_metric_export_counter() {
        let records = vec![TelemetryRecord::Metric {
            timestamp: Utc::now(),
            name: "requests.total".to_string(),
            value: 42.0,
            metric_type: MetricType::Counter,
            tags: HashMap::new(),
        }];

        let result = OtlpExporter::export_metrics(&records).unwrap();

        // Check structure
        assert!(result.get("resourceMetrics").is_some());
        let resource_metrics = result["resourceMetrics"].as_array().unwrap();
        assert_eq!(resource_metrics.len(), 1);

        let scope_metrics = resource_metrics[0]["scopeMetrics"].as_array().unwrap();
        let metrics = scope_metrics[0]["metrics"].as_array().unwrap();
        assert_eq!(metrics.len(), 1);

        // Check metric content
        let metric = &metrics[0];
        assert_eq!(metric["name"], "requests.total");
        assert!(metric["sum"].is_object());
        assert_eq!(metric["sum"]["isMonotonic"], true);
    }

    #[test]
    fn test_otlp_metric_export_gauge() {
        let mut tags = HashMap::new();
        tags.insert("host".to_string(), "server01".to_string());

        let records = vec![TelemetryRecord::Metric {
            timestamp: Utc::now(),
            name: "cpu.usage".to_string(),
            value: 75.5,
            metric_type: MetricType::Gauge,
            tags,
        }];

        let result = OtlpExporter::export_metrics(&records).unwrap();

        let metrics = result["resourceMetrics"][0]["scopeMetrics"][0]["metrics"]
            .as_array()
            .unwrap();
        let metric = &metrics[0];

        assert_eq!(metric["name"], "cpu.usage");
        assert!(metric["gauge"].is_object());
        let data_points = metric["gauge"]["dataPoints"].as_array().unwrap();
        assert_eq!(data_points[0]["asDouble"], 75.5);
    }

    #[test]
    fn test_severity_numbers() {
        assert_eq!(severity_number(&LogLevel::Trace), 1);
        assert_eq!(severity_number(&LogLevel::Debug), 5);
        assert_eq!(severity_number(&LogLevel::Info), 9);
        assert_eq!(severity_number(&LogLevel::Warn), 13);
        assert_eq!(severity_number(&LogLevel::Error), 17);
        assert_eq!(severity_number(&LogLevel::Fatal), 21);
    }

    #[test]
    fn test_hex_encode_trace_id() {
        let id = "abc-123-def-456";
        let encoded = hex_encode_trace_id(id);
        assert_eq!(encoded.len(), 32);
        assert!(!encoded.contains('-'));
    }

    #[test]
    fn test_hex_encode_span_id() {
        let id = "span-001-xyz";
        let encoded = hex_encode_span_id(id);
        assert_eq!(encoded.len(), 16);
        assert!(!encoded.contains('-'));
    }

    #[test]
    fn test_empty_records() {
        let records: Vec<TelemetryRecord> = vec![];

        let logs = OtlpExporter::export_logs(&records).unwrap();
        assert_eq!(logs["resourceLogs"][0]["scopeLogs"][0]["logRecords"].as_array().unwrap().len(), 0);

        let traces = OtlpExporter::export_traces(&records).unwrap();
        assert_eq!(traces["resourceSpans"][0]["scopeSpans"][0]["spans"].as_array().unwrap().len(), 0);

        let metrics = OtlpExporter::export_metrics(&records).unwrap();
        assert_eq!(metrics["resourceMetrics"][0]["scopeMetrics"][0]["metrics"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_mixed_records_filtering() {
        let records = vec![
            TelemetryRecord::Log {
                timestamp: Utc::now(),
                level: LogLevel::Info,
                message: "Log".to_string(),
                attributes: HashMap::new(),
            },
            TelemetryRecord::Trace {
                trace_id: "trace1".to_string(),
                span_id: "span1".to_string(),
                parent_span_id: None,
                name: "op".to_string(),
                start_time: Utc::now(),
                end_time: Utc::now(),
                attributes: HashMap::new(),
            },
            TelemetryRecord::Metric {
                timestamp: Utc::now(),
                name: "metric1".to_string(),
                value: 1.0,
                metric_type: MetricType::Counter,
                tags: HashMap::new(),
            },
        ];

        // Each exporter should filter to its type
        let logs = OtlpExporter::export_logs(&records).unwrap();
        assert_eq!(logs["resourceLogs"][0]["scopeLogs"][0]["logRecords"].as_array().unwrap().len(), 1);

        let traces = OtlpExporter::export_traces(&records).unwrap();
        assert_eq!(traces["resourceSpans"][0]["scopeSpans"][0]["spans"].as_array().unwrap().len(), 1);

        let metrics = OtlpExporter::export_metrics(&records).unwrap();
        assert_eq!(metrics["resourceMetrics"][0]["scopeMetrics"][0]["metrics"].as_array().unwrap().len(), 1);
    }
}
