//! GeoArrow-compliant schema for location storage
//!
//! This module defines the Arrow schema for storing locations with GeoArrow geometry columns.
//! The schema follows GeoArrow specifications for interoperability with GIS tools.
//!
//! # Schema Design
//!
//! - **id**: Binary field storing LocationId UUID (16 bytes)
//! - **name**: Optional UTF8 string for location name
//! - **location_type**: UTF8 string for type identifier (e.g., "warehouse", "store")
//! - **geometry**: Binary field storing WKB-encoded geometry (Point, Polygon, etc.)
//! - **latitude**: Float64 for fast coordinate access (denormalized)
//! - **longitude**: Float64 for fast coordinate access (denormalized)
//! - **h3_index**: UInt64 storing H3 cell index for spatial queries
//! - **metadata**: Optional UTF8 field for JSON-encoded domain-specific properties
//!
//! # GeoArrow Compliance
//!
//! The `geometry` column uses WKB (Well-Known Binary) format, which is the standard
//! GeoArrow encoding for geometry data. This enables:
//! - Compatibility with tools like QGIS, PostGIS, and GeoPandas
//! - Support for multiple geometry types (Point, LineString, Polygon, etc.)
//! - Efficient binary storage and transmission
//!
//! # Performance Strategy
//!
//! We use a dual-indexing approach:
//! 1. **RecordBatch**: Columnar storage for efficient scanning and filtering
//! 2. **H3 Index**: Spatial index (HashMap<H3Cell, Vec<row_idx>>) for O(1) proximity queries
//!
//! This combination provides:
//! - O(1) spatial lookups via H3 index
//! - Efficient batch operations via Arrow
//! - Memory-efficient columnar storage
//! - SIMD-friendly data layout

use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use std::sync::Arc;

/// Create the GeoArrow-compliant schema for location storage
///
/// # Schema Fields
///
/// | Field         | Type     | Nullable | Description                          |
/// |---------------|----------|----------|--------------------------------------|
/// | id            | Binary   | No       | LocationId UUID (16 bytes)           |
/// | name          | Utf8     | Yes      | Optional location name               |
/// | location_type | Utf8     | No       | Type identifier (e.g., "warehouse")  |
/// | geometry      | Binary   | No       | WKB-encoded geometry (GeoArrow)      |
/// | latitude      | Float64  | No       | Latitude for fast access             |
/// | longitude     | Float64  | No       | Longitude for fast access            |
/// | h3_index      | UInt64   | No       | H3 cell index for spatial queries    |
/// | metadata      | Utf8     | Yes      | JSON-encoded domain properties       |
///
/// # Example
///
/// ```rust
/// use sim_geo::locations::schema::location_schema;
///
/// let schema = location_schema();
/// assert_eq!(schema.fields().len(), 8);
/// ```
pub fn location_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        // Primary key: LocationId as 16-byte UUID
        Field::new("id", DataType::Binary, false),

        // Optional human-readable name
        Field::new("name", DataType::Utf8, true),

        // Type identifier for filtering (e.g., "warehouse", "store", "depot")
        Field::new("location_type", DataType::Utf8, false),

        // GeoArrow geometry column: WKB (Well-Known Binary) format
        // Supports Point, LineString, Polygon, MultiPoint, MultiLineString, MultiPolygon
        Field::new("geometry", DataType::Binary, false)
            .with_metadata([
                ("ARROW:extension:name".to_string(), "geoarrow.wkb".to_string()),
                ("ARROW:extension:metadata".to_string(), "{}".to_string()),
            ].into_iter().collect()),

        // Denormalized coordinates for fast access without WKB decoding
        Field::new("latitude", DataType::Float64, false),
        Field::new("longitude", DataType::Float64, false),

        // H3 spatial index: cell index as u64
        Field::new("h3_index", DataType::UInt64, false),

        // Domain-specific metadata as JSON string
        // Examples: {"capacity": 100, "hours": "9-5", "zone": "A"}
        Field::new("metadata", DataType::Utf8, true),
    ]))
}

