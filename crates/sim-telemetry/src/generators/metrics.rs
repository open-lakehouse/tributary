//! Metric generator for converting events into metrics.
//!
//! This module provides the `EventToMetricGenerator` which tracks counters,
//! gauges, and histograms from event streams.

use crate::core::{MetricType, TelemetryError, TelemetryGenerator, TelemetryRecord};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Generator that converts events into metrics (counters, gauges, histograms).
///
/// This generator maintains internal state to track:
/// - Counters: Monotonically increasing values (e.g., event counts)
/// - Gauges: Current values that can increase or decrease (e.g., current state)
/// - Histograms: Distributions of values over time (e.g., latencies)
///
/// # Example
///
/// ```rust
/// use sim_telemetry::core::TelemetryGenerator;
/// use sim_telemetry::generators::EventToMetricGenerator;
/// use serde_json::json;
///
/// let mut generator = EventToMetricGenerator::new();
///
/// let event = json!({
///     "type": "agent_ticked",
///     "time": "2026-03-16T10:00:00Z",
///     "data": {
///         "energy": 75.5,
///         "speed": 12.3
///     }
/// });
///
/// let metrics = generator.generate(&event).unwrap();
/// // Generates counter for event type, gauges for numeric fields
/// assert!(metrics.len() > 0);
/// ```
#[derive(Debug, Default)]
pub struct EventToMetricGenerator {
    counters: HashMap<String, u64>,
    gauges: HashMap<String, f64>,
    histograms: HashMap<String, Vec<f64>>,
}

impl EventToMetricGenerator {
    /// Create a new metric generator with empty state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment a counter and return its new value.
    fn increment_counter(&mut self, name: &str) -> u64 {
        let counter = self.counters.entry(name.to_string()).or_insert(0);
        *counter += 1;
        *counter
    }

    /// Update a gauge to a new value.
    fn update_gauge(&mut self, name: &str, value: f64) {
        self.gauges.insert(name.to_string(), value);
    }

    /// Record a value in a histogram.
    fn record_histogram(&mut self, name: &str, value: f64) {
        self.histograms
            .entry(name.to_string())
            .or_default()
            .push(value);
    }

    /// Get the current value of a counter.
    pub fn get_counter(&self, name: &str) -> Option<u64> {
        self.counters.get(name).copied()
    }

    /// Get the current value of a gauge.
    pub fn get_gauge(&self, name: &str) -> Option<f64> {
        self.gauges.get(name).copied()
    }

    /// Get all values recorded in a histogram.
    pub fn get_histogram(&self, name: &str) -> Option<&[f64]> {
        self.histograms.get(name).map(|v| v.as_slice())
    }
}

