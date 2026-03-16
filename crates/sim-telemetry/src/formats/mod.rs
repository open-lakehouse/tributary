//! Export formats for telemetry data.
//!
//! This module provides various export formats for telemetry records:
//! - **OTLP**: OpenTelemetry Protocol JSON format
//! - **JSON Logs**: Simple JSON file export for logs
//! - **Prometheus**: Prometheus text exposition format for metrics
//!
//! # Example
//!
//! ```rust
//! use sim_telemetry::formats::{OtlpExporter, PrometheusExporter, JsonLogExporter};
//! use sim_telemetry::core::{TelemetryRecord, MetricType, LogLevel};
//! use chrono::Utc;
//! use std::collections::HashMap;
//!
//! // Create some telemetry records
//! let records = vec![
//!     TelemetryRecord::Log {
//!         timestamp: Utc::now(),
//!         level: LogLevel::Info,
//!         message: "Test log".to_string(),
//!         attributes: HashMap::new(),
//!     },
//!     TelemetryRecord::Metric {
//!         timestamp: Utc::now(),
//!         name: "cpu.usage".to_string(),
//!         value: 75.5,
//!         metric_type: MetricType::Gauge,
//!         tags: HashMap::new(),
//!     }
//! ];
//!
//! // Export to different formats
//! let otlp_logs = OtlpExporter::export_logs(&records).unwrap();
//! let otlp_metrics = OtlpExporter::export_metrics(&records).unwrap();
//! let prometheus = PrometheusExporter::export(&records).unwrap();
//! let json_logs = JsonLogExporter::export_to_string(&records).unwrap();
//! ```

pub mod json_logs;
pub mod otlp;
pub mod prometheus;

pub use json_logs::JsonLogExporter;
pub use otlp::OtlpExporter;
pub use prometheus::PrometheusExporter;
