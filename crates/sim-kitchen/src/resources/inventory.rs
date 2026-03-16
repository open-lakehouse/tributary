//! Inventory management system for ghost kitchens
//!
//! This module provides thread-safe inventory tracking with consumption, replenishment,
//! and automatic reorder point detection for ghost kitchen operations.
//!
//! # Overview
//!
//! The inventory system tracks ingredients and supplies for ghost kitchens with the
//! following key features:
//!
//! - **Consumption tracking**: Decrease inventory during cooking operations
//! - **Replenishment**: Add inventory from supply chain deliveries
//! - **Low inventory detection**: Trigger reorder workflows automatically
//! - **Thread-safe operations**: Safe concurrent access via Arc<Mutex<T>>
//!
//! # Architecture
//!
//! ```text
//! Inventory (single item)
//! ├── item_type: String (e.g., "flour", "tomatoes")
//! ├── quantity: u32 (current stock)
//! ├── capacity: u32 (max storage)
//! ├── reorder_point: u32 (trigger threshold)
//! └── reorder_quantity: u32 (order amount)
//!
//! InventoryManager (kitchen-wide)
//! └── items: HashMap<String, Inventory>
//! ```
//!
//! # Example Usage
//!
//! ```rust
//! use sim_kitchen::resources::inventory::{Inventory, InventoryManager};
//!
//! // Single inventory item
//! let mut flour = Inventory::new("flour".to_string(), 100, 500, 50, 200);
//! assert!(flour.consume(10).is_ok());
//! assert_eq!(flour.quantity(), 90);
//! assert!(!flour.needs_reorder());
//!
//! // Kitchen-wide manager
//! let mut manager = InventoryManager::new();
//! manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));
//! manager.add_item(Inventory::new("sugar".to_string(), 40, 300, 50, 150));
//!
//! assert!(manager.check_availability("flour", 50));
//! assert_eq!(manager.get_quantity("flour"), Some(100));
//!
//! // Consume multiple items at once
//! let mut items = std::collections::HashMap::new();
//! items.insert("flour".to_string(), 20);
//! items.insert("sugar".to_string(), 5);
//! assert!(manager.consume_multiple(&items).is_ok());
//! ```
//!
//! # Integration with ResourceAllocator
//!
//! The inventory system integrates with the framework's ResourceAllocator through:
//!
//! 1. **Order workflow tasks**: When inventory is low, create supply order tasks
//! 2. **Supply delivery completion**: When supply tasks complete, call `replenish()`
//! 3. **Cooking consumption**: When food prep starts, call `consume()` for ingredients
//!
//! ## Integration Pattern
//!
//! ```rust,ignore
//! // In kitchen simulation hooks
//! impl SimulationHooks for KitchenHooks {
//!     fn on_resources_allocated(&mut self, ctx: &SimulationContext, assignments: &[Assignment]) {
//!         // When food order allocated to kitchen
//!         for assignment in assignments {
//!             let kitchen_id = assignment.resource_id;
//!             let order = get_order(assignment.task_id);
//!
//!             // Consume inventory for this order
//!             if let Some(manager) = self.inventory_managers.get_mut(&kitchen_id) {
//!                 let ingredients = calculate_ingredients(&order.items);
//!                 manager.consume_multiple(&ingredients)?;
//!
//!                 // Check if we need to reorder
//!                 for item in manager.items_needing_reorder() {
//!                     // Create supply order task for ResourceAllocator
//!                     let supply_task = SupplyOrderTask::new(kitchen_id, item);
//!                     allocator.add_task(supply_task)?;
//!                 }
//!             }
//!         }
//!     }
//!
//!     fn on_tasks_completed(&mut self, ctx: &SimulationContext, tasks: &[CompletedTask]) {
//!         // When supply delivery task completes
//!         for task in tasks {
//!             if let Some(supply_order) = task.as_supply_order() {
//!                 let kitchen_id = supply_order.kitchen_id;
//!                 let item = &supply_order.item_type;
//!                 let quantity = supply_order.quantity;
//!
//!                 // Replenish inventory
//!                 if let Some(manager) = self.inventory_managers.get_mut(&kitchen_id) {
//!                     manager.replenish_item(item, quantity)?;
//!                 }
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! # Event Emission
//!
//! The inventory system should emit CloudEvents for observability:
//!
//! - `io.kitchen.inventory.consumed` - When ingredients are used
//! - `io.kitchen.inventory.low` - When stock hits reorder point
//! - `io.kitchen.inventory.replenished` - When supplies arrive
//!
//! See Phase 9 plan for complete event payload specifications.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Result type for inventory operations
pub type Result<T> = std::result::Result<T, InventoryError>;

