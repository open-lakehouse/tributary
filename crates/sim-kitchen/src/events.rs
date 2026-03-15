//! Event payloads for ghost kitchen domain

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sim_events::EventPayload;

/// Event emitted when a customer places an order
#[derive(Debug)]
pub struct OrderCreatedEvent {
    pub order_id: String,
    pub customer_id: String,
    pub items: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for OrderCreatedEvent {
    fn event_type(&self) -> &str {
        "io.kitchen.order.created"
    }

    fn source(&self) -> String {
        format!("/orders/{}", self.order_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.customer_id.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "order_id": self.order_id,
            "customer_id": self.customer_id,
            "items": self.items,
            "timestamp": self.timestamp.to_rfc3339(),
        })
    }
}

/// Event emitted when an order is assigned to a kitchen
#[derive(Debug)]
pub struct OrderAssignedEvent {
    pub order_id: String,
    pub kitchen_id: String,
    pub kitchen_name: String,
    pub estimated_completion: DateTime<Utc>,
}

impl EventPayload for OrderAssignedEvent {
    fn event_type(&self) -> &str {
        "io.kitchen.order.assigned"
    }

    fn source(&self) -> String {
        format!("/kitchens/{}", self.kitchen_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.order_id.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "order_id": self.order_id,
            "kitchen_id": self.kitchen_id,
            "kitchen_name": self.kitchen_name,
            "estimated_completion": self.estimated_completion.to_rfc3339(),
        })
    }
}

/// Event emitted when an order is completed
#[derive(Debug)]
pub struct OrderCompletedEvent {
    pub order_id: String,
    pub customer_id: String,
    pub prep_time_seconds: i64,
}

impl EventPayload for OrderCompletedEvent {
    fn event_type(&self) -> &str {
        "io.kitchen.order.completed"
    }

    fn source(&self) -> String {
        format!("/orders/{}", self.order_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.customer_id.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "order_id": self.order_id,
            "customer_id": self.customer_id,
            "prep_time_seconds": self.prep_time_seconds,
        })
    }
}

/// Event emitted when a customer starts eating
#[derive(Debug)]
pub struct CustomerEatingEvent {
    pub customer_id: String,
    pub order_id: String,
    pub finish_time: DateTime<Utc>,
}

impl EventPayload for CustomerEatingEvent {
    fn event_type(&self) -> &str {
        "io.kitchen.customer.eating"
    }

    fn source(&self) -> String {
        format!("/customers/{}", self.customer_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.order_id.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "customer_id": self.customer_id,
            "order_id": self.order_id,
            "finish_time": self.finish_time.to_rfc3339(),
        })
    }
}

/// Event emitted when a customer finishes and leaves
#[derive(Debug)]
pub struct CustomerDoneEvent {
    pub customer_id: String,
    pub total_time_seconds: i64,
}

impl EventPayload for CustomerDoneEvent {
    fn event_type(&self) -> &str {
        "io.kitchen.customer.done"
    }

    fn source(&self) -> String {
        format!("/customers/{}", self.customer_id)
    }

    fn to_json(&self) -> Value {
        json!({
            "customer_id": self.customer_id,
            "total_time_seconds": self.total_time_seconds,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_created_event() {
        let event = OrderCreatedEvent {
            order_id: "order-123".to_string(),
            customer_id: "customer-456".to_string(),
            items: vec!["burger".to_string()],
            timestamp: Utc::now(),
        };

        assert_eq!(event.event_type(), "io.kitchen.order.created");
        assert_eq!(event.source(), "/orders/order-123");
        assert_eq!(event.subject(), Some("customer-456".to_string()));

        let json = event.to_json();
        assert_eq!(json["order_id"], "order-123");
        assert_eq!(json["items"][0], "burger");
    }

    #[test]
    fn test_order_assigned_event() {
        let event = OrderAssignedEvent {
            order_id: "order-123".to_string(),
            kitchen_id: "kitchen-1".to_string(),
            kitchen_name: "Main Kitchen".to_string(),
            estimated_completion: Utc::now(),
        };

        assert_eq!(event.event_type(), "io.kitchen.order.assigned");
        assert_eq!(event.source(), "/kitchens/kitchen-1");
    }

    #[test]
    fn test_customer_done_event() {
        let event = CustomerDoneEvent {
            customer_id: "customer-456".to_string(),
            total_time_seconds: 1200,
        };

        assert_eq!(event.event_type(), "io.kitchen.customer.done");

        let json = event.to_json();
        assert_eq!(json["total_time_seconds"], 1200);
    }
}
