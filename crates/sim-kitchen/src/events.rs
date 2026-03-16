//! Event payloads for ghost kitchen domain

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
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

/// Event emitted when inventory is consumed during order preparation
#[derive(Debug)]
pub struct InventoryConsumedEvent {
    pub kitchen_id: String,
    pub order_id: String,
    pub items: std::collections::HashMap<String, u32>,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for InventoryConsumedEvent {
    fn event_type(&self) -> &str {
        "io.kitchen.inventory.consumed"
    }

    fn source(&self) -> String {
        format!("/kitchens/{}/inventory", self.kitchen_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.order_id.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "kitchen_id": self.kitchen_id,
            "order_id": self.order_id,
            "items": self.items,
            "timestamp": self.timestamp.to_rfc3339(),
        })
    }
}

/// Event emitted when there is insufficient inventory to fulfill an order
#[derive(Debug)]
pub struct InsufficientInventoryEvent {
    pub kitchen_id: String,
    pub order_id: String,
    pub error: String,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for InsufficientInventoryEvent {
    fn event_type(&self) -> &str {
        "io.kitchen.inventory.insufficient"
    }

    fn source(&self) -> String {
        format!("/kitchens/{}/inventory", self.kitchen_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.order_id.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "kitchen_id": self.kitchen_id,
            "order_id": self.order_id,
            "error": self.error,
            "timestamp": self.timestamp.to_rfc3339(),
        })
    }
}

/// Event emitted when inventory falls below reorder point
#[derive(Debug)]
pub struct InventoryLowEvent {
    pub kitchen_id: String,
    pub item_type: String,
    pub current_quantity: u32,
    pub reorder_point: u32,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for InventoryLowEvent {
    fn event_type(&self) -> &str {
        "io.kitchen.inventory.low"
    }

    fn source(&self) -> String {
        format!("/kitchens/{}/inventory", self.kitchen_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.item_type.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "kitchen_id": self.kitchen_id,
            "item_type": self.item_type,
            "current_quantity": self.current_quantity,
            "reorder_point": self.reorder_point,
            "timestamp": self.timestamp.to_rfc3339(),
        })
    }
}

/// Event emitted when a supply order is created for inventory replenishment
#[derive(Debug)]
pub struct SupplyOrderCreatedEvent {
    pub order_id: String,
    pub kitchen_id: String,
    pub item_type: String,
    pub quantity: u32,
    pub source_site_id: String,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for SupplyOrderCreatedEvent {
    fn event_type(&self) -> &str {
        "io.kitchen.supply.order.created"
    }

    fn source(&self) -> String {
        format!("/kitchens/{}/supply", self.kitchen_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.order_id.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "order_id": self.order_id,
            "kitchen_id": self.kitchen_id,
            "item_type": self.item_type,
            "quantity": self.quantity,
            "source_site_id": self.source_site_id,
            "timestamp": self.timestamp.to_rfc3339(),
        })
    }
}