/// Errors that can occur during inventory operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryError {
    /// Attempted to consume more than available
    InsufficientQuantity {
        item: String,
        requested: u32,
        available: u32,
    },
    /// Attempted to add more than capacity allows
    CapacityExceeded {
        item: String,
        requested: u32,
        capacity: u32,
        current: u32,
    },
    /// Item not found in inventory
    ItemNotFound { item: String },
    /// Invalid quantity (e.g., zero or negative)
    InvalidQuantity { reason: String },
}

impl std::fmt::Display for InventoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InventoryError::InsufficientQuantity {
                item,
                requested,
                available,
            } => write!(
                f,
                "Insufficient inventory for '{}': requested {}, available {}",
                item, requested, available
            ),
            InventoryError::CapacityExceeded {
                item,
                requested,
                capacity,
                current,
            } => write!(
                f,
                "Capacity exceeded for '{}': requested {}, capacity {}, current {}",
                item, requested, capacity, current
            ),
            InventoryError::ItemNotFound { item } => {
                write!(f, "Item '{}' not found in inventory", item)
            }
            InventoryError::InvalidQuantity { reason } => {
                write!(f, "Invalid quantity: {}", reason)
            }
        }
    }
}

impl std::error::Error for InventoryError {}

/// Represents a single inventory item with capacity and reorder settings
///
/// # Thread Safety
///
/// Inventory instances are NOT thread-safe by themselves. For concurrent access,
/// wrap in Arc<Mutex<Inventory>> or use InventoryManager which handles locking.
///
/// # Example
///
/// ```rust
/// use sim_kitchen::resources::inventory::Inventory;
///
/// let mut flour = Inventory::new("flour".to_string(), 100, 500, 50, 200);
///
/// // Consume during cooking
/// assert!(flour.consume(30).is_ok());
/// assert_eq!(flour.quantity(), 70);
///
/// // Check if reorder needed
/// assert!(flour.consume(25).is_ok());
/// assert_eq!(flour.quantity(), 45);
/// assert!(flour.needs_reorder()); // Below reorder_point of 50
/// assert!(flour.is_low());
///
/// // Replenish when supply arrives
/// assert!(flour.replenish(200).is_ok());
/// assert_eq!(flour.quantity(), 245);
/// assert!(!flour.needs_reorder());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    /// Type/name of the item (e.g., "flour", "tomatoes", "cooking_oil")
    pub item_type: String,
    /// Current quantity in stock
    quantity: u32,
    /// Maximum storage capacity
    pub capacity: u32,
    /// Threshold below which reorder should be triggered
    pub reorder_point: u32,
    /// Amount to order when replenishing
    pub reorder_quantity: u32,
}

impl Inventory {
    /// Create a new inventory item
    ///
    /// # Arguments
    ///
    /// * `item_type` - Name/identifier for the item
    /// * `quantity` - Initial quantity in stock
    /// * `capacity` - Maximum storage capacity
    /// * `reorder_point` - Threshold for triggering reorders
    /// * `reorder_quantity` - Amount to order when replenishing
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::resources::inventory::Inventory;
    ///
    /// let flour = Inventory::new("flour".to_string(), 100, 500, 50, 200);
    /// assert_eq!(flour.quantity(), 100);
    /// assert_eq!(flour.capacity, 500);
    /// ```
    pub fn new(
        item_type: String,
        quantity: u32,
        capacity: u32,
        reorder_point: u32,
        reorder_quantity: u32,
    ) -> Self {
        Self {
            item_type,
            quantity,
            capacity,
            reorder_point,
            reorder_quantity,
        }
    }

    /// Get current quantity
    pub fn quantity(&self) -> u32 {
        self.quantity
    }

