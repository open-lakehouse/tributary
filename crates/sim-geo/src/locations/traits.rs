//! Location trait

use geo_types::Point;
use h3o::{CellIndex, LatLng, Resolution};
use sim_core::LocationId;
use std::fmt;

/// Validate geographic coordinates
///
/// # Arguments
///
/// * `lat` - Latitude in degrees
/// * `lon` - Longitude in degrees
///
/// # Returns
///
/// Ok(()) if valid, or an error if coordinates are out of bounds
pub fn validate_coordinates(lat: f64, lon: f64) -> crate::Result<()> {
    if !(-90.0..=90.0).contains(&lat) {
        return Err(crate::Error::Location(format!(
            "Invalid latitude: {}. Must be between -90 and 90 degrees.",
            lat
        )));
    }
    if !(-180.0..=180.0).contains(&lon) {
        return Err(crate::Error::Location(format!(
            "Invalid longitude: {}. Must be between -180 and 180 degrees.",
            lon
        )));
    }
    Ok(())
}

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

        // Validate coordinates first
        validate_coordinates(pos.y(), pos.x())?;

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

    #[test]
    fn test_validate_coordinates_valid() {
        // Valid coordinates
        assert!(validate_coordinates(0.0, 0.0).is_ok());
        assert!(validate_coordinates(37.7749, -122.4194).is_ok()); // San Francisco
        assert!(validate_coordinates(-33.8688, 151.2093).is_ok()); // Sydney
        assert!(validate_coordinates(90.0, 180.0).is_ok()); // Edge cases (valid)
        assert!(validate_coordinates(-90.0, -180.0).is_ok()); // Edge cases (valid)
    }

    #[test]
    fn test_validate_coordinates_invalid_latitude() {
        // Latitude out of bounds
        let result = validate_coordinates(200.0, 0.0);
        assert!(result.is_err());
        if let Err(crate::Error::Location(msg)) = result {
            assert!(msg.contains("Invalid latitude"));
            assert!(msg.contains("200"));
        } else {
            panic!("Expected Location error for invalid latitude");
        }

        let result = validate_coordinates(-100.0, 0.0);
        assert!(result.is_err());
        if let Err(crate::Error::Location(msg)) = result {
            assert!(msg.contains("Invalid latitude"));
        } else {
            panic!("Expected Location error for invalid latitude");
        }
    }

    #[test]
    fn test_validate_coordinates_invalid_longitude() {
        // Longitude out of bounds
        let result = validate_coordinates(0.0, 400.0);
        assert!(result.is_err());
        if let Err(crate::Error::Location(msg)) = result {
            assert!(msg.contains("Invalid longitude"));
            assert!(msg.contains("400"));
        } else {
            panic!("Expected Location error for invalid longitude");
        }

        let result = validate_coordinates(0.0, -200.0);
        assert!(result.is_err());
        if let Err(crate::Error::Location(msg)) = result {
            assert!(msg.contains("Invalid longitude"));
        } else {
            panic!("Expected Location error for invalid longitude");
        }
    }

    #[test]
    fn test_location_h3_cell_invalid_coordinates() {
        // Test that h3_cell properly validates coordinates
        let location = TestLocation {
            id: LocationId::new(),
            position: Point::new(400.0, 200.0), // Invalid coordinates
        };

        let result = location.h3_cell(Resolution::Nine);
        assert!(result.is_err());
        if let Err(crate::Error::Location(msg)) = result {
            assert!(msg.contains("Invalid latitude") || msg.contains("Invalid longitude"));
        } else {
            panic!("Expected Location error for invalid coordinates");
        }
    }
}
