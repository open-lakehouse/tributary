//! Log generator for converting events to structured log records.
//!
//! This module provides `EventToLogGenerator` which transforms CloudEvents
//! into OpenTelemetry-compatible log records with probabilistic severity levels.
//!
//! # Example
//!
//! ```rust
//! use sim_telemetry::generators::EventToLogGenerator;
//! use sim_telemetry::core::TelemetryGenerator;
//! use serde_json::json;
//!
//! let mut generator = EventToLogGenerator::new();
//! let event = json!({
//!     "id": "evt-123",
//!     "type": "com.simulation.agent_ticked",
//!     "source": "urn:sim:agent:agent-001",
//!     "time": "2026-03-16T09:17:46Z",
//!     "data": {"agent_id": "agent-001", "tick": 42}
//! });
//!
//! let records = generator.generate(&event).unwrap();
//! assert_eq!(records.len(), 1);
//! ```

use crate::core::{LogLevel, TelemetryError, TelemetryGenerator, TelemetryRecord};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Generator that converts events into log records with probabilistic severity.
///
/// This generator extracts key fields from CloudEvents and creates structured
/// log records. The severity level is determined probabilistically based on
/// configured probabilities for error and trace levels.
///
/// # Log Level Selection
///
/// The generator uses a weighted random selection:
/// - `error_probability`: Chance of generating Error level logs
/// - `trace_probability`: Chance of generating Trace level logs
/// - Remaining probability is split between Debug, Info, and Warn
///
/// # Examples
///
/// ```rust
/// use sim_telemetry::generators::EventToLogGenerator;
///
/// // Default probabilities (5% error, 10% trace)
/// let generator = EventToLogGenerator::new();
///
/// // Custom probabilities
/// let generator = EventToLogGenerator::with_probabilities(0.10, 0.05);
/// ```
pub struct EventToLogGenerator {
    /// Probability of generating Error or Fatal level logs (0.0 to 1.0)
    pub error_probability: f64,
    /// Probability of generating Trace level logs (0.0 to 1.0)
    pub trace_probability: f64,
}

impl EventToLogGenerator {
    /// Create a new log generator with default probabilities.
    ///
    /// Default: 5% error, 10% trace
    pub fn new() -> Self {
        Self {
            error_probability: 0.05,
            trace_probability: 0.10,
        }
    }

    /// Create a log generator with custom probabilities.
    ///
    /// # Arguments
    ///
    /// * `error_prob` - Probability of error/fatal logs (0.0 to 1.0)
    /// * `trace_prob` - Probability of trace logs (0.0 to 1.0)
    ///
    /// # Panics
    ///
    /// Panics if probabilities are not in valid range or sum exceeds 1.0
    pub fn with_probabilities(error_prob: f64, trace_prob: f64) -> Self {
        assert!(
            (0.0..=1.0).contains(&error_prob),
            "error_probability must be between 0.0 and 1.0"
        );
        assert!(
            (0.0..=1.0).contains(&trace_prob),
            "trace_probability must be between 0.0 and 1.0"
        );
        assert!(
            error_prob + trace_prob <= 1.0,
            "Combined probabilities cannot exceed 1.0"
        );

        Self {
            error_probability: error_prob,
            trace_probability: trace_prob,
        }
    }

    /// Select a log level based on configured probabilities and a random value.
    ///
    /// # Distribution
    ///
    /// Given random value r in [0.0, 1.0):
    /// - [0, error_prob): Error
    /// - [error_prob, error_prob + trace_prob): Trace
    /// - [error_prob + trace_prob, 1.0): Info, Debug, or Warn (evenly split)
    fn select_log_level(&self, random_value: f64) -> LogLevel {
        if random_value < self.error_probability {
            // Randomly choose Error or Fatal
            if random_value < self.error_probability / 2.0 {
                LogLevel::Fatal
            } else {
                LogLevel::Error
            }
        } else if random_value < self.error_probability + self.trace_probability {
            LogLevel::Trace
        } else {
            // Distribute remaining probability among Info, Debug, Warn
            let remaining = 1.0 - self.error_probability - self.trace_probability;
            let adjusted = (random_value - self.error_probability - self.trace_probability)
                / remaining;

            if adjusted < 0.33 {
                LogLevel::Debug
            } else if adjusted < 0.66 {
                LogLevel::Info
            } else {
                LogLevel::Warn
            }
        }
    }