    /// Consume a specified amount from inventory
    ///
    /// # Arguments
    ///
    /// * `amount` - Quantity to consume
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Consumption successful
    /// * `Err(InsufficientQuantity)` - Not enough inventory available
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::resources::inventory::Inventory;
    ///
    /// let mut flour = Inventory::new("flour".to_string(), 100, 500, 50, 200);
    ///
    /// assert!(flour.consume(30).is_ok());
    /// assert_eq!(flour.quantity(), 70);
    ///
    /// // Attempting to consume more than available
    /// let result = flour.consume(100);
    /// assert!(result.is_err());
    /// assert_eq!(flour.quantity(), 70); // Quantity unchanged on error
    /// ```
    pub fn consume(&mut self, amount: u32) -> Result<()> {
        if amount == 0 {
            return Err(InventoryError::InvalidQuantity {
                reason: "Cannot consume zero quantity".to_string(),
            });
        }

        if self.quantity < amount {
            return Err(InventoryError::InsufficientQuantity {
                item: self.item_type.clone(),
                requested: amount,
                available: self.quantity,
            });
        }

        self.quantity -= amount;
        Ok(())
    }

    /// Replenish inventory by adding a specified amount
    ///
    /// # Arguments
    ///
    /// * `amount` - Quantity to add
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Replenishment successful
    /// * `Err(CapacityExceeded)` - Adding would exceed capacity
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::resources::inventory::Inventory;
    ///
    /// let mut flour = Inventory::new("flour".to_string(), 100, 500, 50, 200);
    ///
    /// assert!(flour.replenish(200).is_ok());
    /// assert_eq!(flour.quantity(), 300);
    ///
    /// // Attempting to exceed capacity
    /// let result = flour.replenish(250);
    /// assert!(result.is_err());
    /// assert_eq!(flour.quantity(), 300); // Quantity unchanged on error
    /// ```
    pub fn replenish(&mut self, amount: u32) -> Result<()> {
        if amount == 0 {
            return Err(InventoryError::InvalidQuantity {
                reason: "Cannot replenish zero quantity".to_string(),
            });
        }

        let new_quantity = self.quantity.checked_add(amount).ok_or_else(|| {
            InventoryError::InvalidQuantity {
                reason: "Quantity overflow".to_string(),
            }
        })?;

        if new_quantity > self.capacity {
            return Err(InventoryError::CapacityExceeded {
                item: self.item_type.clone(),
                requested: amount,
                capacity: self.capacity,
                current: self.quantity,
            });
        }

        self.quantity = new_quantity;
        Ok(())
    }

    /// Check if inventory is at or below reorder point
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::resources::inventory::Inventory;
    ///
    /// let mut flour = Inventory::new("flour".to_string(), 100, 500, 50, 200);
    /// assert!(!flour.is_low());
    ///
    /// flour.consume(55).unwrap();
    /// assert!(flour.is_low()); // 45 <= 50
    /// ```
    pub fn is_low(&self) -> bool {
        self.quantity <= self.reorder_point
    }

    /// Check if inventory needs reordering (same as is_low)
    ///
    /// Convenience method with clearer intent for reorder logic.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::resources::inventory::Inventory;
    ///
    /// let mut flour = Inventory::new("flour".to_string(), 55, 500, 50, 200);
    /// assert!(!flour.needs_reorder());
    ///
    /// flour.consume(10).unwrap();
    /// assert!(flour.needs_reorder()); // 45 <= 50
    /// ```
    pub fn needs_reorder(&self) -> bool {
        self.is_low()
    }

    /// Get the percentage of capacity currently filled
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::resources::inventory::Inventory;
    ///
    /// let flour = Inventory::new("flour".to_string(), 250, 500, 50, 200);
    /// assert_eq!(flour.capacity_percentage(), 50.0);
    /// ```
    pub fn capacity_percentage(&self) -> f64 {
        if self.capacity == 0 {
            0.0
        } else {
            (self.quantity as f64 / self.capacity as f64) * 100.0
        }
    }
}

