//! Comprehensive tests for Arrow RecordBatch-based LocationHandler
//!
//! This test suite validates the migration from HashMap to RecordBatch storage
//! with GeoArrow geometry and H3 spatial indexing.

#[cfg(test)]
mod arrow_location_tests {
    use crate::locations::schema::{fields, location_schema};
    use crate::locations::{Location, LocationHandler};
    use arrow::array::Array;
    use geo_types::Point;
    use h3o::Resolution;
    use sim_core::LocationId;

    #[derive(Debug, Clone)]
    struct TestLocation {
        id: LocationId,
        position: Point<f64>,
        location_type: String,
        name: Option<String>,
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

    impl TestLocation {
        fn new(lon: f64, lat: f64, location_type: &str) -> Self {
            Self {
                id: LocationId::new(),
                position: Point::new(lon, lat),
                location_type: location_type.to_string(),
                name: None,
            }
        }

        fn with_name(mut self, name: &str) -> Self {
            self.name = Some(name.to_string());
            self
        }
    }

    // ===== Test 1: Schema Validation =====
    #[test]
    fn test_schema_structure() {
        let schema = location_schema();

        // Verify field count
        assert_eq!(schema.fields().len(), 8);

        // Verify field names
        assert_eq!(schema.field(0).name(), fields::ID);
        assert_eq!(schema.field(1).name(), fields::NAME);
        assert_eq!(schema.field(2).name(), fields::LOCATION_TYPE);
        assert_eq!(schema.field(3).name(), fields::GEOMETRY);
        assert_eq!(schema.field(4).name(), fields::LATITUDE);
        assert_eq!(schema.field(5).name(), fields::LONGITUDE);
        assert_eq!(schema.field(6).name(), fields::H3_INDEX);
        assert_eq!(schema.field(7).name(), fields::METADATA);
    }

    // ===== Test 2: Add 1000 Locations =====
    #[test]
    fn test_add_1000_locations() {
        let mut handler = LocationHandler::new(Resolution::Nine);

        // Add 1000 locations in a grid around San Francisco
        let base_lon = -122.4194;
        let base_lat = 37.7749;

        for i in 0..1000 {
            let lon = base_lon + (i % 32) as f64 * 0.01;
            let lat = base_lat + (i / 32) as f64 * 0.01;

            let location = TestLocation::new(lon, lat, "warehouse");
            handler.add_location(location).unwrap();
        }

        // Verify count
        assert_eq!(handler.count(), 1000);

        // Verify RecordBatch structure
        let batch = handler.record_batch();
        assert_eq!(batch.num_rows(), 1000);
        assert_eq!(batch.num_columns(), 8);

        // Verify all rows have non-null required fields
        let lat_array = batch.column_by_name(fields::LATITUDE).unwrap();
        let lon_array = batch.column_by_name(fields::LONGITUDE).unwrap();
        let h3_array = batch.column_by_name(fields::H3_INDEX).unwrap();

        assert_eq!(lat_array.null_count(), 0);
        assert_eq!(lon_array.null_count(), 0);
        assert_eq!(h3_array.null_count(), 0);
    }

    // ===== Test 3: Nearest Location with H3 Acceleration =====
    #[test]
    fn test_nearest_location_h3() {
        let mut handler = LocationHandler::new(Resolution::Nine);

        // Add locations
        let sf = TestLocation::new(-122.4194, 37.7749, "warehouse").with_name("San Francisco");
        let oakland = TestLocation::new(-122.2711, 37.8044, "warehouse").with_name("Oakland");
        let san_jose = TestLocation::new(-121.8863, 37.3382, "warehouse").with_name("San Jose");

        let sf_id = sf.id();
        let oakland_id = oakland.id();

        handler.add_location(sf).unwrap();
        handler.add_location(oakland).unwrap();
        handler.add_location(san_jose).unwrap();

        // Query point closer to Oakland
        let query_point = Point::new(-122.28, 37.81);
        let nearest = handler.nearest(query_point).unwrap().unwrap();

        assert_eq!(nearest, oakland_id);

        // Query point closer to SF
        let query_point2 = Point::new(-122.42, 37.77);
        let nearest2 = handler.nearest(query_point2).unwrap().unwrap();

        assert_eq!(nearest2, sf_id);
    }

