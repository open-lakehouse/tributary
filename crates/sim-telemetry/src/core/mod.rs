//! Core telemetry abstractions for generating observability data.
//!
//! This module provides the foundational types for representing telemetry data
//! (logs, traces, metrics) and a trait for implementing custom telemetry generators.
//!
//! # Overview
//!
//! The telemetry system is built around three core concepts:
//!
//! - **TelemetryRecord**: An enum representing different types of telemetry data
//! - **TelemetryGenerator**: A trait for implementing custom generators that transform
//!   events into telemetry records
//! - **TelemetryError**: Error types for telemetry operations
//!
//! # Example
//!
//! ```rust
//! use sim_telemetry::core::{TelemetryGenerator, TelemetryRecord, LogLevel};
//! use serde_json::json;
//! use std::collections::HashMap;
//!
//! # struct MyGenerator;
//! # impl TelemetryGenerator for MyGenerator {
//! #     fn generate(&mut self, event: &serde_json::Value) -> Result<Vec<TelemetryRecord>, sim_telemetry::core::TelemetryError> {
//! #         Ok(vec![])
//! #     }
//! # }
//! // Implement a custom generator
//! let mut generator = MyGenerator;
//! let event = json!({"type": "test", "message": "hello"});
//! let records = generator.generate(&event).unwrap();
//! ```

pub mod collector;

pub use collector::TelemetryCollector;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A telemetry record that can represent logs, traces, or metrics.
///
/// This enum provides a unified representation for different types of observability
/// data. Each variant contains the fields necessary for that telemetry type.
///
/// # Examples
///
/// ```
/// use sim_telemetry::core::{TelemetryRecord, LogLevel};
/// use chrono::Utc;
/// use std::collections::HashMap;
///
/// let log = TelemetryRecord::Log {
///     timestamp: Utc::now(),
///     level: LogLevel::Info,
///     message: "Processing event".to_string(),
///     attributes: HashMap::new(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TelemetryRecord {
    /// A log record with timestamp, severity level, message, and attributes.
    Log {
        /// When the log was generated
        timestamp: DateTime<Utc>,
        /// Severity level of the log
        level: LogLevel,
        /// Human-readable log message
        message: String,
        /// Additional structured attributes (key-value pairs)
        attributes: HashMap<String, String>,
    },
    /// A distributed trace span with timing and relationship information.
    Trace {
        /// Unique identifier for the entire trace
        trace_id: String,
        /// Unique identifier for this span
        span_id: String,
        /// Parent span ID if this is a child span
        parent_span_id: Option<String>,
        /// Name of the operation being traced
        name: String,
        /// When the operation started
        start_time: DateTime<Utc>,
        /// When the operation completed
        end_time: DateTime<Utc>,
        /// Additional structured attributes (key-value pairs)
        attributes: HashMap<String, String>,
    },
    /// A metric measurement with value, type, and tags.
    Metric {
        /// When the metric was recorded
        timestamp: DateTime<Utc>,
        /// Metric name (e.g., "http.requests", "cpu.usage")
        name: String,
        /// Numeric value of the metric
        value: f64,
        /// Type of metric (counter, gauge, or histogram)
        metric_type: MetricType,
        /// Additional tags for metric dimensions
        tags: HashMap<String, String>,
    },
}

/// Log severity levels compatible with OpenTelemetry.
///
/// These levels represent increasing severity from Trace (most verbose)
/// to Fatal (most critical).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    /// Fine-grained debug information (most verbose)
    Trace,
    /// Debug information for troubleshooting
    Debug,
    /// Informational messages about normal operation
    Info,
    /// Warning messages about potential issues
    Warn,
    /// Error messages about failures
    Error,
    /// Critical errors that may cause system failure
    Fatal,
}

/// Types of metrics supported by the telemetry system.
///
/// These align with common metric types in observability systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType {
    /// A monotonically increasing counter (e.g., total requests)
    Counter,
    /// A value that can increase or decrease (e.g., memory usage)
    Gauge,
    /// A distribution of values (e.g., request latencies)
    Histogram,
}

/// Trait for implementing telemetry generators.
///
/// A telemetry generator transforms generic events (represented as JSON values)
/// into structured telemetry records (logs, traces, or metrics).
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow parallel processing of events.
///
/// # Example Implementation
///
/// ```rust
/// use sim_telemetry::core::{TelemetryGenerator, TelemetryRecord, TelemetryError, LogLevel};
/// use chrono::Utc;
/// use std::collections::HashMap;
///
/// struct SimpleLogGenerator;
///
/// impl TelemetryGenerator for SimpleLogGenerator {
///     fn generate(&mut self, event: &serde_json::Value) -> Result<Vec<TelemetryRecord>, TelemetryError> {
///         let message = event.get("message")
///             .and_then(|v| v.as_str())
///             .ok_or_else(|| TelemetryError::InvalidEvent("Missing message field".to_string()))?;
///
///         Ok(vec![TelemetryRecord::Log {
///             timestamp: Utc::now(),
///             level: LogLevel::Info,
///             message: message.to_string(),
///             attributes: HashMap::new(),
///         }])
///     }
/// }
/// ```
pub trait TelemetryGenerator: Send + Sync {
    /// Generate telemetry records from an event.
    ///
    /// # Arguments
    ///
    /// * `event` - A generic event representation as a JSON value
    ///
    /// # Returns
    ///
    /// A vector of telemetry records. Multiple records can be generated from
    /// a single event (e.g., both a log and a trace span).
    ///
    /// # Errors
    ///
    /// Returns `TelemetryError` if the event cannot be processed.
    fn generate(
        &mut self,
        event: &serde_json::Value,
    ) -> Result<Vec<TelemetryRecord>, TelemetryError>;
}

