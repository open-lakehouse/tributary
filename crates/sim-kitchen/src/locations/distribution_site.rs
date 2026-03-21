//! Distribution site locations for supply chain workflows
//!
//! Distribution sites are special locations that serve as unlimited sources of inventory
//! in supply chain simulations. They fulfill orders from supply drivers without capacity
//! constraints, making them ideal for modeling warehouses, distribution centers, or
//! central supply depots.
//!
//! # Architecture
//!
//! Distribution sites integrate with the sim-geo Location trait, enabling:
//! - H3 spatial indexing for efficient site lookups
//! - Geographic positioning for routing calculations
//! - Multiple distribution sites in a single simulation
//!
//! # Example
//!
//! ```rust
//! use sim_kitchen::locations::{DistributionSite, OrderFulfillment};
//! use geo::Point;
//!
//! // Create a distribution site in San Francisco
//! let site = DistributionSite::new(
//!     "SF Distribution Center".to_string(),
//!     Point::new(-122.4194, 37.7749), // longitude, latitude
//!     vec!["food".to_string(), "beverages".to_string()],
//! );
//!
//! // Check if site can fulfill an order (always true for unlimited inventory)
//! assert!(site.can_fulfill("food", 100));
//!
//! // Process an order
//! let fulfillment = site.process_order("food", 100);
//! assert!(fulfillment.fulfilled);
//! assert_eq!(fulfillment.quantity_fulfilled, 100);
//! ```
//!
//! # Order Processing
//!
//! Distribution sites accept orders and generate fulfillment records. Since they have
//! unlimited inventory, all orders are fulfilled as long as the item type is available.
//!
//! ```rust
//! use sim_kitchen::locations::DistributionSite;
//! use geo::Point;
//!
//! let site = DistributionSite::new(
//!     "Central Warehouse".to_string(),
//!     Point::new(-118.2437, 34.0522), // Los Angeles
//!     vec!["produce".to_string(), "dairy".to_string()],
//! );
//!
//! // Large orders are no problem
//! assert!(site.can_fulfill("produce", 10000));
//!
//! // Unknown items cannot be fulfilled
//! assert!(!site.can_fulfill("electronics", 1));
//! ```

use chrono::{DateTime, Utc};
use geo::Point;
use serde::{Deserialize, Serialize};
use sim_core::LocationId;
use sim_geo::Location;
use std::collections::HashSet;
use uuid::Uuid;

/// Represents an order fulfillment from a distribution site
///
/// Tracks the details of an order that was processed by a distribution site,
/// including what was requested, what was fulfilled, and when.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderFulfillment {
    /// Unique identifier for this fulfillment
    pub fulfillment_id: Uuid,
    /// The distribution site that fulfilled the order
    pub site_id: Uuid,
    /// Type of item ordered
    pub item_type: String,
    /// Quantity requested
    pub quantity_requested: u32,
    /// Quantity actually fulfilled
    pub quantity_fulfilled: u32,
    /// Whether the order was fully fulfilled
    pub fulfilled: bool,
    /// Timestamp when the order was processed
    pub timestamp: DateTime<Utc>,
}

/// A distribution site with unlimited inventory capacity
///
/// Distribution sites serve as sources of inventory in supply chain simulations.
/// They have unlimited capacity and can fulfill any quantity of items they stock.
///
/// # Fields
///
/// - `site_id`: Unique identifier for the distribution site
/// - `location_id`: sim-core LocationId for integration with the framework
/// - `name`: Human-readable name (e.g., "SF Distribution Center")
/// - `position`: Geographic position (longitude, latitude)
/// - `available_items`: Set of item types that can be fulfilled
///
/// # Integration
///
/// Implements the `sim_geo::Location` trait for spatial indexing and queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionSite {
    /// Unique identifier for this distribution site
    pub site_id: Uuid,
    /// Location ID for sim-core integration
    location_id: LocationId,
    /// Human-readable name of the distribution site
    pub name: String,
    /// Geographic position (longitude, latitude)
    pub position: Point<f64>,
    /// Set of item types available at this site
    pub available_items: HashSet<String>,
}