    /// Extract attributes from the event's data field.
    fn extract_attributes(&self, event: &serde_json::Value) -> HashMap<String, String> {
        let mut attributes = HashMap::new();

        // Add event-level attributes
        if let Some(id) = event.get("id").and_then(|v| v.as_str()) {
            attributes.insert("event.id".to_string(), id.to_string());
        }
        if let Some(source) = event.get("source").and_then(|v| v.as_str()) {
            attributes.insert("event.source".to_string(), source.to_string());
        }
        if let Some(event_type) = event.get("type").and_then(|v| v.as_str()) {
            attributes.insert("event.type".to_string(), event_type.to_string());
        }

        // Extract data field attributes
        if let Some(data) = event.get("data").and_then(|v| v.as_object()) {
            for (key, value) in data {
                let value_str = match value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                        serde_json::to_string(value).unwrap_or_default()
                    }
                    serde_json::Value::Null => "null".to_string(),
                };
                attributes.insert(format!("data.{}", key), value_str);
            }
        }

        attributes
    }

    /// Build a human-readable log message from the event.
    fn build_message(&self, event: &serde_json::Value) -> String {
        let event_type = event
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let source = event
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // Extract key data fields for the message
        let mut details = Vec::new();
        if let Some(data) = event.get("data").and_then(|v| v.as_object()) {
            if let Some(agent_id) = data.get("agent_id").and_then(|v| v.as_str()) {
                details.push(format!("agent={}", agent_id));
            }
            if let Some(tick) = data.get("tick").and_then(|v| v.as_i64()) {
                details.push(format!("tick={}", tick));
            }
        }

        if details.is_empty() {
            format!("Event {} from {}", event_type, source)
        } else {
            format!("Event {} from {} ({})", event_type, source, details.join(", "))
        }
    }
}

impl Default for EventToLogGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetryGenerator for EventToLogGenerator {
    fn generate(
        &mut self,
        event: &serde_json::Value,
    ) -> Result<Vec<TelemetryRecord>, TelemetryError> {
        // Validate event structure
        if !event.is_object() {
            return Err(TelemetryError::InvalidEvent(
                "Event must be a JSON object".to_string(),
            ));
        }

        // Extract timestamp
        let timestamp = if let Some(time_str) = event.get("time").and_then(|v| v.as_str()) {
            DateTime::parse_from_rfc3339(time_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now())
        } else {
            Utc::now()
        };

        // Select log level (use a deterministic hash for testing, random in production)
        let random_value = self.compute_deterministic_random(event);
        let level = self.select_log_level(random_value);

        // Build message and extract attributes
        let message = self.build_message(event);
        let attributes = self.extract_attributes(event);

        // Create log record
        let record = TelemetryRecord::Log {
            timestamp,
            level,
            message,
            attributes,
        };

        Ok(vec![record])
    }
}

impl EventToLogGenerator {
    /// Compute a deterministic "random" value from the event ID for consistent testing.
    ///
    /// This uses the event ID's hash to generate a consistent value in [0.0, 1.0).
    /// In production, you might want to use a real RNG instead.
    fn compute_deterministic_random(&self, event: &serde_json::Value) -> f64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let id = event
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        let hash = hasher.finish();

        // Convert hash to [0.0, 1.0)
        (hash % 1000) as f64 / 1000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;
    use serde_json::json;

    #[test]
    fn test_new_generator_default_probabilities() {
        let generator = EventToLogGenerator::new();
        assert_eq!(generator.error_probability, 0.05);
        assert_eq!(generator.trace_probability, 0.10);
    }

