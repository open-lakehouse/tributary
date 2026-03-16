//! Location types for logistics domain
//!
//! Warehouses and delivery addresses with H3 spatial indexing

use geo::Point;
use h3o::{CellIndex, LatLng, Resolution};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Warehouse location
///
/// Central distribution point for package processing and dispatch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Warehouse {
    pub id: Uuid,
    pub name: String,
    pub location: Point<f64>,
    #[serde(skip)]
    pub h3_cell: Option<CellIndex>,
    pub package_capacity: u32,
    pub current_package_count: u32,
    pub operating_hours_start: u8, // Hour 0-23
    pub operating_hours_end: u8,
}

impl Warehouse {
    /// Create a new warehouse
    pub fn new(
        name: String,
        location: Point<f64>,
        package_capacity: u32,
    ) -> Result<Self, String> {
        let h3_cell = location_to_h3(location, Resolution::Nine)?;

        Ok(Self {
            id: Uuid::new_v4(),
            name,
            location,
            h3_cell: Some(h3_cell),
            package_capacity,
            current_package_count: 0,
            operating_hours_start: 0,
            operating_hours_end: 24,
        })
    }

    /// Check if warehouse has capacity
    pub fn has_capacity(&self) -> bool {
        self.current_package_count < self.package_capacity
    }

    /// Get available capacity
    pub fn available_capacity(&self) -> u32 {
        self.package_capacity.saturating_sub(self.current_package_count)
    }

    /// Add packages to warehouse
    pub fn add_packages(&mut self, count: u32) -> Result<(), String> {
        if self.current_package_count + count > self.package_capacity {
            return Err("Warehouse capacity exceeded".to_string());
        }
        self.current_package_count += count;
        Ok(())
    }

    /// Remove packages from warehouse
    pub fn remove_packages(&mut self, count: u32) -> Result<(), String> {
        if self.current_package_count < count {
            return Err("Insufficient packages in warehouse".to_string());
        }
        self.current_package_count -= count;
        Ok(())
    }

    /// Check if warehouse is operating at current hour
    pub fn is_operating(&self, hour: u8) -> bool {
        hour >= self.operating_hours_start && hour < self.operating_hours_end
    }
}

/// Delivery address
///
/// Final destination for package delivery with spatial indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryAddress {
    pub id: Uuid,
    pub location: Point<f64>,
    #[serde(skip)]
    pub h3_cell: Option<CellIndex>,
    pub delivery_time_window_start: Option<u8>, // Hour 0-23
    pub delivery_time_window_end: Option<u8>,
    pub recipient_name: String,
}

impl DeliveryAddress {
    /// Create a new delivery address
    pub fn new(
        location: Point<f64>,
        recipient_name: String,
    ) -> Result<Self, String> {
        let h3_cell = location_to_h3(location, Resolution::Nine)?;

        Ok(Self {
            id: Uuid::new_v4(),
            location,
            h3_cell: Some(h3_cell),
            delivery_time_window_start: None,
            delivery_time_window_end: None,
            recipient_name,
        })
    }

    /// Set delivery time window
    pub fn with_time_window(mut self, start: u8, end: u8) -> Self {
        self.delivery_time_window_start = Some(start);
        self.delivery_time_window_end = Some(end);
        self
    }

    /// Check if delivery can be made at current hour
    pub fn accepts_delivery_at(&self, hour: u8) -> bool {
        if let (Some(start), Some(end)) = (self.delivery_time_window_start, self.delivery_time_window_end) {
            hour >= start && hour < end
        } else {
            true // No restriction
        }
    }
}

/// Convert geographic coordinates to H3 cell index
fn location_to_h3(location: Point<f64>, resolution: Resolution) -> Result<CellIndex, String> {
    let lat_lng = LatLng::new(location.y(), location.x())
        .map_err(|e| format!("Invalid coordinates: {}", e))?;

    Ok(lat_lng.to_cell(resolution))
}

