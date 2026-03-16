//! Prometheus text format export.
//!
//! This module provides export functionality for metrics in Prometheus
//! text exposition format.

use crate::core::{MetricType, TelemetryRecord};

/// Prometheus format exporter for metrics.
///
/// Exports metric records in Prometheus text format, which can be scraped
/// by Prometheus servers or compatible monitoring systems.
pub struct PrometheusExporter;

impl PrometheusExporter {
    /// Export metric records to Prometheus text format.
    ///
    /// Converts metric records into Prometheus exposition format with proper
    /// metric names (replacing dots with underscores) and label formatting.
    ///
    /// # Format
    ///
    /// ```text
    /// # TYPE metric_name counter
    /// metric_name{label="value"} 42.0
    /// ```
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_telemetry::formats::PrometheusExporter;
    /// use sim_telemetry::core::{TelemetryRecord, MetricType};
    /// use chrono::Utc;
    /// use std::collections::HashMap;
    ///
    /// let mut tags = HashMap::new();
    /// tags.insert("host".to_string(), "server01".to_string());
    ///
    /// let records = vec![
    ///     TelemetryRecord::Metric {
    ///         timestamp: Utc::now(),
    ///         name: "http.requests.total".to_string(),
    ///         value: 100.0,
    ///         metric_type: MetricType::Counter,
    ///         tags,
    ///     }
    /// ];
    ///
    /// let output = PrometheusExporter::export(&records).unwrap();
    /// assert!(output.contains("http_requests_total"));
    /// assert!(output.contains("host=\"server01\""));
    /// ```
    pub fn export(records: &[TelemetryRecord]) -> Result<String, Box<dyn std::error::Error>> {
        let mut output = String::new();

        for record in records {
            if let TelemetryRecord::Metric {
                name,
                value,
                metric_type,
                tags,
                ..
            } = record
            {
                let metric_name = sanitize_metric_name(name);
                let metric_type_str = metric_type_to_prometheus(metric_type);

                // Add TYPE comment
                output.push_str(&format!("# TYPE {} {}\n", metric_name, metric_type_str));

                // Format labels
                let labels = if tags.is_empty() {
                    String::new()
                } else {
                    let label_pairs: Vec<String> = tags
                        .iter()
                        .map(|(k, v)| {
                            format!("{}=\"{}\"", sanitize_label_name(k), escape_label_value(v))
                        })
                        .collect();
                    format!("{{{}}}", label_pairs.join(","))
                };

                // Add metric line
                output.push_str(&format!("{}{} {}\n", metric_name, labels, value));
            }
        }

        Ok(output)
    }

    /// Export metrics without TYPE comments (for appending to existing output).
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_telemetry::formats::PrometheusExporter;
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
    /// let output = PrometheusExporter::export_without_type(&records).unwrap();
    /// assert!(output.contains("cpu_usage 75.5"));
    /// assert!(!output.contains("# TYPE"));
    /// ```
    pub fn export_without_type(
        records: &[TelemetryRecord],
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut output = String::new();

        for record in records {
            if let TelemetryRecord::Metric {
                name, value, tags, ..
            } = record
            {
                let metric_name = sanitize_metric_name(name);

                let labels = if tags.is_empty() {
                    String::new()
                } else {
                    let label_pairs: Vec<String> = tags
                        .iter()
                        .map(|(k, v)| {
                            format!("{}=\"{}\"", sanitize_label_name(k), escape_label_value(v))
                        })
                        .collect();
                    format!("{{{}}}", label_pairs.join(","))
                };

                output.push_str(&format!("{}{} {}\n", metric_name, labels, value));
            }
        }

        Ok(output)
    }
}

