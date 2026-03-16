//! Event payload trait

use serde_json::Value;
use std::fmt;

/// Domain-specific event payload
///
/// Implement this trait to define custom event types for your simulation.
/// The payload contains the actual data associated with an event.
///
/// # CloudEvents Compliance
///
/// This trait maps to CloudEvents fields:
/// - `event_type()` → CloudEvents `type`
/// - `source()` → CloudEvents `source`
/// - `data_content_type()` → CloudEvents `datacontenttype` (optional)
/// - `to_json()` → CloudEvents `data`
///
/// # Example
///
/// ```rust,ignore
/// use sim_events::EventPayload;
/// use serde_json::json;
///
/// struct OrderCreated {
///     order_id: String,
///     customer_id: String,
///     items: Vec<String>,
/// }
///
/// impl EventPayload for OrderCreated {
///     fn event_type(&self) -> &str {
///         "com.example.order.created"
///     }
///
///     fn source(&self) -> String {
///         format!("/orders/{}", self.order_id)
///     }
///
///     fn to_json(&self) -> serde_json::Value {
///         json!({
///             "order_id": self.order_id,
///             "customer_id": self.customer_id,
///             "items": self.items,
///         })
///     }
/// }
/// ```
pub trait EventPayload: Send + Sync + fmt::Debug {
    /// CloudEvents type field
    ///
    /// This should be a reverse-DNS string identifying the event type.
    /// Examples:
    /// - "com.example.order.created"
    /// - "io.simulation.agent.spawned"
    /// - "com.warehouse.shipment.dispatched"
    fn event_type(&self) -> &str;

    /// CloudEvents source field
    ///
    /// URI identifying the source of the event. Examples:
    /// - "/agents/customer-123"
    /// - "/orders/order-456"
    /// - "/locations/warehouse-789"
    fn source(&self) -> String;

    /// Convert payload to JSON for storage
    ///
    /// This is used to serialize the event data. The JSON will be stored
    /// in the CloudEvents `data` field.
    fn to_json(&self) -> Value;

    /// Optional data content type (CloudEvents datacontenttype)
    ///
    /// Defaults to "application/json". Override if using a different format.
    fn data_content_type(&self) -> Option<&str> {
        Some("application/json")
    }

    /// Optional subject (CloudEvents subject)
    ///
    /// Additional context about the event. For example, if source is "/orders"
    /// and an event is about order "123", the subject could be "123".
    fn subject(&self) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Debug)]
    struct TestPayload {
        id: String,
        value: i32,
    }

    impl EventPayload for TestPayload {
        fn event_type(&self) -> &str {
            "test.event"
        }

        fn source(&self) -> String {
            format!("/test/{}", self.id)
        }

        fn to_json(&self) -> Value {
            json!({
                "id": self.id,
                "value": self.value,
            })
        }
    }

    #[test]
    fn test_event_payload() {
        let payload = TestPayload {
            id: "test-123".to_string(),
            value: 42,
        };

        assert_eq!(payload.event_type(), "test.event");
        assert_eq!(payload.source(), "/test/test-123");
        assert_eq!(payload.data_content_type(), Some("application/json"));

        let json = payload.to_json();
        assert_eq!(json["id"], "test-123");
        assert_eq!(json["value"], 42);
    }
}