/// Calculate distance between two H3 cells (approximate)
pub fn h3_distance(cell1: CellIndex, cell2: CellIndex) -> u32 {
    cell1.grid_distance(cell2).unwrap_or(i32::MAX) as u32
}

/// Get all neighbors of an H3 cell
pub fn h3_neighbors(cell: CellIndex) -> Vec<CellIndex> {
    cell.grid_disk::<Vec<_>>(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_warehouse_creation() {
        let location = Point::new(-122.4, 37.8);
        let warehouse = Warehouse::new("Main Warehouse".to_string(), location, 1000).unwrap();

        assert_eq!(warehouse.name, "Main Warehouse");
        assert_eq!(warehouse.location, location);
        assert_eq!(warehouse.package_capacity, 1000);
        assert_eq!(warehouse.current_package_count, 0);
        assert!(warehouse.has_capacity());
    }

    #[test]
    fn test_warehouse_capacity() {
        let mut warehouse = Warehouse::new(
            "Test".to_string(),
            Point::new(0.0, 0.0),
            100,
        ).unwrap();

        assert!(warehouse.add_packages(50).is_ok());
        assert_eq!(warehouse.current_package_count, 50);
        assert_eq!(warehouse.available_capacity(), 50);

        assert!(warehouse.add_packages(50).is_ok());
        assert_eq!(warehouse.current_package_count, 100);
        assert!(!warehouse.has_capacity());

        assert!(warehouse.add_packages(1).is_err());
    }

    #[test]
    fn test_warehouse_remove_packages() {
        let mut warehouse = Warehouse::new(
            "Test".to_string(),
            Point::new(0.0, 0.0),
            100,
        ).unwrap();

        warehouse.add_packages(50).unwrap();
        assert!(warehouse.remove_packages(30).is_ok());
        assert_eq!(warehouse.current_package_count, 20);

        assert!(warehouse.remove_packages(30).is_err());
    }

    #[test]
    fn test_warehouse_operating_hours() {
        let warehouse = Warehouse::new(
            "24/7".to_string(),
            Point::new(0.0, 0.0),
            100,
        ).unwrap();

        assert!(warehouse.is_operating(0));
        assert!(warehouse.is_operating(12));
        assert!(warehouse.is_operating(23));
    }

    #[test]
    fn test_delivery_address_creation() {
        let location = Point::new(-122.3, 37.7);
        let address = DeliveryAddress::new(location, "John Doe".to_string()).unwrap();

        assert_eq!(address.location, location);
        assert_eq!(address.recipient_name, "John Doe");
        assert!(address.accepts_delivery_at(12));
    }

    #[test]
    fn test_delivery_time_window() {
        let location = Point::new(0.0, 0.0);
        let address = DeliveryAddress::new(location, "Jane".to_string())
            .unwrap()
            .with_time_window(9, 17); // 9am-5pm

        assert!(address.accepts_delivery_at(9));
        assert!(address.accepts_delivery_at(12));
        assert!(address.accepts_delivery_at(16));
        assert!(!address.accepts_delivery_at(8));
        assert!(!address.accepts_delivery_at(17));
        assert!(!address.accepts_delivery_at(20));
    }

    #[test]
    fn test_h3_indexing() {
        let loc1 = Point::new(-122.4, 37.8);
        let cell1 = location_to_h3(loc1, Resolution::Nine).unwrap();

        // Same location should give same cell
        let cell1_again = location_to_h3(loc1, Resolution::Nine).unwrap();
        assert_eq!(cell1, cell1_again);

        // Different location should give different cell
        let loc2 = Point::new(-122.5, 37.9);
        let cell2 = location_to_h3(loc2, Resolution::Nine).unwrap();
        assert_ne!(cell1, cell2);
    }

    #[test]
    fn test_h3_neighbors() {
        let location = Point::new(0.0, 0.0);
        let cell = location_to_h3(location, Resolution::Nine).unwrap();

        let neighbors = h3_neighbors(cell);
        assert!(neighbors.len() >= 6); // Hexagons have 6 neighbors (+ center = 7)
    }
}