/// Thread-safe manager for multiple inventory items
///
/// InventoryManager tracks all inventory items for a kitchen and provides
/// thread-safe operations for concurrent workflow access.
///
/// # Thread Safety
///
/// All operations are protected by an internal Mutex, making InventoryManager
/// safe to share across threads via Arc.
///
/// # Example
///
/// ```rust
/// use sim_kitchen::resources::inventory::{Inventory, InventoryManager};
/// use std::collections::HashMap;
///
/// let mut manager = InventoryManager::new();
///
/// // Add items
/// manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));
/// manager.add_item(Inventory::new("sugar".to_string(), 40, 300, 50, 150));
/// manager.add_item(Inventory::new("oil".to_string(), 60, 200, 30, 100));
///
/// // Check availability
/// assert!(manager.check_availability("flour", 50));
/// assert!(!manager.check_availability("flour", 150));
///
/// // Get quantity
/// assert_eq!(manager.get_quantity("sugar"), Some(40));
/// assert_eq!(manager.get_quantity("salt"), None);
///
/// // Consume multiple items
/// let mut recipe = HashMap::new();
/// recipe.insert("flour".to_string(), 20);
/// recipe.insert("sugar".to_string(), 10);
/// assert!(manager.consume_multiple(&recipe).is_ok());
///
/// // Check for low inventory
/// let low_items = manager.items_needing_reorder();
/// assert_eq!(low_items.len(), 1); // sugar at 30, below 50
/// ```
#[derive(Debug)]
pub struct InventoryManager {
    items: Arc<Mutex<HashMap<String, Inventory>>>,
}