    #[test]
    fn test_custom_probabilities() {
        let generator = EventToLogGenerator::with_probabilities(0.15, 0.20);
        assert_eq!(generator.error_probability, 0.15);
        assert_eq!(generator.trace_probability, 0.20);
    }

    #[test]
    #[should_panic(expected = "error_probability must be between 0.0 and 1.0")]
    fn test_invalid_error_probability() {
        EventToLogGenerator::with_probabilities(1.5, 0.10);
    }

    #[test]
    #[should_panic(expected = "Combined probabilities cannot exceed 1.0")]
    fn test_probabilities_exceed_one() {
        EventToLogGenerator::with_probabilities(0.6, 0.5);
    }

    #[test]
    fn test_log_level_selection_error_range() {
        let generator = EventToLogGenerator::with_probabilities(0.10, 0.20);
        let level = generator.select_log_level(0.05);
        assert!(matches!(level, LogLevel::Error | LogLevel::Fatal));
    }

    #[test]
    fn test_log_level_selection_trace_range() {
        let generator = EventToLogGenerator::with_probabilities(0.10, 0.20);
        let level = generator.select_log_level(0.15);
        assert_eq!(level, LogLevel::Trace);
    }

    #[test]
    fn test_log_level_selection_normal_range() {
        let generator = EventToLogGenerator::with_probabilities(0.10, 0.20);
        let level = generator.select_log_level(0.80);
        assert!(matches!(
            level,
            LogLevel::Debug | LogLevel::Info | LogLevel::Warn
        ));
    }

    #[test]
    fn test_extract_attributes_basic() {
        let generator = EventToLogGenerator::new();
        let event = json!({
            "id": "evt-123",
            "type": "com.test.event",
            "source": "urn:test:source",
            "data": {
                "agent_id": "agent-001",
                "tick": 42
            }
        });

        let attrs = generator.extract_attributes(&event);
        assert_eq!(attrs.get("event.id").unwrap(), "evt-123");
        assert_eq!(attrs.get("event.type").unwrap(), "com.test.event");
        assert_eq!(attrs.get("event.source").unwrap(), "urn:test:source");
        assert_eq!(attrs.get("data.agent_id").unwrap(), "agent-001");
        assert_eq!(attrs.get("data.tick").unwrap(), "42");
    }

    #[test]
    fn test_extract_attributes_complex_types() {
        let generator = EventToLogGenerator::new();
        let event = json!({
            "id": "evt-456",
            "data": {
                "flag": true,
                "count": 3.14,
                "nested": {"x": 1, "y": 2}
            }
        });

        let attrs = generator.extract_attributes(&event);
        assert_eq!(attrs.get("data.flag").unwrap(), "true");
        assert_eq!(attrs.get("data.count").unwrap(), "3.14");
        assert!(attrs.get("data.nested").unwrap().contains("\"x\""));
    }

    #[test]
    fn test_build_message_with_agent_info() {
        let generator = EventToLogGenerator::new();
        let event = json!({
            "type": "com.simulation.agent_ticked",
            "source": "urn:sim:agent:agent-007",
            "data": {
                "agent_id": "agent-007",
                "tick": 99
            }
        });

        let message = generator.build_message(&event);
        assert!(message.contains("com.simulation.agent_ticked"));
        assert!(message.contains("agent-007"));
        assert!(message.contains("tick=99"));
    }

    #[test]
    fn test_build_message_minimal() {
        let generator = EventToLogGenerator::new();
        let event = json!({
            "type": "test.event",
            "source": "test:source"
        });

        let message = generator.build_message(&event);
        assert!(message.contains("test.event"));
        assert!(message.contains("test:source"));
    }