/// Get the index of a field in the schema
///
/// # Arguments
///
/// * `field_name` - Name of the field to find
///
/// # Returns
///
/// Index of the field, or None if not found
///
/// # Example
///
/// ```rust
/// use sim_geo::locations::schema::{location_schema, field_index};
///
/// let schema = location_schema();
/// let idx = field_index(&schema, "latitude").unwrap();
/// assert_eq!(idx, 4);
/// ```
pub fn field_index(schema: &Schema, field_name: &str) -> Option<usize> {
    schema.fields().iter().position(|f| f.name() == field_name)
}

/// Field name constants for type-safe schema access
pub mod fields {
    pub const ID: &str = "id";
    pub const NAME: &str = "name";
    pub const LOCATION_TYPE: &str = "location_type";
    pub const GEOMETRY: &str = "geometry";
    pub const LATITUDE: &str = "latitude";
    pub const LONGITUDE: &str = "longitude";
    pub const H3_INDEX: &str = "h3_index";
    pub const METADATA: &str = "metadata";
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::datatypes::DataType;

    #[test]
    fn test_schema_creation() {
        let schema = location_schema();
        assert_eq!(schema.fields().len(), 8);
    }

    #[test]
    fn test_schema_field_types() {
        let schema = location_schema();

        // Check field names and types
        let fields: Vec<_> = schema.fields().iter()
            .map(|f| (f.name().as_str(), f.data_type()))
            .collect();

        assert_eq!(fields[0], ("id", &DataType::Binary));
        assert_eq!(fields[1], ("name", &DataType::Utf8));
        assert_eq!(fields[2], ("location_type", &DataType::Utf8));
        assert_eq!(fields[3], ("geometry", &DataType::Binary));
        assert_eq!(fields[4], ("latitude", &DataType::Float64));
        assert_eq!(fields[5], ("longitude", &DataType::Float64));
        assert_eq!(fields[6], ("h3_index", &DataType::UInt64));
        assert_eq!(fields[7], ("metadata", &DataType::Utf8));
    }

    #[test]
    fn test_schema_nullability() {
        let schema = location_schema();

        // Check nullable fields
        assert!(!schema.field(0).is_nullable()); // id
        assert!(schema.field(1).is_nullable());  // name
        assert!(!schema.field(2).is_nullable()); // location_type
        assert!(!schema.field(3).is_nullable()); // geometry
        assert!(!schema.field(4).is_nullable()); // latitude
        assert!(!schema.field(5).is_nullable()); // longitude
        assert!(!schema.field(6).is_nullable()); // h3_index
        assert!(schema.field(7).is_nullable());  // metadata
    }

    #[test]
    fn test_geometry_field_metadata() {
        let schema = location_schema();
        let geometry_field = schema.field(3);

        // Check GeoArrow extension metadata
        let metadata = geometry_field.metadata();
        assert_eq!(
            metadata.get("ARROW:extension:name"),
            Some(&"geoarrow.wkb".to_string())
        );
    }

    #[test]
    fn test_field_index_lookup() {
        let schema = location_schema();

        assert_eq!(field_index(&schema, "id"), Some(0));
        assert_eq!(field_index(&schema, "name"), Some(1));
        assert_eq!(field_index(&schema, "location_type"), Some(2));
        assert_eq!(field_index(&schema, "geometry"), Some(3));
        assert_eq!(field_index(&schema, "latitude"), Some(4));
        assert_eq!(field_index(&schema, "longitude"), Some(5));
        assert_eq!(field_index(&schema, "h3_index"), Some(6));
        assert_eq!(field_index(&schema, "metadata"), Some(7));
        assert_eq!(field_index(&schema, "nonexistent"), None);
    }

    #[test]
    fn test_field_constants() {
        use super::fields::*;

        let schema = location_schema();

        assert_eq!(field_index(&schema, ID), Some(0));
        assert_eq!(field_index(&schema, NAME), Some(1));
        assert_eq!(field_index(&schema, LOCATION_TYPE), Some(2));
        assert_eq!(field_index(&schema, GEOMETRY), Some(3));
        assert_eq!(field_index(&schema, LATITUDE), Some(4));
        assert_eq!(field_index(&schema, LONGITUDE), Some(5));
        assert_eq!(field_index(&schema, H3_INDEX), Some(6));
        assert_eq!(field_index(&schema, METADATA), Some(7));
    }
}