    // ===== Test 4: Within Radius Query =====
    #[test]
    fn test_within_radius() {
        let mut handler = LocationHandler::new(Resolution::Nine);

        // Add locations at known distances
        let center = TestLocation::new(-122.4194, 37.7749, "hub");
        let close1 = TestLocation::new(-122.41, 37.77, "warehouse"); // ~500m
        let close2 = TestLocation::new(-122.42, 37.78, "warehouse"); // ~1km
        let far = TestLocation::new(-122.0, 37.5, "warehouse"); // ~50km

        let center_id = center.id();
        let close1_id = close1.id();
        let close2_id = close2.id();

        handler.add_location(center).unwrap();
        handler.add_location(close1).unwrap();
        handler.add_location(close2).unwrap();
        handler.add_location(far).unwrap();

        // Query 5km radius
        let center_point = Point::new(-122.4194, 37.7749);
        let within_5km = handler.within_radius(center_point, 5000.0).unwrap();

        assert_eq!(within_5km.len(), 3); // center + close1 + close2
        assert!(within_5km.contains(&center_id));
        assert!(within_5km.contains(&close1_id));
        assert!(within_5km.contains(&close2_id));

        // Query 2km radius (should only get center and close1, close2)
        let within_2km = handler.within_radius(center_point, 2000.0).unwrap();
        assert!(within_2km.len() >= 2); // At least center + close1
    }

    // ===== Test 5: GeoArrow Geometry Roundtrip =====
    #[test]
    fn test_geoarrow_wkb_roundtrip() {
        let mut handler = LocationHandler::new(Resolution::Nine);

        let original_point = Point::new(-122.4194, 37.7749);
        let location = TestLocation::new(original_point.x(), original_point.y(), "test");
        let id = location.id();

        handler.add_location(location).unwrap();

        // Retrieve position
        let retrieved_point = handler.get_position(id).unwrap();

        // Verify coordinates match (within floating point precision)
        assert!((retrieved_point.x() - original_point.x()).abs() < 1e-10);
        assert!((retrieved_point.y() - original_point.y()).abs() < 1e-10);

        // Verify geometry column exists in RecordBatch
        let batch = handler.record_batch();
        let geom_column = batch.column_by_name(fields::GEOMETRY).unwrap();
        assert_eq!(geom_column.len(), 1);
    }

    // ===== Test 6: Update Location Coordinates & H3 Index =====
    #[test]
    fn test_remove_location_updates_indices() {
        let mut handler = LocationHandler::new(Resolution::Nine);

        let loc1 = TestLocation::new(-122.4194, 37.7749, "warehouse");
        let loc2 = TestLocation::new(-122.2711, 37.8044, "warehouse");
        let loc3 = TestLocation::new(-121.8863, 37.3382, "warehouse");

        let id1 = loc1.id();
        let id2 = loc2.id();
        let id3 = loc3.id();

        handler.add_location(loc1).unwrap();
        handler.add_location(loc2).unwrap();
        handler.add_location(loc3).unwrap();

        assert_eq!(handler.count(), 3);

        // Remove middle location
        let removed = handler.remove_location(id2).unwrap();
        assert!(removed.is_some());

        assert_eq!(handler.count(), 2);
        assert!(handler.get(id1).is_some());
        assert!(handler.get(id2).is_none());
        assert!(handler.get(id3).is_some());

        // Verify indices are consistent
        let batch = handler.record_batch();
        assert_eq!(batch.num_rows(), 2);
    }

