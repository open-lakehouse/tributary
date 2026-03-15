//! Location handler implementation

use super::traits::Location;
use crate::spatial::H3Index;
use geo::Distance;
use geo::Haversine;
use geo_types::Point;
use h3o::Resolution;
use sim_core::LocationId;
use std::collections::HashMap;
use std::marker::PhantomData;

/// Manages locations in the simulation
///
/// Provides spatial indexing and proximity queries using H3.
///
/// # Type Parameters
///
/// * `L` - Location type
///
/// # Example
///
/// ```rust,ignore
/// use sim_geo::{LocationHandler, Location};
///
/// let mut handler = LocationHandler::<MyLocation>::new(Resolution::Nine);
///
/// handler.add_location(warehouse1);
/// handler.add_location(warehouse2);
///
/// // Find nearest location to a point
/// let nearest = handler.nearest(my_position)?;
/// ```
pub struct LocationHandler<L: Location> {
    /// Map of location ID to location
    locations: HashMap<LocationId, L>,
    /// H3 spatial index
    spatial_index: H3Index<LocationId>,
    /// H3 resolution for indexing
    resolution: Resolution,
    /// Phantom data
    _phantom: PhantomData<L>,
}

impl<L: Location> LocationHandler<L> {
    /// Create a new location handler
    ///
    /// # Arguments
    ///
    /// * `resolution` - H3 resolution for spatial indexing (typically 9 for block-level)
    pub fn new(resolution: Resolution) -> Self {
        Self {
            locations: HashMap::new(),
            spatial_index: H3Index::new(),
            resolution,
            _phantom: PhantomData,
        }
    }

    /// Add a location to the handler
    pub fn add_location(&mut self, location: L) -> crate::Result<()> {
        let id = location.id();
        let cell = location.h3_cell(self.resolution)?;

        // Add to spatial index
        self.spatial_index.insert(cell, id);

        // Store location
        self.locations.insert(id, location);

        Ok(())
    }

    /// Remove a location
    pub fn remove_location(&mut self, id: LocationId) -> Option<L> {
        if let Some(location) = self.locations.remove(&id) {
            if let Ok(cell) = location.h3_cell(self.resolution) {
                self.spatial_index.remove(cell, id);
            }
            Some(location)
        } else {
            None
        }
    }

    /// Get a location by ID
    pub fn get(&self, id: LocationId) -> Option<&L> {
        self.locations.get(&id)
    }

    /// Get location count
    pub fn count(&self) -> usize {
        self.locations.len()
    }

    /// Find the nearest location to a point
    ///
    /// # Arguments
    ///
    /// * `point` - Position to search from
    ///
    /// # Returns
    ///
    /// The ID of the nearest location, or None if no locations exist
    pub fn nearest(&self, point: Point<f64>) -> crate::Result<Option<LocationId>> {
        if self.locations.is_empty() {
            return Ok(None);
        }

        // Find candidates using H3
        let candidates = self.spatial_index.nearby(point, self.resolution)?;

        if candidates.is_empty() {
            // Fall back to brute force if no H3 candidates
            return Ok(self.nearest_brute_force(point));
        }

        // Find nearest among candidates
        let nearest = candidates
            .into_iter()
            .filter_map(|id| {
                self.locations.get(&id).map(|loc| {
                    let dist = Haversine.distance(point.into(), loc.position().into());
                    (id, dist)
                })
            })
            .min_by(|(_, d1), (_, d2)| d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| id);

        Ok(nearest)
    }

    /// Brute force nearest location (fallback)
    fn nearest_brute_force(&self, point: Point<f64>) -> Option<LocationId> {
        self.locations
            .iter()
            .map(|(id, loc)| {
                let dist = Haversine.distance(point.into(), loc.position().into());
                (*id, dist)
            })
            .min_by(|(_, d1), (_, d2)| d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| id)
    }

    /// Find all locations within a radius
    ///
    /// # Arguments
    ///
    /// * `point` - Center point
    /// * `radius_meters` - Search radius in meters
    ///
    /// # Returns
    ///
    /// Vector of location IDs within the radius
    pub fn within_radius(&self, point: Point<f64>, radius_meters: f64) -> crate::Result<Vec<LocationId>> {
        // For now, use brute force - H3 candidate search may miss locations at cell boundaries
        // TODO: Improve H3 grid search to cover all cells within radius
        let within: Vec<LocationId> = self
            .locations
            .iter()
            .filter_map(|(id, loc)| {
                let dist = Haversine.distance(point.into(), loc.position().into());
                if dist <= radius_meters {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        Ok(within)
    }

    /// Get all locations of a specific type
    pub fn get_by_type(&self, location_type: &str) -> Vec<&L> {
        self.locations
            .values()
            .filter(|loc| loc.location_type() == location_type)
            .collect()
    }

    /// Iterate over all locations
    pub fn iter(&self) -> impl Iterator<Item = &L> {
        self.locations.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestLocation {
        id: LocationId,
        position: Point<f64>,
        location_type: String,
    }

    impl Location for TestLocation {
        fn id(&self) -> LocationId {
            self.id
        }

        fn location_type(&self) -> &str {
            &self.location_type
        }

        fn position(&self) -> Point<f64> {
            self.position
        }
    }

    #[test]
    fn test_add_and_get_location() {
        let mut handler = LocationHandler::new(Resolution::Nine);

        let location = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.4194, 37.7749),
            location_type: "warehouse".to_string(),
        };
        let id = location.id();

        handler.add_location(location).unwrap();

        assert_eq!(handler.count(), 1);
        assert!(handler.get(id).is_some());
    }

    #[test]
    fn test_nearest_location() {
        let mut handler = LocationHandler::new(Resolution::Nine);

        let loc1 = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.4194, 37.7749), // San Francisco
            location_type: "warehouse".to_string(),
        };
        let loc2 = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.0842, 37.4224), // Palo Alto
            location_type: "warehouse".to_string(),
        };

        let id1 = loc1.id();
        let _id2 = loc2.id();

        handler.add_location(loc1).unwrap();
        handler.add_location(loc2).unwrap();

        // Point closer to San Francisco
        let search_point = Point::new(-122.4, 37.78);
        let nearest = handler.nearest(search_point).unwrap().unwrap();

        assert_eq!(nearest, id1);
    }

    #[test]
    fn test_within_radius() {
        let mut handler = LocationHandler::new(Resolution::Nine);

        let loc1 = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.4194, 37.7749),
            location_type: "warehouse".to_string(),
        };
        let loc2 = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.4, 37.78), // ~2km away
            location_type: "warehouse".to_string(),
        };
        let loc3 = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.0, 37.5), // ~50km away
            location_type: "warehouse".to_string(),
        };

        handler.add_location(loc1).unwrap();
        handler.add_location(loc2).unwrap();
        handler.add_location(loc3).unwrap();

        let center = Point::new(-122.4194, 37.7749);
        let within = handler.within_radius(center, 5000.0).unwrap(); // 5km radius

        assert_eq!(within.len(), 2); // Should include loc1 and loc2, not loc3
    }

    #[test]
    fn test_get_by_type() {
        let mut handler = LocationHandler::new(Resolution::Nine);

        let warehouse = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.4, 37.78),
            location_type: "warehouse".to_string(),
        };
        let store = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.5, 37.79),
            location_type: "store".to_string(),
        };

        handler.add_location(warehouse).unwrap();
        handler.add_location(store).unwrap();

        let warehouses = handler.get_by_type("warehouse");
        assert_eq!(warehouses.len(), 1);

        let stores = handler.get_by_type("store");
        assert_eq!(stores.len(), 1);
    }
}
