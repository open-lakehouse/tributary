//! Kitchen resources for food preparation

use chrono::Duration;
use sim_core::{Resource, ResourceId, Task, TaskId};

/// Kitchen resource that prepares orders
#[derive(Debug, Clone)]
pub struct Kitchen {
    pub id: ResourceId,
    pub name: String,
    pub capacity: usize,
    pub current_load: usize,
}

impl Kitchen {
    /// Create a new kitchen
    pub fn new(name: String, capacity: usize) -> Self {
        Self {
            id: ResourceId::new(),
            name,
            capacity,
            current_load: 0,
        }
    }

    /// Assign an order to this kitchen
    pub fn assign_order(&mut self) {
        self.current_load += 1;
    }

    /// Complete an order
    pub fn complete_order(&mut self) {
        if self.current_load > 0 {
            self.current_load -= 1;
        }
    }

    /// Check if kitchen has capacity
    pub fn has_capacity(&self) -> bool {
        self.current_load < self.capacity
    }
}

impl Resource for Kitchen {
    fn id(&self) -> ResourceId {
        self.id
    }

    fn resource_type(&self) -> &str {
        "kitchen"
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn current_load(&self) -> usize {
        self.current_load
    }
}

/// Order task to be processed by a kitchen
#[derive(Debug, Clone)]
pub struct Order {
    pub id: TaskId,
    pub order_id: String,
    pub customer_id: String,
    pub items: Vec<String>,
    pub prep_time: Duration,
}

impl Order {
    /// Create a new order
    pub fn new(order_id: String, customer_id: String, items: Vec<String>) -> Self {
        // Calculate prep time based on items (simplified)
        let prep_time = Duration::minutes(5 + (items.len() as i64 * 3));

        Self {
            id: TaskId::new(),
            order_id,
            customer_id,
            items,
            prep_time,
        }
    }

    /// Create an order with custom prep time
    pub fn with_prep_time(
        order_id: String,
        customer_id: String,
        items: Vec<String>,
        prep_time: Duration,
    ) -> Self {
        Self {
            id: TaskId::new(),
            order_id,
            customer_id,
            items,
            prep_time,
        }
    }
}

impl Task for Order {
    fn id(&self) -> TaskId {
        self.id
    }

    fn required_resource_type(&self) -> &str {
        "kitchen"
    }

    fn estimated_duration(&self) -> Duration {
        self.prep_time
    }

    fn priority(&self) -> i32 {
        // Orders with more items have higher priority (simplified)
        self.items.len() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kitchen_creation() {
        let kitchen = Kitchen::new("Kitchen-1".to_string(), 5);

        assert_eq!(kitchen.resource_type(), "kitchen");
        assert_eq!(kitchen.capacity(), 5);
        assert_eq!(kitchen.current_load(), 0);
        assert!(kitchen.has_capacity());
    }

    #[test]
    fn test_kitchen_load_management() {
        let mut kitchen = Kitchen::new("Kitchen-1".to_string(), 2);

        assert!(kitchen.has_capacity());

        kitchen.assign_order();
        assert_eq!(kitchen.current_load(), 1);
        assert!(kitchen.has_capacity());

        kitchen.assign_order();
        assert_eq!(kitchen.current_load(), 2);
        assert!(!kitchen.has_capacity());

        kitchen.complete_order();
        assert_eq!(kitchen.current_load(), 1);
        assert!(kitchen.has_capacity());
    }

    #[test]
    fn test_order_creation() {
        let order = Order::new(
            "order-123".to_string(),
            "customer-456".to_string(),
            vec!["burger".to_string(), "fries".to_string()],
        );

        assert_eq!(order.required_resource_type(), "kitchen");
        assert_eq!(order.priority(), 2); // 2 items
        assert_eq!(order.estimated_duration(), Duration::minutes(11)); // 5 + 2*3
    }

    #[test]
    fn test_order_with_custom_prep_time() {
        let order = Order::with_prep_time(
            "order-123".to_string(),
            "customer-456".to_string(),
            vec!["burger".to_string()],
            Duration::minutes(10),
        );

        assert_eq!(order.estimated_duration(), Duration::minutes(10));
    }
}
