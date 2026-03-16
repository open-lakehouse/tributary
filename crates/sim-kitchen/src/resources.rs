//! Kitchen resources for food preparation

use chrono::Duration;
use sim_core::{Resource, ResourceId, Task, TaskId};

/// Kitchen resource that prepares orders
///
/// The kitchen's load (number of active orders) is automatically tracked by the
/// framework's ResourceAllocator. You don't need to manually increment/decrement load.
///
/// # Example
///
/// ```rust
/// use sim_kitchen::Kitchen;
///
/// let kitchen = Kitchen::new("Main Kitchen".to_string(), 5);
/// assert_eq!(kitchen.capacity(), 5);
/// assert!(kitchen.is_available());
/// ```
#[derive(Debug, Clone)]
pub struct Kitchen {
    pub id: ResourceId,
    pub name: String,
    pub capacity: usize,
}

impl Kitchen {
    /// Create a new kitchen with the specified capacity
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name for the kitchen
    /// * `capacity` - Maximum number of concurrent orders this kitchen can handle
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::Kitchen;
    ///
    /// let kitchen = Kitchen::new("Kitchen-1".to_string(), 5);
    /// assert_eq!(kitchen.name, "Kitchen-1");
    /// ```
    pub fn new(name: String, capacity: usize) -> Self {
        Self {
            id: ResourceId::new(),
            name,
            capacity,
        }
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
        // Load is tracked by the ResourceAllocator, not by the Kitchen itself.
        // This is called by the framework during task assignment.
        // For standalone Kitchen instances (e.g., in tests), return 0.
        0
    }
}

/// Order task to be processed by a kitchen
///
/// Orders are created by customers and assigned to kitchens by the ResourceAllocator.
/// The prep time is automatically calculated based on the number of items, or can be
/// specified explicitly.
///
/// # Lifecycle
///
/// 1. Customer creates order (transitions from Ordering -> WaitingForFood)
/// 2. Order is added to ResourceAllocator via `add_task()`
/// 3. ResourceAllocator assigns order to least-loaded kitchen
/// 4. Kitchen prepares order (duration = prep_time)
/// 5. Order is completed, customer starts eating
///
/// # Example
///
/// ```rust
/// use sim_kitchen::Order;
///
/// let order = Order::new(
///     "order-123".to_string(),
///     "customer-456".to_string(),
///     vec!["burger".to_string(), "fries".to_string()],
/// );
/// assert_eq!(order.required_resource_type(), "kitchen");
/// ```
#[derive(Debug, Clone)]
pub struct Order {
    pub id: TaskId,
    pub order_id: String,
    pub customer_id: String,
    pub items: Vec<String>,
    pub prep_time: Duration,
}

impl Order {
    /// Create a new order with automatic prep time calculation
    ///
    /// Prep time is calculated as: 5 minutes + (3 minutes per item)
    ///
    /// # Arguments
    ///
    /// * `order_id` - Unique identifier for this order
    /// * `customer_id` - ID of the customer who placed the order
    /// * `items` - List of items in the order
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::Order;
    ///
    /// let order = Order::new(
    ///     "order-1".to_string(),
    ///     "customer-1".to_string(),
    ///     vec!["burger".to_string()],
    /// );
    /// ```
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
    ///
    /// Use this when you want precise control over preparation duration.
    ///
    /// # Arguments
    ///
    /// * `order_id` - Unique identifier for this order
    /// * `customer_id` - ID of the customer who placed the order
    /// * `items` - List of items in the order
    /// * `prep_time` - Custom preparation duration
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
        // Load tracking is handled by ResourceAllocator, not Kitchen
        assert_eq!(kitchen.current_load(), 0);
        assert!(kitchen.is_available());
    }

    #[test]
    fn test_kitchen_resource_trait() {
        // Test that Kitchen properly implements the Resource trait
        let kitchen = Kitchen::new("Main Kitchen".to_string(), 10);

        assert_eq!(kitchen.resource_type(), "kitchen");
        assert_eq!(kitchen.capacity(), 10);
        assert!(kitchen.is_available());
        assert_eq!(kitchen.available_capacity(), 10);
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
