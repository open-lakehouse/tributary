//! Event payloads for the ghost kitchen domain
//!
//! Order events (`io.kitchen.order.*`) use the order ID as CloudEvents
//! subject; customer events (`io.kitchen.customer.*`) use the customer ID.

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sim_events::EventPayload;

/// A customer placed an order
#[derive(Debug)]
pub struct OrderCreatedEvent {
    pub order_id: String,
    pub customer_id: String,
    pub items: Vec<String>,
    /// Menu price of the items, before tax and tip
    pub total_price: f64,
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
        Some(self.order_id.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "order_id": self.order_id,
            "customer_id": self.customer_id,
            "items": self.items,
            "total_price": self.total_price,
            "timestamp": self.timestamp.to_rfc3339(),
        })
    }
}

/// A kitchen started preparing an order
#[derive(Debug)]
pub struct OrderAssignedEvent {
    pub order_id: String,
    pub kitchen_id: String,
    pub kitchen_name: String,
    /// When preparation will finish if no step has to queue
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

/// A kitchen finished preparing an order
#[derive(Debug)]
pub struct OrderCompletedEvent {
    pub order_id: String,
    pub customer_id: String,
    pub kitchen_id: String,
    /// Seconds from order creation to completion (queueing included)
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
        Some(self.order_id.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "order_id": self.order_id,
            "customer_id": self.customer_id,
            "kitchen_id": self.kitchen_id,
            "prep_time_seconds": self.prep_time_seconds,
        })
    }
}

/// A customer received their order and started eating
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
        Some(self.customer_id.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "customer_id": self.customer_id,
            "order_id": self.order_id,
            "finish_time": self.finish_time.to_rfc3339(),
        })
    }
}

/// A customer finished eating
#[derive(Debug)]
pub struct CustomerDoneEvent {
    pub customer_id: String,
    pub order_id: String,
    pub total_price: f64,
    /// Seconds from placing the order to finishing the meal
    pub total_time_seconds: i64,
}

impl EventPayload for CustomerDoneEvent {
    fn event_type(&self) -> &str {
        "io.kitchen.customer.done"
    }

    fn source(&self) -> String {
        format!("/customers/{}", self.customer_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.customer_id.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "customer_id": self.customer_id,
            "order_id": self.order_id,
            "total_price": self.total_price,
            "total_time_seconds": self.total_time_seconds,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_events_use_order_subject() {
        let created = OrderCreatedEvent {
            order_id: "o1".into(),
            customer_id: "c1".into(),
            items: vec!["burger".into()],
            total_price: 12.5,
            timestamp: DateTime::UNIX_EPOCH,
        };
        assert_eq!(created.subject().as_deref(), Some("o1"));
        assert_eq!(created.to_json()["total_price"], 12.5);

        let done = CustomerDoneEvent {
            customer_id: "c1".into(),
            order_id: "o1".into(),
            total_price: 12.5,
            total_time_seconds: 900,
        };
        assert_eq!(done.subject().as_deref(), Some("c1"));
        assert_eq!(done.to_json()["order_id"], "o1");
    }
}
