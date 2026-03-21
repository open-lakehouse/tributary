//! Simulation telemetry generation and processing.
//!
//! This crate provides tools for generating OpenTelemetry-compatible observability
//! data (logs, traces, metrics) from simulation events.
//!
//! # Architecture
//!
//! The telemetry system is built around a few core concepts:
//!
//! - **Core Types**: Fundamental telemetry data structures (`TelemetryRecord`, `LogLevel`, etc.)
//! - **Generators**: Implementations that transform events into telemetry records
//! - **Extensibility**: The `TelemetryGenerator` trait allows custom implementations
//!
//! # Example Usage
//!
//! ```rust
//! use sim_telemetry::core::TelemetryGenerator;
//! use sim_telemetry::generators::EventToLogGenerator;
//! use serde_json::json;
//!
//! // Create a log generator
//! let mut generator = EventToLogGenerator::new();
//!
//! // Generate telemetry from an event
//! let event = json!({
//!     "id": "evt-123",
//!     "type": "com.simulation.agent_ticked",
//!     "source": "urn:sim:agent:agent-001",
//!     "time": "2026-03-16T10:00:00Z",
//!     "data": {
//!         "agent_id": "agent-001",
//!         "tick": 42
//!     }
//! });
//!
//! let records = generator.generate(&event).unwrap();
//! println!("Generated {} telemetry record(s)", records.len());
//! ```
//!
//! # Feature Flags
//!
//! Currently no feature flags are defined. Future versions may add:
//! - `otel-export`: Enable direct export to OpenTelemetry collectors
//! - `arrow-batch`: Batch processing with Apache Arrow

pub mod core;
pub mod formats;
pub mod generators;
pub mod processor;

// Re-export commonly used types for convenience
pub use core::{
    LogLevel, MetricType, TelemetryCollector, TelemetryError, TelemetryGenerator, TelemetryRecord,
};
pub use generators::{EventToLogGenerator, EventToMetricGenerator, EventToTraceGenerator};
pub use processor::TelemetryCollectorProcessor;

#[cfg(test)]
mod integration_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_end_to_end_log_generation() {
        let mut generator = EventToLogGenerator::new();

        let event = json!({
            "specversion": "1.0",
            "id": "integration-test-001",
            "source": "urn:sim:agent:agent-test",
            "type": "com.simulation.agent_ticked",
            "time": "2026-03-16T09:17:46.518945Z",
            "datacontenttype": "application/json",
            "data": {
                "agent_id": "agent-test",
                "tick": 42,
                "state": {
                    "position": {"x": 10.0, "y": 20.0},
                    "energy": 75.5
                }
            }
        });

        let records = generator.generate(&event).unwrap();
        assert_eq!(records.len(), 1);

        // Verify record structure
        match &records[0] {
            TelemetryRecord::Log {
                message,
                attributes,
                level,
                ..
            } => {
                assert!(message.contains("com.simulation.agent_ticked"));
                assert_eq!(attributes.get("event.id").unwrap(), "integration-test-001");
                assert_eq!(attributes.get("data.agent_id").unwrap(), "agent-test");
                assert_eq!(attributes.get("data.tick").unwrap(), "42");
                assert!(matches!(
                    level,
                    LogLevel::Trace
                        | LogLevel::Debug
                        | LogLevel::Info
                        | LogLevel::Warn
                        | LogLevel::Error
                        | LogLevel::Fatal
                ));
            }
            _ => panic!("Expected Log record"),
        }

        // Verify JSON serialization
        let json_output = serde_json::to_string_pretty(&records[0]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_output).unwrap();

        assert_eq!(parsed.get("type").and_then(|v| v.as_str()), Some("Log"));
        assert!(parsed.get("timestamp").is_some());
        assert!(parsed.get("level").is_some());
    }

    #[test]
    fn test_multiple_events_batch_processing() {
        let mut generator = EventToLogGenerator::with_probabilities(0.05, 0.10);

        let events = vec![
            json!({
                "id": "evt-001",
                "type": "com.simulation.agent_created",
                "source": "urn:sim:agent:agent-001",
                "time": "2026-03-16T10:00:00Z",
                "data": {"agent_id": "agent-001"}
            }),
            json!({
                "id": "evt-002",
                "type": "com.simulation.agent_ticked",
                "source": "urn:sim:agent:agent-001",
                "time": "2026-03-16T10:00:01Z",
                "data": {"agent_id": "agent-001", "tick": 1}
            }),
            json!({
                "id": "evt-003",
                "type": "com.simulation.agent_destroyed",
                "source": "urn:sim:agent:agent-001",
                "time": "2026-03-16T10:00:02Z",
                "data": {"agent_id": "agent-001"}
            }),
        ];

        let mut all_records = Vec::new();
        for event in events {
            let records = generator.generate(&event).unwrap();
            all_records.extend(records);
        }

        assert_eq!(all_records.len(), 3);

        // Verify each record is a log
        for record in all_records {
            assert!(matches!(record, TelemetryRecord::Log { .. }));
        }
    }
}