impl DistributionSite {
    /// Create a new distribution site
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name for the site
    /// * `position` - Geographic position (longitude, latitude)
    /// * `items` - Vector of item types available at this site
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::locations::DistributionSite;
    /// use geo::Point;
    ///
    /// let site = DistributionSite::new(
    ///     "Main Warehouse".to_string(),
    ///     Point::new(-122.4194, 37.7749),
    ///     vec!["food".to_string(), "beverages".to_string()],
    /// );
    /// ```
    pub fn new(name: String, position: Point<f64>, items: Vec<String>) -> Self {
        Self {
            site_id: Uuid::new_v4(),
            location_id: LocationId::new(),
            name,
            position,
            available_items: items.into_iter().collect(),
        }
    }

    /// Create a distribution site with a specific site_id
    ///
    /// Useful for testing or when you need deterministic IDs.
    ///
    /// # Arguments
    ///
    /// * `site_id` - Specific UUID to use
    /// * `name` - Human-readable name for the site
    /// * `position` - Geographic position (longitude, latitude)
    /// * `items` - Vector of item types available at this site
    pub fn with_id(site_id: Uuid, name: String, position: Point<f64>, items: Vec<String>) -> Self {
        Self {
            site_id,
            location_id: LocationId::new(),
            name,
            position,
            available_items: items.into_iter().collect(),
        }
    }

    /// Check if the site can fulfill an order for the given item and quantity
    ///
    /// Distribution sites have unlimited inventory, so this only checks if the
    /// item type is available.
    ///
    /// # Arguments
    ///
    /// * `item_type` - The type of item being requested
    /// * `quantity` - The quantity requested (always fulfillable if item is available)
    ///
    /// # Returns
    ///
    /// `true` if the item type is available, `false` otherwise
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::locations::DistributionSite;
    /// use geo::Point;
    ///
    /// let site = DistributionSite::new(
    ///     "Warehouse".to_string(),
    ///     Point::new(0.0, 0.0),
    ///     vec!["widgets".to_string()],
    /// );
    ///
    /// assert!(site.can_fulfill("widgets", 1000));
    /// assert!(!site.can_fulfill("gadgets", 1));
    /// ```
    pub fn can_fulfill(&self, item_type: &str, _quantity: u32) -> bool {
        self.available_items.contains(item_type)
    }

    /// Process an order and generate a fulfillment record
    ///
    /// Distribution sites have unlimited capacity, so orders are always fulfilled
    /// if the item type is available.
    ///
    /// # Arguments
    ///
    /// * `item_type` - The type of item being ordered
    /// * `quantity` - The quantity requested
    ///
    /// # Returns
    ///
    /// An `OrderFulfillment` record with fulfillment details
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::locations::DistributionSite;
    /// use geo::Point;
    ///
    /// let site = DistributionSite::new(
    ///     "Distribution Center".to_string(),
    ///     Point::new(-118.2437, 34.0522),
    ///     vec!["supplies".to_string()],
    /// );
    ///
    /// let fulfillment = site.process_order("supplies", 500);
    /// assert!(fulfillment.fulfilled);
    /// assert_eq!(fulfillment.quantity_fulfilled, 500);
    /// ```
    pub fn process_order(&self, item_type: &str, quantity: u32) -> OrderFulfillment {
        let can_fulfill = self.can_fulfill(item_type, quantity);

        OrderFulfillment {
            fulfillment_id: Uuid::new_v4(),
            site_id: self.site_id,
            item_type: item_type.to_string(),
            quantity_requested: quantity,
            quantity_fulfilled: if can_fulfill { quantity } else { 0 },
            fulfilled: can_fulfill,
            timestamp: Utc::now(),
        }
    }

    /// List all available item types at this distribution site
    ///
    /// # Returns
    ///
    /// A vector of item type strings
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::locations::DistributionSite;
    /// use geo::Point;
    ///
    /// let site = DistributionSite::new(
    ///     "Warehouse".to_string(),
    ///     Point::new(0.0, 0.0),
    ///     vec!["food".to_string(), "beverages".to_string()],
    /// );
    ///
    /// let items = site.list_available_items();
    /// assert_eq!(items.len(), 2);
    /// assert!(items.contains(&"food".to_string()));
    /// ```
    pub fn list_available_items(&self) -> Vec<String> {
        self.available_items.iter().cloned().collect()
    }

    /// Add a new item type to the distribution site's inventory
    ///
    /// # Arguments
    ///
    /// * `item_type` - The item type to add
    ///
    /// # Returns
    ///
    /// `true` if the item was added, `false` if it already existed
    pub fn add_item_type(&mut self, item_type: String) -> bool {
        self.available_items.insert(item_type)
    }