/// Event emitted when inventory is replenished from supply delivery
#[derive(Debug)]
pub struct InventoryReplenishedEvent {
    pub kitchen_id: String,
    pub item_type: String,
    pub quantity: u32,
    pub new_quantity: u32,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for InventoryReplenishedEvent {
    fn event_type(&self) -> &str {
        "io.kitchen.inventory.replenished"
    }

    fn source(&self) -> String {
        format!("/kitchens/{}/inventory", self.kitchen_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.item_type.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "kitchen_id": self.kitchen_id,
            "item_type": self.item_type,
            "quantity": self.quantity,
            "new_quantity": self.new_quantity,
            "timestamp": self.timestamp.to_rfc3339(),
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

    #[test]
    fn test_inventory_consumed_event() {
        let mut items = std::collections::HashMap::new();
        items.insert("flour".to_string(), 2);
        items.insert("beef".to_string(), 1);

        let event = InventoryConsumedEvent {
            kitchen_id: "kitchen-1".to_string(),
            order_id: "order-123".to_string(),
            items: items.clone(),
            timestamp: Utc::now(),
        };

        assert_eq!(event.event_type(), "io.kitchen.inventory.consumed");
        assert_eq!(event.source(), "/kitchens/kitchen-1/inventory");
        assert_eq!(event.subject(), Some("order-123".to_string()));

        let json = event.to_json();
        assert_eq!(json["kitchen_id"], "kitchen-1");
        assert_eq!(json["order_id"], "order-123");
        assert_eq!(json["items"]["flour"], 2);
        assert_eq!(json["items"]["beef"], 1);
    }

    #[test]
    fn test_insufficient_inventory_event() {
        let event = InsufficientInventoryEvent {
            kitchen_id: "kitchen-1".to_string(),
            order_id: "order-123".to_string(),
            error: "Insufficient flour: requested 5, available 2".to_string(),
            timestamp: Utc::now(),
        };

        assert_eq!(event.event_type(), "io.kitchen.inventory.insufficient");
        assert_eq!(event.source(), "/kitchens/kitchen-1/inventory");
        assert_eq!(event.subject(), Some("order-123".to_string()));

        let json = event.to_json();
        assert_eq!(json["kitchen_id"], "kitchen-1");
        assert_eq!(json["order_id"], "order-123");
        assert!(json["error"].as_str().unwrap().contains("Insufficient"));
    }

    #[test]
    fn test_inventory_low_event() {
        let event = InventoryLowEvent {
            kitchen_id: "kitchen-1".to_string(),
            item_type: "flour".to_string(),
            current_quantity: 40,
            reorder_point: 50,
            timestamp: Utc::now(),
        };

        assert_eq!(event.event_type(), "io.kitchen.inventory.low");
        assert_eq!(event.source(), "/kitchens/kitchen-1/inventory");
        assert_eq!(event.subject(), Some("flour".to_string()));

        let json = event.to_json();
        assert_eq!(json["kitchen_id"], "kitchen-1");
        assert_eq!(json["item_type"], "flour");
        assert_eq!(json["current_quantity"], 40);
        assert_eq!(json["reorder_point"], 50);
    }

    #[test]
    fn test_supply_order_created_event() {
        let event = SupplyOrderCreatedEvent {
            order_id: "supply-order-456".to_string(),
            kitchen_id: "kitchen-1".to_string(),
            item_type: "flour".to_string(),
            quantity: 200,
            source_site_id: "dist-site-789".to_string(),
            timestamp: Utc::now(),
        };

        assert_eq!(event.event_type(), "io.kitchen.supply.order.created");
        assert_eq!(event.source(), "/kitchens/kitchen-1/supply");
        assert_eq!(event.subject(), Some("supply-order-456".to_string()));

        let json = event.to_json();
        assert_eq!(json["order_id"], "supply-order-456");
        assert_eq!(json["kitchen_id"], "kitchen-1");
        assert_eq!(json["item_type"], "flour");
        assert_eq!(json["quantity"], 200);
        assert_eq!(json["source_site_id"], "dist-site-789");
    }

    #[test]
    fn test_inventory_replenished_event() {
        let event = InventoryReplenishedEvent {
            kitchen_id: "kitchen-1".to_string(),
            item_type: "flour".to_string(),
            quantity: 200,
            new_quantity: 240,
            timestamp: Utc::now(),
        };

        assert_eq!(event.event_type(), "io.kitchen.inventory.replenished");
        assert_eq!(event.source(), "/kitchens/kitchen-1/inventory");
        assert_eq!(event.subject(), Some("flour".to_string()));

        let json = event.to_json();
        assert_eq!(json["kitchen_id"], "kitchen-1");
        assert_eq!(json["item_type"], "flour");
        assert_eq!(json["quantity"], 200);
        assert_eq!(json["new_quantity"], 240);
    }
}