/// Errors that can occur during telemetry generation.
#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    /// Error serializing or deserializing telemetry data
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// The provided event data is invalid or missing required fields
    #[error("Invalid event data: {0}")]
    InvalidEvent(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_record_serialization() {
        let mut attributes = HashMap::new();
        attributes.insert("agent_id".to_string(), "agent-001".to_string());
        attributes.insert("tick".to_string(), "42".to_string());

        let record = TelemetryRecord::Log {
            timestamp: Utc::now(),
            level: LogLevel::Info,
            message: "Agent processed event".to_string(),
            attributes,
        };

        let json = serde_json::to_string(&record).expect("Failed to serialize log record");
        let deserialized: TelemetryRecord =
            serde_json::from_str(&json).expect("Failed to deserialize log record");

        match deserialized {
            TelemetryRecord::Log { level, message, .. } => {
                assert_eq!(level, LogLevel::Info);
                assert_eq!(message, "Agent processed event");
            }
            _ => panic!("Expected Log variant"),
        }
    }

    #[test]
    fn test_trace_record_serialization() {
        let mut attributes = HashMap::new();
        attributes.insert("service".to_string(), "sim-core".to_string());

        let record = TelemetryRecord::Trace {
            trace_id: "trace-123".to_string(),
            span_id: "span-456".to_string(),
            parent_span_id: Some("span-789".to_string()),
            name: "process_event".to_string(),
            start_time: Utc::now(),
            end_time: Utc::now(),
            attributes,
        };

        let json = serde_json::to_string(&record).expect("Failed to serialize trace record");
        let deserialized: TelemetryRecord =
            serde_json::from_str(&json).expect("Failed to deserialize trace record");

        match deserialized {
            TelemetryRecord::Trace {
                trace_id,
                span_id,
                name,
                parent_span_id,
                ..
            } => {
                assert_eq!(trace_id, "trace-123");
                assert_eq!(span_id, "span-456");
                assert_eq!(parent_span_id, Some("span-789".to_string()));
                assert_eq!(name, "process_event");
            }
            _ => panic!("Expected Trace variant"),
        }
    }

    #[test]
    fn test_metric_record_serialization() {
        let mut tags = HashMap::new();
        tags.insert("host".to_string(), "server-01".to_string());
        tags.insert("region".to_string(), "us-west".to_string());

        let record = TelemetryRecord::Metric {
            timestamp: Utc::now(),
            name: "cpu.usage".to_string(),
            value: 75.5,
            metric_type: MetricType::Gauge,
            tags,
        };

        let json = serde_json::to_string(&record).expect("Failed to serialize metric record");
        let deserialized: TelemetryRecord =
            serde_json::from_str(&json).expect("Failed to deserialize metric record");

        match deserialized {
            TelemetryRecord::Metric {
                name,
                value,
                metric_type,
                ..
            } => {
                assert_eq!(name, "cpu.usage");
                assert_eq!(value, 75.5);
                assert_eq!(metric_type, MetricType::Gauge);
            }
            _ => panic!("Expected Metric variant"),
        }
    }

    #[test]
    fn test_log_level_ordering() {
        assert_eq!(LogLevel::Trace, LogLevel::Trace);
        assert_ne!(LogLevel::Info, LogLevel::Error);
    }

    #[test]
    fn test_metric_types() {
        assert_eq!(MetricType::Counter, MetricType::Counter);
        assert_ne!(MetricType::Gauge, MetricType::Histogram);
    }

    #[test]
    fn test_telemetry_error_display() {
        let err = TelemetryError::InvalidEvent("Missing field 'id'".to_string());
        assert_eq!(err.to_string(), "Invalid event data: Missing field 'id'");
    }

    #[test]
    fn test_json_tag_discriminator() {
        let log = TelemetryRecord::Log {
            timestamp: Utc::now(),
            level: LogLevel::Info,
            message: "test".to_string(),
            attributes: HashMap::new(),
        };

        let json_value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&log).unwrap()).unwrap();
        assert_eq!(json_value.get("type").and_then(|v| v.as_str()), Some("Log"));
    }
}
