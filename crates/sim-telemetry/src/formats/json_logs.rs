//! JSON log file export format.
//!
//! This module provides simple JSON export for log records to files.

use crate::core::TelemetryRecord;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// JSON log exporter for writing log records to files.
///
/// Exports only log records from a telemetry record collection,
/// filtering out traces and metrics.
pub struct JsonLogExporter;

impl JsonLogExporter {
    /// Export log records to a JSON file.
    ///
    /// Only `TelemetryRecord::Log` variants are exported. Other record types
    /// are filtered out.
    ///
    /// # Arguments
    ///
    /// * `records` - Slice of telemetry records to export
    /// * `path` - File path where JSON should be written
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use sim_telemetry::formats::JsonLogExporter;
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
    /// JsonLogExporter::export_to_file(&records, "/tmp/logs.json").unwrap();
    /// ```
    pub fn export_to_file(
        records: &[TelemetryRecord],
        path: impl AsRef<Path>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let logs: Vec<&TelemetryRecord> = records
            .iter()
            .filter(|r| matches!(r, TelemetryRecord::Log { .. }))
            .collect();

        let json = serde_json::to_string_pretty(&logs)?;
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;

        Ok(())
    }

    /// Export log records to a JSON string.
    ///
    /// Similar to `export_to_file` but returns the JSON as a string
    /// instead of writing to a file.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_telemetry::formats::JsonLogExporter;
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
    /// let json = JsonLogExporter::export_to_string(&records).unwrap();
    /// assert!(json.contains("Test message"));
    /// ```
    pub fn export_to_string(
        records: &[TelemetryRecord],
    ) -> Result<String, Box<dyn std::error::Error>> {
        let logs: Vec<&TelemetryRecord> = records
            .iter()
            .filter(|r| matches!(r, TelemetryRecord::Log { .. }))
            .collect();

        Ok(serde_json::to_string_pretty(&logs)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::LogLevel;
    use chrono::Utc;
    use std::collections::HashMap;

    #[test]
    fn test_export_to_string() {
        let mut attributes = HashMap::new();
        attributes.insert("agent_id".to_string(), "agent-001".to_string());

        let records = vec![
            TelemetryRecord::Log {
                timestamp: Utc::now(),
                level: LogLevel::Info,
                message: "First log".to_string(),
                attributes: attributes.clone(),
            },
            TelemetryRecord::Log {
                timestamp: Utc::now(),
                level: LogLevel::Error,
                message: "Second log".to_string(),
                attributes,
            },
        ];

        let json = JsonLogExporter::export_to_string(&records).unwrap();

        // Verify JSON contains expected content
        assert!(json.contains("First log"));
        assert!(json.contains("Second log"));
        assert!(json.contains("agent-001"));
        assert!(json.contains("Info"));
        assert!(json.contains("Error"));
    }

    #[test]
    fn test_filters_non_log_records() {
        let records = vec![
            TelemetryRecord::Log {
                timestamp: Utc::now(),
                level: LogLevel::Info,
                message: "Log message".to_string(),
                attributes: HashMap::new(),
            },
            TelemetryRecord::Trace {
                trace_id: "trace1".to_string(),
                span_id: "span1".to_string(),
                parent_span_id: None,
                name: "operation".to_string(),
                start_time: Utc::now(),
                end_time: Utc::now(),
                attributes: HashMap::new(),
            },
            TelemetryRecord::Metric {
                timestamp: Utc::now(),
                name: "counter".to_string(),
                value: 1.0,
                metric_type: crate::core::MetricType::Counter,
                tags: HashMap::new(),
            },
        ];

        let json = JsonLogExporter::export_to_string(&records).unwrap();

        // Should only contain log record
        assert!(json.contains("Log message"));
        assert!(!json.contains("trace1"));
        assert!(!json.contains("counter"));
    }

    #[test]
    fn test_export_to_file() {
        use std::fs;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_logs.json");

        let records = vec![TelemetryRecord::Log {
            timestamp: Utc::now(),
            level: LogLevel::Debug,
            message: "File export test".to_string(),
            attributes: HashMap::new(),
        }];

        // Export to file
        JsonLogExporter::export_to_file(&records, &file_path).unwrap();

        // Read back and verify
        let contents = fs::read_to_string(&file_path).unwrap();
        assert!(contents.contains("File export test"));
        assert!(contents.contains("Debug"));
    }

    #[test]
    fn test_empty_records() {
        let records: Vec<TelemetryRecord> = vec![];
        let json = JsonLogExporter::export_to_string(&records).unwrap();

        // Should produce empty array
        assert_eq!(json.trim(), "[]");
    }

    #[test]
    fn test_json_structure() {
        let records = vec![TelemetryRecord::Log {
            timestamp: Utc::now(),
            level: LogLevel::Warn,
            message: "Warning message".to_string(),
            attributes: HashMap::new(),
        }];

        let json = JsonLogExporter::export_to_string(&records).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // Should be an array
        assert!(parsed.is_array());
        let array = parsed.as_array().unwrap();
        assert_eq!(array.len(), 1);

        // Check structure of first element
        let log = &array[0];
        assert_eq!(log["type"], "Log");
        assert!(log["timestamp"].is_string());
        assert_eq!(log["level"], "Warn");
        assert_eq!(log["message"], "Warning message");
        assert!(log["attributes"].is_object());
    }

    #[test]
    fn test_multiple_logs_with_attributes() {
        let mut attr1 = HashMap::new();
        attr1.insert("service".to_string(), "api".to_string());
        attr1.insert("request_id".to_string(), "req-001".to_string());

        let mut attr2 = HashMap::new();
        attr2.insert("service".to_string(), "database".to_string());
        attr2.insert("query_time".to_string(), "150ms".to_string());

        let records = vec![
            TelemetryRecord::Log {
                timestamp: Utc::now(),
                level: LogLevel::Info,
                message: "API request".to_string(),
                attributes: attr1,
            },
            TelemetryRecord::Log {
                timestamp: Utc::now(),
                level: LogLevel::Debug,
                message: "Database query".to_string(),
                attributes: attr2,
            },
        ];

        let json = JsonLogExporter::export_to_string(&records).unwrap();

        assert!(json.contains("API request"));
        assert!(json.contains("Database query"));
        assert!(json.contains("req-001"));
        assert!(json.contains("150ms"));
    }
}
