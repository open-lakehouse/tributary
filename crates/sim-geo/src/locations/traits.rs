//! Location trait

use geo_types::Point;
use h3o::{CellIndex, LatLng, Resolution};
use sim_core::LocationId;
use std::fmt;

/// A location in the simulation world
///
/// Locations represent places where agents can be or travel to/from.
/// They support spatial indexing via H3 for efficient proximity queries.
///
/// # Example
///
/// ```rust,ignore
/// use sim_geo::{Location, LocationId};
/// use geo_types::Point;
/// use h3o::{CellIndex, Resolution};
///
/// struct Warehouse {
///     id: LocationId,
///     position: Point<f64>,
///     name: String,
///     capacity: usize,
/// }
///
/// impl Location for Warehouse {
///     fn id(&self) -> LocationId {
///         self.id
///     }
///
///     fn location_type(&self) -> &str {
///         "warehouse"
///     }
///
///     fn position(&self) -> Point<f64> {
///         self.position
///     }
/// }
/// ```
pub trait Location: Send + Sync + fmt::Debug {
    /// Unique identifier for this location
    fn id(&self) -> LocationId;

    /// Type of location (e.g., "warehouse", "kitchen", "store", "depot")
    fn location_type(&self) -> &str;

    /// Geographic position (longitude, latitude)
    fn position(&self) -> Point<f64>;

    /// Get H3 cell index at specified resolution
    ///
    /// This is used for spatial indexing and proximity queries.
    /// Higher resolution = smaller cells (more precise).
    ///
    /// Typical resolutions:
    /// - 5: ~252 km² (city-level)
    /// - 7: ~5.16 km² (neighborhood)
    /// - 9: ~0.105 km² (block-level)
    /// - 11: ~4,250 m² (building-level)
    fn h3_cell(&self, resolution: Resolution) -> crate::Result<CellIndex> {
        let pos = self.position();
        let latlng = LatLng::new(pos.y(), pos.x())
            .map_err(|e| crate::Error::H3(format!("Invalid coordinates: {}", e)))?;

        Ok(latlng.to_cell(resolution))
    }

    /// Optional capacity constraint
    ///
    /// If this location has limited capacity (e.g., number of loading docks,
    /// checkout lanes, parking spaces), return Some(capacity).
    fn capacity(&self) -> Option<usize> {
        None
    }

    /// Optional custom properties
    ///
    /// Domain-specific metadata can be stored here.
    fn properties(&self) -> serde_json::Value {
        serde_json::Value::Object(serde_json::Map::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestLocation {
        id: LocationId,
        position: Point<f64>,
    }

    impl Location for TestLocation {
        fn id(&self) -> LocationId {
            self.id
        }

        fn location_type(&self) -> &str {
            "test"
        }

        fn position(&self) -> Point<f64> {
            self.position
        }
    }

    #[test]
    fn test_location_h3_cell() {
        let location = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.4194, 37.7749), // San Francisco
        };

        let cell = location.h3_cell(Resolution::Nine).unwrap();
        // Just verify we got a cell - CellIndex doesn't have is_valid() in h3o 0.9
        assert!(cell.resolution() == Resolution::Nine);
    }

    #[test]
    fn test_location_defaults() {
        let location = TestLocation {
            id: LocationId::new(),
            position: Point::new(0.0, 0.0),
        };

        assert_eq!(location.location_type(), "test");
        assert_eq!(location.capacity(), None);
        assert!(location.properties().is_object());
    }
}