    /// Remove an item type from the distribution site's inventory
    ///
    /// # Arguments
    ///
    /// * `item_type` - The item type to remove
    ///
    /// # Returns
    ///
    /// `true` if the item was removed, `false` if it didn't exist
    pub fn remove_item_type(&mut self, item_type: &str) -> bool {
        self.available_items.remove(item_type)
    }
}

/// Implement the Location trait for spatial indexing
impl Location for DistributionSite {
    fn id(&self) -> LocationId {
        self.location_id
    }

    fn location_type(&self) -> &str {
        "distribution_site"
    }

    fn position(&self) -> Point<f64> {
        self.position
    }

    fn properties(&self) -> serde_json::Value {
        serde_json::json!({
            "site_id": self.site_id.to_string(),
            "name": self.name,
            "available_items": self.available_items.iter().collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use h3o::Resolution;

    #[test]
    fn test_create_distribution_site() {
        // Create a distribution site in San Francisco
        let site = DistributionSite::new(
            "SF Distribution Center".to_string(),
            Point::new(-122.4194, 37.7749),
            vec!["food".to_string(), "beverages".to_string()],
        );

        assert_eq!(site.name, "SF Distribution Center");
        assert_relative_eq!(site.position.x(), -122.4194, epsilon = 0.0001);
        assert_relative_eq!(site.position.y(), 37.7749, epsilon = 0.0001);
        assert_eq!(site.available_items.len(), 2);
        assert!(site.available_items.contains("food"));
        assert!(site.available_items.contains("beverages"));
    }

    #[test]
    fn test_fulfill_order_happy_path() {
        let site = DistributionSite::new(
            "Test Warehouse".to_string(),
            Point::new(-118.2437, 34.0522), // Los Angeles
            vec!["widgets".to_string()],
        );

        // Can fulfill available item
        assert!(site.can_fulfill("widgets", 100));

        // Process order
        let fulfillment = site.process_order("widgets", 100);
        assert_eq!(fulfillment.site_id, site.site_id);
        assert_eq!(fulfillment.item_type, "widgets");
        assert_eq!(fulfillment.quantity_requested, 100);
        assert_eq!(fulfillment.quantity_fulfilled, 100);
        assert!(fulfillment.fulfilled);
    }

    #[test]
    fn test_check_item_availability() {
        let site = DistributionSite::new(
            "Multi-Item Warehouse".to_string(),
            Point::new(0.0, 0.0),
            vec![
                "electronics".to_string(),
                "clothing".to_string(),
                "books".to_string(),
            ],
        );

        // Available items
        assert!(site.can_fulfill("electronics", 1));
        assert!(site.can_fulfill("clothing", 1000));
        assert!(site.can_fulfill("books", 5));

        // Unavailable items
        assert!(!site.can_fulfill("furniture", 1));
        assert!(!site.can_fulfill("toys", 100));

        // List available items
        let items = site.list_available_items();
        assert_eq!(items.len(), 3);
        assert!(items.contains(&"electronics".to_string()));
        assert!(items.contains(&"clothing".to_string()));
        assert!(items.contains(&"books".to_string()));
    }

    #[test]
    fn test_multiple_sites_management() {
        // Create multiple distribution sites
        let site1 = DistributionSite::new(
            "East Coast Hub".to_string(),
            Point::new(-74.0060, 40.7128), // New York
            vec!["electronics".to_string()],
        );

        let site2 = DistributionSite::new(
            "West Coast Hub".to_string(),
            Point::new(-118.2437, 34.0522), // Los Angeles
            vec!["clothing".to_string()],
        );

        // Sites have unique IDs
        assert_ne!(site1.site_id, site2.site_id);
        assert_ne!(site1.id(), site2.id());

        // Sites have different inventories
        assert!(site1.can_fulfill("electronics", 1));
        assert!(!site1.can_fulfill("clothing", 1));
        assert!(site2.can_fulfill("clothing", 1));
        assert!(!site2.can_fulfill("electronics", 1));

        // Sites have different locations
        assert_ne!(site1.position, site2.position);
    }

    #[test]
    fn test_large_order_fulfillment_no_capacity_limits() {
        let site = DistributionSite::new(
            "Unlimited Warehouse".to_string(),
            Point::new(-122.3321, 47.6062), // Seattle
            vec!["bulk_goods".to_string()],
        );

        // Test extremely large orders (unlimited capacity)
        assert!(site.can_fulfill("bulk_goods", 1_000_000));
        assert!(site.can_fulfill("bulk_goods", u32::MAX));

        let fulfillment = site.process_order("bulk_goods", 1_000_000);
        assert!(fulfillment.fulfilled);
        assert_eq!(fulfillment.quantity_fulfilled, 1_000_000);
    }

    #[test]
    fn test_unfulfillable_order() {
        let site = DistributionSite::new(
            "Limited Selection Warehouse".to_string(),
            Point::new(0.0, 0.0),
            vec!["specific_item".to_string()],
        );

        // Try to order an unavailable item
        assert!(!site.can_fulfill("unavailable_item", 1));

        let fulfillment = site.process_order("unavailable_item", 100);
        assert!(!fulfillment.fulfilled);
        assert_eq!(fulfillment.quantity_fulfilled, 0);
        assert_eq!(fulfillment.quantity_requested, 100);
    }

    #[test]
    fn test_location_trait_implementation() {
        let site = DistributionSite::new(
            "Trait Test Site".to_string(),
            Point::new(-122.4194, 37.7749),
            vec!["test_item".to_string()],
        );

        // Test Location trait methods
        assert_eq!(site.location_type(), "distribution_site");
        assert_relative_eq!(site.position().x(), -122.4194, epsilon = 0.0001);
        assert_relative_eq!(site.position().y(), 37.7749, epsilon = 0.0001);

        // Test H3 spatial indexing
        let h3_cell = site.h3_cell(Resolution::Nine);
        assert!(h3_cell.is_ok());

        // Test properties
        let props = site.properties();
        assert!(props.is_object());
        assert!(props.get("name").is_some());
        assert!(props.get("site_id").is_some());
    }

    #[test]
    fn test_add_remove_item_types() {
        let mut site = DistributionSite::new(
            "Dynamic Inventory Site".to_string(),
            Point::new(0.0, 0.0),
            vec!["initial_item".to_string()],
        );

        // Initially has one item
        assert_eq!(site.list_available_items().len(), 1);
        assert!(site.can_fulfill("initial_item", 1));

        // Add new item type
        assert!(site.add_item_type("new_item".to_string()));
        assert_eq!(site.list_available_items().len(), 2);
        assert!(site.can_fulfill("new_item", 1));

        // Adding duplicate returns false
        assert!(!site.add_item_type("new_item".to_string()));
        assert_eq!(site.list_available_items().len(), 2);

        // Remove item type
        assert!(site.remove_item_type("initial_item"));
        assert_eq!(site.list_available_items().len(), 1);
        assert!(!site.can_fulfill("initial_item", 1));

        // Removing non-existent item returns false
        assert!(!site.remove_item_type("non_existent"));
    }

    #[test]
    fn test_with_id_constructor() {
        let specific_id = Uuid::new_v4();
        let site = DistributionSite::with_id(
            specific_id,
            "ID Test Site".to_string(),
            Point::new(0.0, 0.0),
            vec!["test".to_string()],
        );

        assert_eq!(site.site_id, specific_id);
        assert_eq!(site.name, "ID Test Site");
    }

    #[test]
    fn test_order_fulfillment_timestamp() {
        let site = DistributionSite::new(
            "Timestamp Test Site".to_string(),
            Point::new(0.0, 0.0),
            vec!["item".to_string()],
        );

        let before = Utc::now();
        let fulfillment = site.process_order("item", 10);
        let after = Utc::now();

        // Timestamp should be between before and after
        assert!(fulfillment.timestamp >= before);
        assert!(fulfillment.timestamp <= after);
    }

    #[test]
    fn test_zero_quantity_order() {
        let site = DistributionSite::new(
            "Zero Quantity Test".to_string(),
            Point::new(0.0, 0.0),
            vec!["item".to_string()],
        );

        // Even zero quantity should be fulfillable if item exists
        assert!(site.can_fulfill("item", 0));
        let fulfillment = site.process_order("item", 0);
        assert!(fulfillment.fulfilled);
        assert_eq!(fulfillment.quantity_fulfilled, 0);
    }
}