/// Sanitize metric name for Prometheus (replace dots with underscores).
fn sanitize_metric_name(name: &str) -> String {
    name.replace('.', "_")
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Sanitize label name for Prometheus.
fn sanitize_label_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Escape label value for Prometheus (escape backslashes, quotes, and newlines).
fn escape_label_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Convert MetricType to Prometheus type string.
fn metric_type_to_prometheus(metric_type: &MetricType) -> &'static str {
    match metric_type {
        MetricType::Counter => "counter",
        MetricType::Gauge => "gauge",
        MetricType::Histogram => "histogram",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    #[test]
    fn test_basic_counter() {
        let records = vec![TelemetryRecord::Metric {
            timestamp: Utc::now(),
            name: "requests.total".to_string(),
            value: 42.0,
            metric_type: MetricType::Counter,
            tags: HashMap::new(),
        }];

        let output = PrometheusExporter::export(&records).unwrap();

        assert!(output.contains("# TYPE requests_total counter"));
        assert!(output.contains("requests_total 42"));
    }

    #[test]
    fn test_gauge_with_tags() {
        let mut tags = HashMap::new();
        tags.insert("host".to_string(), "server01".to_string());
        tags.insert("region".to_string(), "us-west".to_string());

        let records = vec![TelemetryRecord::Metric {
            timestamp: Utc::now(),
            name: "cpu.usage".to_string(),
            value: 75.5,
            metric_type: MetricType::Gauge,
            tags,
        }];

        let output = PrometheusExporter::export(&records).unwrap();

        assert!(output.contains("# TYPE cpu_usage gauge"));
        assert!(output.contains("cpu_usage{"));
        assert!(output.contains("host=\"server01\""));
        assert!(output.contains("region=\"us-west\""));
        assert!(output.contains("} 75.5"));
    }

    #[test]
    fn test_histogram() {
        let records = vec![TelemetryRecord::Metric {
            timestamp: Utc::now(),
            name: "request.duration".to_string(),
            value: 0.250,
            metric_type: MetricType::Histogram,
            tags: HashMap::new(),
        }];

        let output = PrometheusExporter::export(&records).unwrap();

        assert!(output.contains("# TYPE request_duration histogram"));
        assert!(output.contains("request_duration 0.25"));
    }

    #[test]
    fn test_multiple_metrics() {
        let records = vec![
            TelemetryRecord::Metric {
                timestamp: Utc::now(),
                name: "counter1".to_string(),
                value: 10.0,
                metric_type: MetricType::Counter,
                tags: HashMap::new(),
            },
            TelemetryRecord::Metric {
                timestamp: Utc::now(),
                name: "gauge1".to_string(),
                value: 20.5,
                metric_type: MetricType::Gauge,
                tags: HashMap::new(),
            },
        ];

        let output = PrometheusExporter::export(&records).unwrap();

        assert!(output.contains("counter1 10"));
        assert!(output.contains("gauge1 20.5"));
    }

    #[test]
    fn test_sanitize_metric_name() {
        assert_eq!(
            sanitize_metric_name("http.requests.total"),
            "http_requests_total"
        );
        assert_eq!(sanitize_metric_name("cpu-usage"), "cpu_usage");
        assert_eq!(sanitize_metric_name("memory@used"), "memory_used");
    }

    #[test]
    fn test_escape_label_value() {
        assert_eq!(escape_label_value("simple"), "simple");
        assert_eq!(escape_label_value("with\"quote"), "with\\\"quote");
        assert_eq!(escape_label_value("with\\backslash"), "with\\\\backslash");
        assert_eq!(escape_label_value("with\nNewline"), "with\\nNewline");
    }

    #[test]
    fn test_export_without_type() {
        let records = vec![TelemetryRecord::Metric {
            timestamp: Utc::now(),
            name: "test.metric".to_string(),
            value: 123.0,
            metric_type: MetricType::Counter,
            tags: HashMap::new(),
        }];

        let output = PrometheusExporter::export_without_type(&records).unwrap();

        assert!(output.contains("test_metric 123"));
        assert!(!output.contains("# TYPE"));
    }

    #[test]
    fn test_empty_records() {
        let records: Vec<TelemetryRecord> = vec![];
        let output = PrometheusExporter::export(&records).unwrap();
        assert_eq!(output, "");
    }

    #[test]
    fn test_filters_non_metric_records() {
        let records = vec![
            TelemetryRecord::Log {
                timestamp: Utc::now(),
                level: crate::core::LogLevel::Info,
                message: "Log message".to_string(),
                attributes: HashMap::new(),
            },
            TelemetryRecord::Metric {
                timestamp: Utc::now(),
                name: "valid.metric".to_string(),
                value: 42.0,
                metric_type: MetricType::Counter,
                tags: HashMap::new(),
            },
        ];

        let output = PrometheusExporter::export(&records).unwrap();

        // Should only contain metric
        assert!(output.contains("valid_metric"));
        assert!(!output.contains("Log message"));
    }

    #[test]
    fn test_multiple_tags_ordering() {
        let mut tags = HashMap::new();
        tags.insert("z_tag".to_string(), "value_z".to_string());
        tags.insert("a_tag".to_string(), "value_a".to_string());
        tags.insert("m_tag".to_string(), "value_m".to_string());

        let records = vec![TelemetryRecord::Metric {
            timestamp: Utc::now(),
            name: "test".to_string(),
            value: 1.0,
            metric_type: MetricType::Counter,
            tags,
        }];

        let output = PrometheusExporter::export(&records).unwrap();

        // All tags should be present
        assert!(output.contains("a_tag=\"value_a\""));
        assert!(output.contains("m_tag=\"value_m\""));
        assert!(output.contains("z_tag=\"value_z\""));
    }

    #[test]
    fn test_metric_type_conversion() {
        assert_eq!(metric_type_to_prometheus(&MetricType::Counter), "counter");
        assert_eq!(metric_type_to_prometheus(&MetricType::Gauge), "gauge");
        assert_eq!(
            metric_type_to_prometheus(&MetricType::Histogram),
            "histogram"
        );
    }
}
