//! CloudEvents-compliant event wrapper

use super::payload::EventPayload;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sim_core::utils::idents::EventId;

/// A CloudEvents 1.0 compliant event
///
/// This wraps a domain-specific payload with CloudEvents metadata.
///
/// # Example
///
/// ```rust,ignore
/// use sim_events::{Event, EventPayload};
///
/// let event = Event::new(Box::new(my_payload));
/// println!("Event ID: {}", event.id());
/// println!("Event type: {}", event.event_type());
/// ```
#[derive(Debug)]
pub struct Event {
    /// Unique event identifier (CloudEvents id)
    id: EventId,
    /// Event type (CloudEvents type)
    event_type: String,
    /// Event source URI (CloudEvents source)
    source: String,
    /// CloudEvents spec version
    spec_version: String,
    /// When the event occurred (CloudEvents time)
    time: DateTime<Utc>,
    /// Optional subject (CloudEvents subject)
    subject: Option<String>,
    /// Data content type (CloudEvents datacontenttype)
    data_content_type: Option<String>,
    /// Event payload
    payload: Box<dyn EventPayload>,
}

impl Event {
    /// Create a new event from a payload
    ///
    /// # Arguments
    ///
    /// * `payload` - Event payload implementing EventPayload trait
    ///
    /// # Returns
    ///
    /// A new Event with CloudEvents metadata
    pub fn new(payload: Box<dyn EventPayload>) -> Self {
        Self {
            id: EventId::new(),
            event_type: payload.event_type().to_string(),
            source: payload.source(),
            spec_version: "1.0".to_string(),
            time: Utc::now(),
            subject: payload.subject(),
            data_content_type: payload.data_content_type().map(|s| s.to_string()),
            payload,
        }
    }

    /// Create an event with a specific timestamp
    pub fn with_time(mut self, time: DateTime<Utc>) -> Self {
        self.time = time;
        self
    }

    /// Get the event ID
    pub fn id(&self) -> EventId {
        self.id
    }

    /// Get the event type
    pub fn event_type(&self) -> &str {
        &self.event_type
    }

    /// Get the event source
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Get the CloudEvents spec version
    pub fn spec_version(&self) -> &str {
        &self.spec_version
    }

    /// Get the event time
    pub fn time(&self) -> DateTime<Utc> {
        self.time
    }

    /// Get the subject (if any)
    pub fn subject(&self) -> Option<&str> {
        self.subject.as_deref()
    }

    /// Get the data content type
    pub fn data_content_type(&self) -> Option<&str> {
        self.data_content_type.as_deref()
    }

    /// Get reference to the payload
    pub fn payload(&self) -> &dyn EventPayload {
        self.payload.as_ref()
    }

    /// Convert event to CloudEvents JSON representation
    pub fn to_cloudevents_json(&self) -> serde_json::Value {
        let mut obj = serde_json::json!({
            "id": self.id.to_string(),
            "source": self.source,
            "specversion": self.spec_version,
            "type": self.event_type,
            "time": self.time.to_rfc3339(),
            "data": self.payload.to_json(),
        });

        if let Some(subject) = &self.subject {
            obj["subject"] = serde_json::Value::String(subject.clone());
        }

        if let Some(content_type) = &self.data_content_type {
            obj["datacontenttype"] = serde_json::Value::String(content_type.clone());
        }

        obj
    }
}

/// Serializable event record (for Arrow/Parquet)
///
/// This is a simplified representation for storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: String,
    pub source: String,
    pub spec_version: String,
    pub event_type: String,
    pub time: String,
    pub subject: Option<String>,
    pub data_content_type: Option<String>,
    pub data: serde_json::Value,
}

impl Event {
    /// Convert to a serializable record
    pub fn to_record(&self) -> EventRecord {
        EventRecord {
            id: self.id.to_string(),
            source: self.source.clone(),
            spec_version: self.spec_version.clone(),
            event_type: self.event_type.clone(),
            time: self.time.to_rfc3339(),
            subject: self.subject.clone(),
            data_content_type: self.data_content_type.clone(),
            data: self.payload.to_json(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Debug)]
    struct TestPayload {
        value: i32,
    }

    impl EventPayload for TestPayload {
        fn event_type(&self) -> &str {
            "test.event"
        }

        fn source(&self) -> String {
            "/test/source".to_string()
        }

        fn to_json(&self) -> serde_json::Value {
            json!({ "value": self.value })
        }
    }

    #[test]
    fn test_event_creation() {
        let payload = Box::new(TestPayload { value: 42 });
        let event = Event::new(payload);

        assert_eq!(event.event_type(), "test.event");
        assert_eq!(event.source(), "/test/source");
        assert_eq!(event.spec_version(), "1.0");
    }

    #[test]
    fn test_event_to_cloudevents_json() {
        let payload = Box::new(TestPayload { value: 42 });
        let event = Event::new(payload);

        let json = event.to_cloudevents_json();

        assert!(json["id"].is_string());
        assert_eq!(json["source"], "/test/source");
        assert_eq!(json["specversion"], "1.0");
        assert_eq!(json["type"], "test.event");
        assert!(json["time"].is_string());
        assert_eq!(json["data"]["value"], 42);
    }

    #[test]
    fn test_event_to_record() {
        let payload = Box::new(TestPayload { value: 42 });
        let event = Event::new(payload);

        let record = event.to_record();

        assert_eq!(record.event_type, "test.event");
        assert_eq!(record.source, "/test/source");
        assert_eq!(record.spec_version, "1.0");
        assert_eq!(record.data["value"], 42);
    }

    #[test]
    fn test_event_with_time() {
        let payload = Box::new(TestPayload { value: 42 });
        let custom_time = Utc::now();
        let event = Event::new(payload).with_time(custom_time);

        assert_eq!(event.time(), custom_time);
    }
}