impl TelemetryGenerator for EventToMetricGenerator {
    fn generate(
        &mut self,
        event: &serde_json::Value,
    ) -> Result<Vec<TelemetryRecord>, TelemetryError> {
        let mut metrics = Vec::new();

        let event_type = event["type"]
            .as_str()
            .ok_or_else(|| TelemetryError::InvalidEvent("Missing type".into()))?;
        let time_str = event["time"]
            .as_str()
            .ok_or_else(|| TelemetryError::InvalidEvent("Missing time".into()))?;

        let timestamp = DateTime::parse_from_rfc3339(time_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        // Counter: event count by type
        let counter_name = format!("events.{}", event_type);
        let count = self.increment_counter(&counter_name);
        metrics.push(TelemetryRecord::Metric {
            timestamp,
            name: counter_name,
            value: count as f64,
            metric_type: MetricType::Counter,
            tags: HashMap::from([("type".to_string(), event_type.to_string())]),
        });

        // Extract numeric data fields for gauges/histograms
        if let Some(data) = event["data"].as_object() {
            for (key, value) in data {
                if let Some(num) = value.as_f64() {
                    // Gauge for current values
                    let gauge_name = format!("{}.{}", event_type, key);
                    self.update_gauge(&gauge_name, num);
                    metrics.push(TelemetryRecord::Metric {
                        timestamp,
                        name: gauge_name.clone(),
                        value: num,
                        metric_type: MetricType::Gauge,
                        tags: HashMap::new(),
                    });

                    // Histogram for distributions
                    let hist_name = format!("{}.{}.histogram", event_type, key);
                    self.record_histogram(&hist_name, num);
                }
            }
        }

        Ok(metrics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_counter_increments() {
        let mut generator = EventToMetricGenerator::new();

        let event1 = json!({
            "type": "agent_ticked",
            "time": "2026-03-16T10:00:00Z",
            "data": {}
        });

        let event2 = json!({
            "type": "agent_ticked",
            "time": "2026-03-16T10:00:01Z",
            "data": {}
        });

        let metrics1 = generator.generate(&event1).unwrap();
        let metrics2 = generator.generate(&event2).unwrap();

        // First event: counter = 1
        assert!(metrics1.iter().any(|m| {
            if let TelemetryRecord::Metric {
                name,
                value,
                metric_type,
                ..
            } = m
            {
                name == "events.agent_ticked"
                    && *value == 1.0
                    && *metric_type == MetricType::Counter
            } else {
                false
            }
        }));

        // Second event: counter = 2
        assert!(metrics2.iter().any(|m| {
            if let TelemetryRecord::Metric {
                name,
                value,
                metric_type,
                ..
            } = m
            {
                name == "events.agent_ticked"
                    && *value == 2.0
                    && *metric_type == MetricType::Counter
            } else {
                false
            }
        }));

        // Verify internal state
        assert_eq!(generator.get_counter("events.agent_ticked"), Some(2));
    }

    #[test]
    fn test_gauge_updates() {
        let mut generator = EventToMetricGenerator::new();

        let event = json!({
            "type": "status_check",
            "time": "2026-03-16T10:00:00Z",
            "data": {
                "temperature": 72.5,
                "pressure": 14.7
            }
        });

        let metrics = generator.generate(&event).unwrap();

        // Should have counter + 2 gauges
        assert_eq!(metrics.len(), 3);

        // Check temperature gauge
        assert!(metrics.iter().any(|m| {
            if let TelemetryRecord::Metric {
                name,
                value,
                metric_type,
                ..
            } = m
            {
                name == "status_check.temperature"
                    && *value == 72.5
                    && *metric_type == MetricType::Gauge
            } else {
                false
            }
        }));

        // Check pressure gauge
        assert!(metrics.iter().any(|m| {
            if let TelemetryRecord::Metric {
                name,
                value,
                metric_type,
                ..
            } = m
            {
                name == "status_check.pressure"
                    && *value == 14.7
                    && *metric_type == MetricType::Gauge
            } else {
                false
            }
        }));

        // Verify internal state
        assert_eq!(generator.get_gauge("status_check.temperature"), Some(72.5));
        assert_eq!(generator.get_gauge("status_check.pressure"), Some(14.7));
    }

    #[test]
    fn test_histogram_recording() {
        let mut generator = EventToMetricGenerator::new();

        let events = vec![
            json!({
                "type": "request",
                "time": "2026-03-16T10:00:00Z",
                "data": {"latency": 10.5}
            }),
            json!({
                "type": "request",
                "time": "2026-03-16T10:00:01Z",
                "data": {"latency": 15.2}
            }),
            json!({
                "type": "request",
                "time": "2026-03-16T10:00:02Z",
                "data": {"latency": 8.7}
            }),
        ];

        for event in events {
            generator.generate(&event).unwrap();
        }

        // Check histogram values
        let histogram = generator
            .get_histogram("request.latency.histogram")
            .unwrap();
        assert_eq!(histogram.len(), 3);
        assert_eq!(histogram[0], 10.5);
        assert_eq!(histogram[1], 15.2);
        assert_eq!(histogram[2], 8.7);
    }

    #[test]
    fn test_multiple_event_types() {
        let mut generator = EventToMetricGenerator::new();

        let event1 = json!({
            "type": "agent_created",
            "time": "2026-03-16T10:00:00Z",
            "data": {}
        });

        let event2 = json!({
            "type": "agent_destroyed",
            "time": "2026-03-16T10:00:01Z",
            "data": {}
        });

        generator.generate(&event1).unwrap();
        generator.generate(&event2).unwrap();

        // Check separate counters
        assert_eq!(generator.get_counter("events.agent_created"), Some(1));
        assert_eq!(generator.get_counter("events.agent_destroyed"), Some(1));
    }

    #[test]
    fn test_metric_tags() {
        let mut generator = EventToMetricGenerator::new();

        let event = json!({
            "type": "test_event",
            "time": "2026-03-16T10:00:00Z",
            "data": {}
        });

        let metrics = generator.generate(&event).unwrap();

        // Counter should have type tag
        if let TelemetryRecord::Metric { tags, name, .. } = &metrics[0] {
            assert_eq!(name, "events.test_event");
            assert_eq!(tags.get("type"), Some(&"test_event".to_string()));
        } else {
            panic!("Expected Metric record");
        }
    }

    #[test]
    fn test_missing_type_field() {
        let mut generator = EventToMetricGenerator::new();

        let event = json!({
            "time": "2026-03-16T10:00:00Z",
            "data": {}
        });

        let result = generator.generate(&event);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TelemetryError::InvalidEvent(_)
        ));
    }

    #[test]
    fn test_missing_time_field() {
        let mut generator = EventToMetricGenerator::new();

        let event = json!({
            "type": "test_event",
            "data": {}
        });

        let result = generator.generate(&event);
        assert!(result.is_err());
    }

    #[test]
    fn test_non_numeric_data_ignored() {
        let mut generator = EventToMetricGenerator::new();

        let event = json!({
            "type": "test_event",
            "time": "2026-03-16T10:00:00Z",
            "data": {
                "name": "test",
                "count": 42,
                "enabled": true
            }
        });

        let metrics = generator.generate(&event).unwrap();

        // Should have counter + 1 gauge (only "count" is numeric)
        assert_eq!(metrics.len(), 2);
        assert!(metrics.iter().any(|m| {
            if let TelemetryRecord::Metric { name, .. } = m {
                name == "test_event.count"
            } else {
                false
            }
        }));
    }
}