impl InventoryManager {
    /// Create a new empty inventory manager
    pub fn new() -> Self {
        Self {
            items: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Add an inventory item to the manager
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::resources::inventory::{Inventory, InventoryManager};
    ///
    /// let mut manager = InventoryManager::new();
    /// manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));
    ///
    /// assert_eq!(manager.get_quantity("flour"), Some(100));
    /// ```
    pub fn add_item(&mut self, item: Inventory) {
        let mut items = self.items.lock().unwrap();
        items.insert(item.item_type.clone(), item);
    }

    /// Get current quantity for an item
    ///
    /// # Returns
    ///
    /// * `Some(quantity)` - If item exists
    /// * `None` - If item not found
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::resources::inventory::{Inventory, InventoryManager};
    ///
    /// let mut manager = InventoryManager::new();
    /// manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));
    ///
    /// assert_eq!(manager.get_quantity("flour"), Some(100));
    /// assert_eq!(manager.get_quantity("salt"), None);
    /// ```
    pub fn get_quantity(&self, item_type: &str) -> Option<u32> {
        let items = self.items.lock().unwrap();
        items.get(item_type).map(|inv| inv.quantity())
    }

    /// Check if sufficient quantity is available
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::resources::inventory::{Inventory, InventoryManager};
    ///
    /// let mut manager = InventoryManager::new();
    /// manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));
    ///
    /// assert!(manager.check_availability("flour", 50));
    /// assert!(manager.check_availability("flour", 100));
    /// assert!(!manager.check_availability("flour", 101));
    /// assert!(!manager.check_availability("salt", 10)); // Item doesn't exist
    /// ```
    pub fn check_availability(&self, item_type: &str, amount: u32) -> bool {
        let items = self.items.lock().unwrap();
        items
            .get(item_type)
            .map(|inv| inv.quantity() >= amount)
            .unwrap_or(false)
    }

    /// Consume a specified amount from a single item
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::resources::inventory::{Inventory, InventoryManager};
    ///
    /// let mut manager = InventoryManager::new();
    /// manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));
    ///
    /// assert!(manager.consume_item("flour", 30).is_ok());
    /// assert_eq!(manager.get_quantity("flour"), Some(70));
    ///
    /// // Error cases
    /// assert!(manager.consume_item("flour", 100).is_err()); // Insufficient
    /// assert!(manager.consume_item("salt", 10).is_err());   // Not found
    /// ```
    pub fn consume_item(&self, item_type: &str, amount: u32) -> Result<()> {
        let mut items = self.items.lock().unwrap();
        let item = items.get_mut(item_type).ok_or_else(|| {
            InventoryError::ItemNotFound {
                item: item_type.to_string(),
            }
        })?;
        item.consume(amount)
    }

    /// Replenish a specified amount to a single item
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::resources::inventory::{Inventory, InventoryManager};
    ///
    /// let mut manager = InventoryManager::new();
    /// manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));
    ///
    /// assert!(manager.replenish_item("flour", 150).is_ok());
    /// assert_eq!(manager.get_quantity("flour"), Some(250));
    ///
    /// // Error cases
    /// assert!(manager.replenish_item("flour", 300).is_err()); // Exceeds capacity
    /// assert!(manager.replenish_item("salt", 100).is_err());  // Not found
    /// ```
    pub fn replenish_item(&self, item_type: &str, amount: u32) -> Result<()> {
        let mut items = self.items.lock().unwrap();
        let item = items.get_mut(item_type).ok_or_else(|| {
            InventoryError::ItemNotFound {
                item: item_type.to_string(),
            }
        })?;
        item.replenish(amount)
    }

    /// Consume multiple items in a single batch operation
    ///
    /// This is an atomic operation - if any item fails, none are consumed.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::resources::inventory::{Inventory, InventoryManager};
    /// use std::collections::HashMap;
    ///
    /// let mut manager = InventoryManager::new();
    /// manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));
    /// manager.add_item(Inventory::new("sugar".to_string(), 50, 300, 50, 150));
    ///
    /// let mut recipe = HashMap::new();
    /// recipe.insert("flour".to_string(), 20);
    /// recipe.insert("sugar".to_string(), 10);
    ///
    /// assert!(manager.consume_multiple(&recipe).is_ok());
    /// assert_eq!(manager.get_quantity("flour"), Some(80));
    /// assert_eq!(manager.get_quantity("sugar"), Some(40));
    ///
    /// // Atomic failure - insufficient flour
    /// recipe.insert("flour".to_string(), 100);
    /// assert!(manager.consume_multiple(&recipe).is_err());
    /// // Quantities unchanged
    /// assert_eq!(manager.get_quantity("flour"), Some(80));
    /// assert_eq!(manager.get_quantity("sugar"), Some(40));
    /// ```
    pub fn consume_multiple(&self, items: &HashMap<String, u32>) -> Result<()> {
        let mut locked_items = self.items.lock().unwrap();

        // First pass: check all items are available
        for (item_type, amount) in items {
            let item = locked_items.get(item_type).ok_or_else(|| {
                InventoryError::ItemNotFound {
                    item: item_type.clone(),
                }
            })?;

            if item.quantity() < *amount {
                return Err(InventoryError::InsufficientQuantity {
                    item: item_type.clone(),
                    requested: *amount,
                    available: item.quantity(),
                });
            }
        }

        // Second pass: actually consume
        for (item_type, amount) in items {
            let item = locked_items.get_mut(item_type).unwrap();
            item.consume(*amount)?;
        }

        Ok(())
    }

    /// Replenish multiple items in a single batch operation
    ///
    /// This is an atomic operation - if any item fails, none are replenished.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::resources::inventory::{Inventory, InventoryManager};
    /// use std::collections::HashMap;
    ///
    /// let mut manager = InventoryManager::new();
    /// manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));
    /// manager.add_item(Inventory::new("sugar".to_string(), 50, 300, 50, 150));
    ///
    /// let mut delivery = HashMap::new();
    /// delivery.insert("flour".to_string(), 200);
    /// delivery.insert("sugar".to_string(), 100);
    ///
    /// assert!(manager.replenish_multiple(&delivery).is_ok());
    /// assert_eq!(manager.get_quantity("flour"), Some(300));
    /// assert_eq!(manager.get_quantity("sugar"), Some(150));
    /// ```
    pub fn replenish_multiple(&self, items: &HashMap<String, u32>) -> Result<()> {
        let mut locked_items = self.items.lock().unwrap();

        // First pass: check all items can be replenished
        for (item_type, amount) in items {
            let item = locked_items.get(item_type).ok_or_else(|| {
                InventoryError::ItemNotFound {
                    item: item_type.clone(),
                }
            })?;

            let new_quantity = item.quantity().checked_add(*amount).ok_or_else(|| {
                InventoryError::InvalidQuantity {
                    reason: "Quantity overflow".to_string(),
                }
            })?;

            if new_quantity > item.capacity {
                return Err(InventoryError::CapacityExceeded {
                    item: item_type.clone(),
                    requested: *amount,
                    capacity: item.capacity,
                    current: item.quantity(),
                });
            }
        }

        // Second pass: actually replenish
        for (item_type, amount) in items {
            let item = locked_items.get_mut(item_type).unwrap();
            item.replenish(*amount)?;
        }

        Ok(())
    }

    /// Get list of items that need reordering
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::resources::inventory::{Inventory, InventoryManager};
    ///
    /// let mut manager = InventoryManager::new();
    /// manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));
    /// manager.add_item(Inventory::new("sugar".to_string(), 40, 300, 50, 150));
    /// manager.add_item(Inventory::new("oil".to_string(), 60, 200, 30, 100));
    ///
    /// let low_items = manager.items_needing_reorder();
    /// assert_eq!(low_items.len(), 1);
    /// assert!(low_items.contains(&"sugar".to_string()));
    /// ```
    pub fn items_needing_reorder(&self) -> Vec<String> {
        let items = self.items.lock().unwrap();
        items
            .iter()
            .filter(|(_, inv)| inv.needs_reorder())
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Get total number of items tracked
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::resources::inventory::{Inventory, InventoryManager};
    ///
    /// let mut manager = InventoryManager::new();
    /// assert_eq!(manager.item_count(), 0);
    ///
    /// manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));
    /// manager.add_item(Inventory::new("sugar".to_string(), 50, 300, 50, 150));
    /// assert_eq!(manager.item_count(), 2);
    /// ```
    pub fn item_count(&self) -> usize {
        let items = self.items.lock().unwrap();
        items.len()
    }

    /// Get reorder point for an item
    ///
    /// # Returns
    ///
    /// * `Some(reorder_point)` - If item exists
    /// * `None` - If item not found
    pub fn get_reorder_point(&self, item_type: &str) -> Option<u32> {
        let items = self.items.lock().unwrap();
        items.get(item_type).map(|inv| inv.reorder_point)
    }

    /// Get reorder quantity for an item
    ///
    /// # Returns
    ///
    /// * `Some(reorder_quantity)` - If item exists
    /// * `None` - If item not found
    pub fn get_reorder_quantity(&self, item_type: &str) -> Option<u32> {
        let items = self.items.lock().unwrap();
        items.get(item_type).map(|inv| inv.reorder_quantity)
    }
}

impl Default for InventoryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for InventoryManager {
    fn clone(&self) -> Self {
        let items = self.items.lock().unwrap();
        Self {
            items: Arc::new(Mutex::new(items.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inventory_creation() {
        let flour = Inventory::new("flour".to_string(), 100, 500, 50, 200);

        assert_eq!(flour.item_type, "flour");
        assert_eq!(flour.quantity(), 100);
        assert_eq!(flour.capacity, 500);
        assert_eq!(flour.reorder_point, 50);
        assert_eq!(flour.reorder_quantity, 200);
        assert!(!flour.is_low());
        assert!(!flour.needs_reorder());
    }

    #[test]
    fn test_consume_success() {
        let mut flour = Inventory::new("flour".to_string(), 100, 500, 50, 200);

        assert!(flour.consume(30).is_ok());
        assert_eq!(flour.quantity(), 70);

        assert!(flour.consume(20).is_ok());
        assert_eq!(flour.quantity(), 50);
    }

    #[test]
    fn test_consume_insufficient() {
        let mut flour = Inventory::new("flour".to_string(), 100, 500, 50, 200);

        let result = flour.consume(150);
        assert!(result.is_err());
        assert_eq!(flour.quantity(), 100); // Unchanged

        match result {
            Err(InventoryError::InsufficientQuantity {
                item,
                requested,
                available,
            }) => {
                assert_eq!(item, "flour");
                assert_eq!(requested, 150);
                assert_eq!(available, 100);
            }
            _ => panic!("Expected InsufficientQuantity error"),
        }
    }

    #[test]
    fn test_consume_zero() {
        let mut flour = Inventory::new("flour".to_string(), 100, 500, 50, 200);

        let result = flour.consume(0);
        assert!(result.is_err());
        assert_eq!(flour.quantity(), 100);

        match result {
            Err(InventoryError::InvalidQuantity { .. }) => {}
            _ => panic!("Expected InvalidQuantity error"),
        }
    }

    #[test]
    fn test_replenish_success() {
        let mut flour = Inventory::new("flour".to_string(), 100, 500, 50, 200);

        assert!(flour.replenish(200).is_ok());
        assert_eq!(flour.quantity(), 300);

        assert!(flour.replenish(100).is_ok());
        assert_eq!(flour.quantity(), 400);
    }

    #[test]
    fn test_replenish_exceeds_capacity() {
        let mut flour = Inventory::new("flour".to_string(), 100, 500, 50, 200);

        let result = flour.replenish(450);
        assert!(result.is_err());
        assert_eq!(flour.quantity(), 100); // Unchanged

        match result {
            Err(InventoryError::CapacityExceeded {
                item,
                requested,
                capacity,
                current,
            }) => {
                assert_eq!(item, "flour");
                assert_eq!(requested, 450);
                assert_eq!(capacity, 500);
                assert_eq!(current, 100);
            }
            _ => panic!("Expected CapacityExceeded error"),
        }
    }

    #[test]
    fn test_replenish_zero() {
        let mut flour = Inventory::new("flour".to_string(), 100, 500, 50, 200);

        let result = flour.replenish(0);
        assert!(result.is_err());
        assert_eq!(flour.quantity(), 100);

        match result {
            Err(InventoryError::InvalidQuantity { .. }) => {}
            _ => panic!("Expected InvalidQuantity error"),
        }
    }

    #[test]
    fn test_low_inventory_detection() {
        let mut flour = Inventory::new("flour".to_string(), 100, 500, 50, 200);

        assert!(!flour.is_low());
        assert!(!flour.needs_reorder());

        flour.consume(45).unwrap();
        assert_eq!(flour.quantity(), 55);
        assert!(!flour.is_low());

        flour.consume(10).unwrap();
        assert_eq!(flour.quantity(), 45);
        assert!(flour.is_low());
        assert!(flour.needs_reorder());

        // Exactly at reorder point
        flour.replenish(5).unwrap();
        assert_eq!(flour.quantity(), 50);
        assert!(flour.is_low());
        assert!(flour.needs_reorder());
    }

    #[test]
    fn test_capacity_percentage() {
        let flour = Inventory::new("flour".to_string(), 250, 500, 50, 200);
        assert_eq!(flour.capacity_percentage(), 50.0);

        let empty = Inventory::new("sugar".to_string(), 0, 300, 50, 150);
        assert_eq!(empty.capacity_percentage(), 0.0);

        let full = Inventory::new("oil".to_string(), 200, 200, 30, 100);
        assert_eq!(full.capacity_percentage(), 100.0);
    }

    #[test]
    fn test_manager_creation() {
        let manager = InventoryManager::new();
        assert_eq!(manager.item_count(), 0);
    }

    #[test]
    fn test_manager_add_item() {
        let mut manager = InventoryManager::new();

        manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));
        assert_eq!(manager.item_count(), 1);

        manager.add_item(Inventory::new("sugar".to_string(), 50, 300, 50, 150));
        assert_eq!(manager.item_count(), 2);
    }

    #[test]
    fn test_manager_get_quantity() {
        let mut manager = InventoryManager::new();
        manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));

        assert_eq!(manager.get_quantity("flour"), Some(100));
        assert_eq!(manager.get_quantity("sugar"), None);
    }

    #[test]
    fn test_manager_check_availability() {
        let mut manager = InventoryManager::new();
        manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));

        assert!(manager.check_availability("flour", 50));
        assert!(manager.check_availability("flour", 100));
        assert!(!manager.check_availability("flour", 101));
        assert!(!manager.check_availability("sugar", 10));
    }

    #[test]
    fn test_manager_consume_item() {
        let mut manager = InventoryManager::new();
        manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));