    #[test]
    fn test_generate_creates_log_record() {
        let mut generator = EventToLogGenerator::new();
        let event = json!({
            "id": "evt-789",
            "type": "com.test.event",
            "source": "urn:test:source",
            "time": "2026-03-16T10:30:00Z",
            "data": {
                "agent_id": "agent-001"
            }
        });

        let records = generator.generate(&event).unwrap();
        assert_eq!(records.len(), 1);

        match &records[0] {
            TelemetryRecord::Log {
                message,
                attributes,
                ..
            } => {
                assert!(message.contains("com.test.event"));
                assert!(attributes.contains_key("event.id"));
                assert_eq!(attributes.get("data.agent_id").unwrap(), "agent-001");
            }
            _ => panic!("Expected Log record"),
        }
    }

    #[test]
    fn test_generate_invalid_event() {
        let mut generator = EventToLogGenerator::new();
        let event = json!("not an object");

        let result = generator.generate(&event);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TelemetryError::InvalidEvent(_)));
    }

    #[test]
    fn test_generate_parses_rfc3339_timestamp() {
        let mut generator = EventToLogGenerator::new();
        let event = json!({
            "id": "evt-001",
            "type": "test.event",
            "source": "test",
            "time": "2026-03-16T09:17:46.518945Z"
        });

        let records = generator.generate(&event).unwrap();
        match &records[0] {
            TelemetryRecord::Log { timestamp, .. } => {
                assert_eq!(timestamp.year(), 2026);
                assert_eq!(timestamp.month(), 3);
                assert_eq!(timestamp.day(), 16);
            }
            _ => panic!("Expected Log record"),
        }
    }

    #[test]
    fn test_deterministic_random_consistent() {
        let generator = EventToLogGenerator::new();
        let event = json!({"id": "test-id"});

        let val1 = generator.compute_deterministic_random(&event);
        let val2 = generator.compute_deterministic_random(&event);

        assert_eq!(val1, val2);
        assert!(val1 >= 0.0 && val1 < 1.0);
    }

    #[test]
    fn test_json_serialization_valid_format() {
        let mut generator = EventToLogGenerator::new();
        let event = json!({
            "id": "evt-json-test",
            "type": "test.event",
            "source": "test:source",
            "time": "2026-03-16T10:00:00Z",
            "data": {"key": "value"}
        });

        let records = generator.generate(&event).unwrap();
        let json_output = serde_json::to_string_pretty(&records[0]).unwrap();

        // Validate it's valid JSON and contains expected fields
        let parsed: serde_json::Value = serde_json::from_str(&json_output).unwrap();
        assert_eq!(parsed.get("type").and_then(|v| v.as_str()), Some("Log"));
        assert!(parsed.get("timestamp").is_some());
        assert!(parsed.get("level").is_some());
        assert!(parsed.get("message").is_some());
        assert!(parsed.get("attributes").is_some());
    }

    #[test]
    fn test_probability_distribution_over_many_events() {
        let mut generator = EventToLogGenerator::with_probabilities(0.10, 0.20);
        let mut error_count = 0;
        let mut trace_count = 0;
        let mut other_count = 0;

        // Generate many events and count level distribution
        for i in 0..100 {
            let event = json!({
                "id": format!("evt-{}", i),
                "type": "test.event",
                "source": "test"
            });

            let records = generator.generate(&event).unwrap();
            match &records[0] {
                TelemetryRecord::Log { level, .. } => match level {
                    LogLevel::Error | LogLevel::Fatal => error_count += 1,
                    LogLevel::Trace => trace_count += 1,
                    _ => other_count += 1,
                },
                _ => panic!("Expected Log record"),
            }
        }

        // Check rough distribution (with tolerance for randomness)
        // We expect approximately: 10% error, 20% trace, 70% other
        assert!(error_count >= 5 && error_count <= 15, "Error count: {}", error_count);
        assert!(trace_count >= 15 && trace_count <= 25, "Trace count: {}", trace_count);
        assert!(other_count >= 65 && other_count <= 75, "Other count: {}", other_count);
    }
}