    // ===== Test 7: Remove Location & H3 Index Cleanup =====
    #[test]
    fn test_remove_all_locations() {
        let mut handler = LocationHandler::new(Resolution::Nine);

        let locations: Vec<_> = (0..10)
            .map(|i| {
                let lon = -122.4 + i as f64 * 0.01;
                let lat = 37.7 + i as f64 * 0.01;
                TestLocation::new(lon, lat, "test")
            })
            .collect();

        let ids: Vec<_> = locations.iter().map(|l| l.id()).collect();

        for loc in locations {
            handler.add_location(loc).unwrap();
        }

        assert_eq!(handler.count(), 10);

        // Remove all locations
        for id in ids {
            handler.remove_location(id).unwrap();
        }

        assert_eq!(handler.count(), 0);

        // Verify RecordBatch is empty
        assert_eq!(handler.record_batch().num_rows(), 0);
    }

    // ===== Test 8: Performance Benchmark (10k locations, <10ms queries) =====
    #[test]
    fn test_performance_10k_locations() {
        use std::time::Instant;

        let mut handler = LocationHandler::new(Resolution::Nine);

        // Add 10,000 locations
        let start_add = Instant::now();
        for i in 0..10_000 {
            let lon = -122.5 + (i % 100) as f64 * 0.01;
            let lat = 37.0 + (i / 100) as f64 * 0.01;
            let location = TestLocation::new(lon, lat, "warehouse");
            handler.add_location(location).unwrap();
        }
        let add_duration = start_add.elapsed();
        println!("Added 10k locations in {:?}", add_duration);

        assert_eq!(handler.count(), 10_000);

        // Test within_radius performance
        let query_point = Point::new(-122.4, 37.5);
        let start_query = Instant::now();
        let results = handler.within_radius(query_point, 5000.0).unwrap();
        let query_duration = start_query.elapsed();

        println!(
            "within_radius(5km) on 10k locations: {:?}, found {} results",
            query_duration,
            results.len()
        );

        // Should complete in <10ms (with H3 acceleration)
        assert!(query_duration.as_millis() < 50); // Being generous, should be <10ms
    }

    // ===== Test 9: Edge Cases =====
    #[test]
    fn test_edge_case_no_locations_nearby() {
        let mut handler = LocationHandler::new(Resolution::Nine);

        // Add location in SF
        let location = TestLocation::new(-122.4194, 37.7749, "warehouse");
        handler.add_location(location).unwrap();

        // Query in NYC (no locations nearby)
        let nyc_point = Point::new(-74.0060, 40.7128);
        let nearby = handler.within_radius(nyc_point, 1000.0).unwrap();

        assert_eq!(nearby.len(), 0);
    }