        assert!(manager.consume_item("flour", 30).is_ok());
        assert_eq!(manager.get_quantity("flour"), Some(70));

        assert!(manager.consume_item("flour", 100).is_err());
        assert_eq!(manager.get_quantity("flour"), Some(70));

        assert!(manager.consume_item("sugar", 10).is_err());
    }

    #[test]
    fn test_manager_replenish_item() {
        let mut manager = InventoryManager::new();
        manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));

        assert!(manager.replenish_item("flour", 150).is_ok());
        assert_eq!(manager.get_quantity("flour"), Some(250));

        assert!(manager.replenish_item("flour", 300).is_err());
        assert_eq!(manager.get_quantity("flour"), Some(250));

        assert!(manager.replenish_item("sugar", 100).is_err());
    }

    #[test]
    fn test_manager_consume_multiple_success() {
        let mut manager = InventoryManager::new();
        manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));
        manager.add_item(Inventory::new("sugar".to_string(), 50, 300, 50, 150));
        manager.add_item(Inventory::new("oil".to_string(), 60, 200, 30, 100));

        let mut recipe = HashMap::new();
        recipe.insert("flour".to_string(), 20);
        recipe.insert("sugar".to_string(), 10);
        recipe.insert("oil".to_string(), 5);

        assert!(manager.consume_multiple(&recipe).is_ok());
        assert_eq!(manager.get_quantity("flour"), Some(80));
        assert_eq!(manager.get_quantity("sugar"), Some(40));
        assert_eq!(manager.get_quantity("oil"), Some(55));
    }

    #[test]
    fn test_manager_consume_multiple_atomic_failure() {
        let mut manager = InventoryManager::new();
        manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));
        manager.add_item(Inventory::new("sugar".to_string(), 50, 300, 50, 150));

        let mut recipe = HashMap::new();
        recipe.insert("flour".to_string(), 20);
        recipe.insert("sugar".to_string(), 60); // Insufficient

        let result = manager.consume_multiple(&recipe);
        assert!(result.is_err());

        // Verify quantities unchanged (atomic operation)
        assert_eq!(manager.get_quantity("flour"), Some(100));
        assert_eq!(manager.get_quantity("sugar"), Some(50));
    }

    #[test]
    fn test_manager_consume_multiple_item_not_found() {
        let mut manager = InventoryManager::new();
        manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));

        let mut recipe = HashMap::new();
        recipe.insert("flour".to_string(), 20);
        recipe.insert("salt".to_string(), 10); // Not in inventory

        let result = manager.consume_multiple(&recipe);
        assert!(result.is_err());
        assert_eq!(manager.get_quantity("flour"), Some(100));

        match result {
            Err(InventoryError::ItemNotFound { item }) => {
                assert_eq!(item, "salt");
            }
            _ => panic!("Expected ItemNotFound error"),
        }
    }

    #[test]
    fn test_manager_replenish_multiple_success() {
        let mut manager = InventoryManager::new();
        manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));
        manager.add_item(Inventory::new("sugar".to_string(), 50, 300, 50, 150));

        let mut delivery = HashMap::new();
        delivery.insert("flour".to_string(), 200);
        delivery.insert("sugar".to_string(), 100);

        assert!(manager.replenish_multiple(&delivery).is_ok());
        assert_eq!(manager.get_quantity("flour"), Some(300));
        assert_eq!(manager.get_quantity("sugar"), Some(150));
    }

    #[test]
    fn test_manager_replenish_multiple_atomic_failure() {
        let mut manager = InventoryManager::new();
        manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));
        manager.add_item(Inventory::new("sugar".to_string(), 50, 300, 50, 150));

        let mut delivery = HashMap::new();
        delivery.insert("flour".to_string(), 200);
        delivery.insert("sugar".to_string(), 300); // Exceeds capacity

        let result = manager.replenish_multiple(&delivery);
        assert!(result.is_err());

        // Verify quantities unchanged (atomic operation)
        assert_eq!(manager.get_quantity("flour"), Some(100));
        assert_eq!(manager.get_quantity("sugar"), Some(50));
    }

    #[test]
    fn test_manager_items_needing_reorder() {
        let mut manager = InventoryManager::new();
        manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));
        manager.add_item(Inventory::new("sugar".to_string(), 40, 300, 50, 150));
        manager.add_item(Inventory::new("oil".to_string(), 25, 200, 30, 100));

        let low_items = manager.items_needing_reorder();
        assert_eq!(low_items.len(), 2);
        assert!(low_items.contains(&"sugar".to_string()));
        assert!(low_items.contains(&"oil".to_string()));
        assert!(!low_items.contains(&"flour".to_string()));
    }

    #[test]
    fn test_overflow_prevention() {
        let mut flour = Inventory::new("flour".to_string(), u32::MAX - 10, u32::MAX, 50, 200);

        let result = flour.replenish(20);
        assert!(result.is_err());
        assert_eq!(flour.quantity(), u32::MAX - 10);
    }

    #[test]
    fn test_manager_clone() {
        let mut manager = InventoryManager::new();
        manager.add_item(Inventory::new("flour".to_string(), 100, 500, 50, 200));

        let cloned = manager.clone();
        assert_eq!(cloned.item_count(), 1);
        assert_eq!(cloned.get_quantity("flour"), Some(100));

        // Modifications to clone don't affect original
        assert!(cloned.consume_item("flour", 30).is_ok());
        assert_eq!(cloned.get_quantity("flour"), Some(70));
        assert_eq!(manager.get_quantity("flour"), Some(100));
    }
}
