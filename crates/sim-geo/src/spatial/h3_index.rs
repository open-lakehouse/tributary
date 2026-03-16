//! H3 spatial index

use dashmap::DashMap;
use geo_types::Point;
use h3o::{CellIndex, LatLng, Resolution};
use std::collections::HashSet;
use std::hash::Hash;

/// H3-based spatial index for fast proximity queries
///
/// This index uses Uber's H3 hierarchical hexagonal grid system to efficiently
/// index and query spatial data.
pub struct H3Index<T: Clone + Eq + Hash> {
    /// Map from H3 cell to items in that cell
    cells: DashMap<CellIndex, HashSet<T>>,
}

impl<T: Clone + Eq + Hash> H3Index<T> {
    /// Create a new H3 index
    pub fn new() -> Self {
        Self {
            cells: DashMap::new(),
        }
    }

    /// Insert an item at a cell
    pub fn insert(&mut self, cell: CellIndex, item: T) {
        self.cells.entry(cell).or_default().insert(item);
    }

    /// Remove an item from a cell
    pub fn remove(&mut self, cell: CellIndex, item: T) -> bool {
        if let Some(mut set) = self.cells.get_mut(&cell) {
            return set.remove(&item);
        }
        false
    }

    /// Get all items in a specific cell
    pub fn get_cell(&self, cell: CellIndex) -> Vec<T> {
        self.cells
            .get(&cell)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Find items near a point
    ///
    /// Returns items in the cell containing the point and its neighbors.
    pub fn nearby(&self, point: Point<f64>, resolution: Resolution) -> crate::Result<Vec<T>> {
        let latlng = LatLng::new(point.y(), point.x())
            .map_err(|e| crate::Error::H3(format!("Invalid coordinates: {}", e)))?;

        let cell = latlng.to_cell(resolution);

        let mut items = HashSet::new();

        // Get items in the cell itself
        if let Some(set) = self.cells.get(&cell) {
            items.extend(set.iter().cloned());
        }

        // Get items in neighboring cells (ring 1)
        for neighbor in cell.grid_disk::<Vec<_>>(1) {
            if let Some(set) = self.cells.get(&neighbor) {
                items.extend(set.iter().cloned());
            }
        }

        Ok(items.into_iter().collect())
    }

    /// Get total number of indexed cells
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Get total number of items (across all cells)
    pub fn item_count(&self) -> usize {
        self.cells.iter().map(|entry| entry.value().len()).sum()
    }

    /// Clear all data
    pub fn clear(&mut self) {
        self.cells.clear();
    }
}

impl<T: Clone + Eq + Hash> Default for H3Index<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_h3_index_insert_and_get() {
        let mut index = H3Index::<u32>::new();

        let point = Point::new(-122.4194, 37.7749); // San Francisco
        let latlng = LatLng::new(point.y(), point.x()).unwrap();
        let cell = latlng.to_cell(Resolution::Nine);

        index.insert(cell, 1);
        index.insert(cell, 2);

        let items = index.get_cell(cell);
        assert_eq!(items.len(), 2);
        assert!(items.contains(&1));
        assert!(items.contains(&2));
    }

    #[test]
    fn test_h3_index_nearby() {
        let mut index = H3Index::<u32>::new();

        let point1 = Point::new(-122.4194, 37.7749); // San Francisco
        let latlng1 = LatLng::new(point1.y(), point1.x()).unwrap();
        let cell1 = latlng1.to_cell(Resolution::Nine);

        index.insert(cell1, 1);

        // Search nearby
        let nearby = index.nearby(point1, Resolution::Nine).unwrap();
        assert!(nearby.contains(&1));
    }

    #[test]
    fn test_h3_index_remove() {
        let mut index = H3Index::<u32>::new();

        let point = Point::new(-122.4194, 37.7749);
        let latlng = LatLng::new(point.y(), point.x()).unwrap();
        let cell = latlng.to_cell(Resolution::Nine);

        index.insert(cell, 1);
        assert_eq!(index.get_cell(cell).len(), 1);

        index.remove(cell, 1);
        assert_eq!(index.get_cell(cell).len(), 0);
    }

    #[test]
    fn test_h3_index_counts() {
        let mut index = H3Index::<u32>::new();

        let point1 = Point::new(-122.4194, 37.7749);
        let point2 = Point::new(-122.0842, 37.4224);

        let latlng1 = LatLng::new(point1.y(), point1.x()).unwrap();
        let latlng2 = LatLng::new(point2.y(), point2.x()).unwrap();

        let cell1 = latlng1.to_cell(Resolution::Nine);
        let cell2 = latlng2.to_cell(Resolution::Nine);

        index.insert(cell1, 1);
        index.insert(cell1, 2);
        index.insert(cell2, 3);

        assert_eq!(index.cell_count(), 2);
        assert_eq!(index.item_count(), 3);
    }

    #[test]
    fn test_h3_index_clear() {
        let mut index = H3Index::<u32>::new();

        let point = Point::new(-122.4194, 37.7749);
        let latlng = LatLng::new(point.y(), point.x()).unwrap();
        let cell = latlng.to_cell(Resolution::Nine);

        index.insert(cell, 1);
        assert_eq!(index.item_count(), 1);

        index.clear();
        assert_eq!(index.item_count(), 0);
        assert_eq!(index.cell_count(), 0);
    }
}
