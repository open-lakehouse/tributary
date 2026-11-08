//! Telemetry generators for transforming events into observability data.
//!
//! This module contains implementations of the `TelemetryGenerator` trait
//! for different types of telemetry output.

pub mod logs;
pub mod metrics;
pub mod traces;

pub use logs::EventToLogGenerator;
pub use metrics::EventToMetricGenerator;
pub use traces::EventToTraceGenerator;

use crate::core::TelemetryError;
use chrono::{DateTime, Utc};
use serde_json::Value;

/// The simulation time of an event, from its RFC 3339 `time` attribute
///
/// Every `sim_events::Event` carries a simulation time, so a missing or
/// unparsable `time` is an input error. It is never replaced with
/// wall-clock time.
pub(crate) fn event_time(event: &Value) -> Result<DateTime<Utc>, TelemetryError> {
    event
        .get("time")
        .and_then(Value::as_str)
        .and_then(|time| DateTime::parse_from_rfc3339(time).ok())
        .map(|time| time.with_timezone(&Utc))
        .ok_or_else(|| {
            let id = event.get("id").and_then(Value::as_str).unwrap_or("<no id>");
            TelemetryError::InvalidEvent(format!("event {id} has no valid time"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::TelemetryGenerator;
    use serde_json::json;

    fn generators() -> Vec<(&'static str, Box<dyn TelemetryGenerator>)> {
        vec![
            ("logs", Box::new(EventToLogGenerator::new())),
            ("metrics", Box::new(EventToMetricGenerator::new())),
            ("traces", Box::new(EventToTraceGenerator::new())),
        ]
    }

    #[test]
    fn missing_or_bad_time_is_an_error() {
        // Regression (#32): generators stamped wall-clock time instead.
        let missing = json!({ "id": "e1", "type": "t", "source": "s", "data": {} });
        let bad =
            json!({ "id": "e1", "type": "t", "source": "s", "time": "yesterday", "data": {} });
        for event in [missing, bad] {
            for (name, mut generator) in generators() {
                match generator.generate(&event) {
                    Err(TelemetryError::InvalidEvent(msg)) => {
                        assert_eq!(msg, "event e1 has no valid time", "{name}")
                    }
                    other => panic!("{name}: expected InvalidEvent, got {other:?}"),
                }
            }
        }
    }

    #[test]
    fn records_carry_the_event_time() {
        let time: DateTime<Utc> = "2020-06-01T12:00:00Z".parse().unwrap();
        let event = json!({
            "id": "e1", "type": "t", "source": "s", "time": "2020-06-01T12:00:00Z", "data": {}
        });
        for (name, mut generator) in generators() {
            let records = generator.generate(&event).unwrap();
            assert!(!records.is_empty(), "{name}");
            for record in records {
                let stamp = match record {
                    crate::TelemetryRecord::Log { timestamp, .. }
                    | crate::TelemetryRecord::Metric { timestamp, .. } => timestamp,
                    crate::TelemetryRecord::Trace { start_time, .. } => start_time,
                };
                assert_eq!(stamp, time, "{name}");
            }
        }
    }
}