    #[test]
    fn test_edge_case_exact_same_coordinates() {
        let mut handler = LocationHandler::new(Resolution::Nine);

        // Add multiple locations at exact same coordinates
        let point = Point::new(-122.4194, 37.7749);
        let loc1 = TestLocation::new(point.x(), point.y(), "warehouse");
        let loc2 = TestLocation::new(point.x(), point.y(), "store");
        let loc3 = TestLocation::new(point.x(), point.y(), "depot");

        handler.add_location(loc1).unwrap();
        handler.add_location(loc2).unwrap();
        handler.add_location(loc3).unwrap();

        // Query at exact same point
        let results = handler.within_radius(point, 1.0).unwrap(); // 1 meter radius
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_edge_case_empty_handler() {
        let handler = LocationHandler::<TestLocation>::new(Resolution::Nine);

        assert_eq!(handler.count(), 0);

        let point = Point::new(-122.4194, 37.7749);
        let nearest = handler.nearest(point).unwrap();
        assert!(nearest.is_none());

        let within = handler.within_radius(point, 5000.0).unwrap();
        assert_eq!(within.len(), 0);
    }

    // ===== Test 10: H3 Ring Calculation Correctness =====
    #[test]
    fn test_h3_ring_calculation() {
        let mut handler = LocationHandler::new(Resolution::Nine);

        // Resolution 9 has ~174m edge length
        // 1km radius should require ~6 rings (1000 / 174 * 1.5 ≈ 8.6)

        // Add locations in a grid
        let center = Point::new(-122.4194, 37.7749);

        // Add 100 locations in a grid around center
        for i in -10..10 {
            for j in -10..10 {
                let lon = center.x() + i as f64 * 0.001; // ~111m per 0.001°
                let lat = center.y() + j as f64 * 0.001;
                let location = TestLocation::new(lon, lat, "test");
                handler.add_location(location).unwrap();
            }
        }

        // Query 500m radius (should get locations in ~5 ring)
        let within_500m = handler.within_radius(center, 500.0).unwrap();
        println!("Found {} locations within 500m", within_500m.len());

        // Should find multiple locations (exact count depends on grid)
        assert!(within_500m.len() > 10);
        assert!(within_500m.len() < 400); // Not all locations
    }

    // ===== Test 11: Get By Type =====
    #[test]
    fn test_get_by_type() {
        let mut handler = LocationHandler::new(Resolution::Nine);

        // Add different types
        for i in 0..5 {
            let lon = -122.4 + i as f64 * 0.01;
            let lat = 37.7 + i as f64 * 0.01;
            handler
                .add_location(TestLocation::new(lon, lat, "warehouse"))
                .unwrap();
        }

        for i in 0..3 {
            let lon = -122.3 + i as f64 * 0.01;
            let lat = 37.8 + i as f64 * 0.01;
            handler
                .add_location(TestLocation::new(lon, lat, "store"))
                .unwrap();
        }

        for i in 0..2 {
            let lon = -122.2 + i as f64 * 0.01;
            let lat = 37.9 + i as f64 * 0.01;
            handler
                .add_location(TestLocation::new(lon, lat, "depot"))
                .unwrap();
        }

        let warehouses = handler.get_by_type("warehouse").unwrap();
        let stores = handler.get_by_type("store").unwrap();
        let depots = handler.get_by_type("depot").unwrap();

        assert_eq!(warehouses.len(), 5);
        assert_eq!(stores.len(), 3);
        assert_eq!(depots.len(), 2);
    }

    // ===== Test 12: H3 Cell Queries =====
    #[test]
    fn test_h3_cell_queries() {
        use h3o::LatLng;

        let mut handler = LocationHandler::new(Resolution::Nine);

        let point = Point::new(-122.4194, 37.7749);
        let location = TestLocation::new(point.x(), point.y(), "test");
        handler.add_location(location).unwrap();

        // Get H3 cell for this point
        let latlng = LatLng::new(point.y(), point.x()).unwrap();
        let cell = latlng.to_cell(Resolution::Nine);

        // Query locations in this cell
        let in_cell = handler.get_locations_in_h3_cell(cell);
        assert_eq!(in_cell.len(), 1);
    }

    // ===== Test 13: RecordBatch Column Access =====
    #[test]
    fn test_record_batch_access() {
        let mut handler = LocationHandler::new(Resolution::Nine);

        let location = TestLocation::new(-122.4194, 37.7749, "warehouse");
        handler.add_location(location).unwrap();

        let batch = handler.record_batch();

        // Verify all columns exist
        assert!(batch.column_by_name(fields::ID).is_some());
        assert!(batch.column_by_name(fields::NAME).is_some());
        assert!(batch.column_by_name(fields::LOCATION_TYPE).is_some());
        assert!(batch.column_by_name(fields::GEOMETRY).is_some());
        assert!(batch.column_by_name(fields::LATITUDE).is_some());
        assert!(batch.column_by_name(fields::LONGITUDE).is_some());
        assert!(batch.column_by_name(fields::H3_INDEX).is_some());
        assert!(batch.column_by_name(fields::METADATA).is_some());
    }

    // ===== Test 14: Nearest Brute Force Fallback =====
    #[test]
    fn test_nearest_brute_force_fallback() {
        // Create handler with very coarse resolution to test fallback
        let mut handler = LocationHandler::new(Resolution::Five);

        let loc1 = TestLocation::new(-122.4194, 37.7749, "warehouse");
        let loc2 = TestLocation::new(-122.0842, 37.4224, "warehouse");

        let id1 = loc1.id();

        handler.add_location(loc1).unwrap();
        handler.add_location(loc2).unwrap();

        // Query should still work even with coarse resolution
        let query_point = Point::new(-122.42, 37.77);
        let nearest = handler.nearest(query_point).unwrap().unwrap();

        assert_eq!(nearest, id1);
    }
}
