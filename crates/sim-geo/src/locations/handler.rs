//! Location handler implementation with Arrow RecordBatch storage
//!
//! This module provides efficient location management using Apache Arrow's columnar
//! storage combined with H3 spatial indexing for fast geospatial queries.

use super::schema::{location_schema, fields};
use super::traits::Location;
use arrow::array::{
    Array, ArrayRef, BinaryArray, BinaryBuilder, Float64Array, Float64Builder,
    RecordBatch, StringArray, StringBuilder, UInt64Array, UInt64Builder,
};
use geo::Distance;
use geo::Haversine;
use geo_types::Point;
use h3o::{CellIndex, LatLng, Resolution};
use sim_core::LocationId;
use std::collections::HashMap;
use std::sync::Arc;

/// Manages locations in the simulation using Apache Arrow storage
///
/// Combines RecordBatch columnar storage with H3 spatial indexing for:
/// - Memory-efficient storage
/// - Fast batch operations
/// - O(1) spatial proximity queries
///
/// # Storage Strategy
///
/// - **RecordBatch**: Columnar storage for all location data
/// - **H3 Index**: HashMap<H3Cell, Vec<row_idx>> for spatial queries
/// - **LocationId Index**: HashMap<LocationId, row_idx> for O(1) lookups
///
/// # Type Parameters
///
/// * `L` - Location type implementing the Location trait
///
/// # Example
///
/// ```rust,ignore
/// use sim_geo::{LocationHandler, Location};
/// use h3o::Resolution;
///
/// let mut handler = LocationHandler::<MyLocation>::new(Resolution::Nine);
///
/// handler.add_location(warehouse1)?;
/// handler.add_location(warehouse2)?;
///
/// // Find nearest location to a point
/// let nearest = handler.nearest(my_position)?;
/// ```
pub struct LocationHandler<L: Location> {
    /// Arrow storage for all location data
    locations: RecordBatch,

    /// Schema reference
    schema: arrow::datatypes::SchemaRef,

    /// Index: LocationId -> row index in RecordBatch
    location_id_to_row: HashMap<LocationId, usize>,

    /// Spatial index: H3 cell -> row indices
    h3_index: HashMap<CellIndex, Vec<usize>>,

    /// H3 resolution for spatial indexing (7-10 typical for city-level)
    resolution: Resolution,

    /// Phantom data to preserve generic type
    _phantom: std::marker::PhantomData<L>,
}

impl<L: Location> LocationHandler<L> {
    /// Create a new location handler
    ///
    /// # Arguments
    ///
    /// * `resolution` - H3 resolution for spatial indexing
    ///   - Resolution::Seven: ~5.16 km² (neighborhood level)
    ///   - Resolution::Eight: ~0.74 km² (good for cities)
    ///   - Resolution::Nine: ~0.105 km² (block level, default)
    ///   - Resolution::Ten: ~0.015 km² (building level)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let handler = LocationHandler::<Warehouse>::new(Resolution::Nine);
    /// ```
    pub fn new(resolution: Resolution) -> Self {
        let schema = location_schema();

        // Create empty RecordBatch
        let empty_batch = RecordBatch::new_empty(schema.clone());

        Self {
            locations: empty_batch,
            schema,
            location_id_to_row: HashMap::new(),
            h3_index: HashMap::new(),
            resolution,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Add a location to the handler
    ///
    /// Appends the location to the RecordBatch and updates spatial indices.
    ///
    /// # Arguments
    ///
    /// * `location` - Location to add
    ///
    /// # Returns
    ///
    /// Ok(()) on success, or an error if the operation fails
    pub fn add_location(&mut self, location: L) -> crate::Result<()> {
        let id = location.id();
        let position = location.position();
        let location_type = location.location_type();

        // Calculate H3 cell
        let cell = location.h3_cell(self.resolution)?;
        let h3_value = u64::from(cell);

        // Encode geometry as WKB
        let geometry_wkb = point_to_wkb(&position)?;

        // Serialize metadata
        let metadata_json = serde_json::to_string(&location.properties())
            .map_err(|e| crate::Error::Location(format!("Failed to serialize metadata: {}", e)))?;

        // Get current row count (new row index)
        let new_row_idx = self.locations.num_rows();

        // Build new arrays with appended data
        let new_batch = self.append_to_batch(
            &id,
            None, // name (not in Location trait)
            location_type,
            &geometry_wkb,
            position.y(), // latitude
            position.x(), // longitude
            h3_value,
            &metadata_json,
        )?;

        // Update RecordBatch
        self.locations = new_batch;

        // Update LocationId index
        self.location_id_to_row.insert(id, new_row_idx);

        // Update H3 spatial index
        self.h3_index
            .entry(cell)
            .or_insert_with(Vec::new)
            .push(new_row_idx);

        Ok(())
    }

    /// Remove a location
    ///
    /// # Note
    ///
    /// This operation rebuilds the RecordBatch without the removed row,
    /// which is O(n). For frequent removals, consider marking as inactive instead.
    pub fn remove_location(&mut self, id: LocationId) -> crate::Result<Option<()>> {
        let row_idx = match self.location_id_to_row.remove(&id) {
            Some(idx) => idx,
            None => return Ok(None),
        };

        // Get H3 cell for this row to update spatial index
        if let Some(h3_value) = self.get_u64_from_batch(row_idx, fields::H3_INDEX)? {
            let cell = CellIndex::try_from(h3_value)
                .map_err(|e| crate::Error::H3(format!("Invalid H3 index: {}", e)))?;

            // Remove from H3 index
            if let Some(row_indices) = self.h3_index.get_mut(&cell) {
                row_indices.retain(|&idx| idx != row_idx);
                if row_indices.is_empty() {
                    self.h3_index.remove(&cell);
                }
            }
        }

        // Rebuild batch without this row
        self.rebuild_batch_without_row(row_idx)?;

        // Update all indices (row numbers have shifted)
        self.rebuild_indices();

        Ok(Some(()))
    }

    /// Get a location by ID (reconstructed from RecordBatch)
    ///
    /// # Note
    ///
    /// This returns None since we can't reconstruct the generic Location type L
    /// without additional trait methods. Use get_position(), get_type(), etc. instead.
    pub fn get(&self, id: LocationId) -> Option<LocationData> {
        let row_idx = self.location_id_to_row.get(&id)?;
        self.get_location_data(*row_idx).ok()
    }

    /// Get location position by ID
    pub fn get_position(&self, id: LocationId) -> Option<Point<f64>> {
        let row_idx = self.location_id_to_row.get(&id)?;
        self.get_point_from_batch(*row_idx).ok()
    }

    /// Get location type by ID
    pub fn get_type(&self, id: LocationId) -> Option<String> {
        let row_idx = self.location_id_to_row.get(&id)?;
        self.get_string_from_batch(*row_idx, fields::LOCATION_TYPE).ok()?
    }

    /// Get location count
    pub fn count(&self) -> usize {
        self.locations.num_rows()
    }

    /// Find the nearest location to a point using H3 acceleration
    ///
    /// # Arguments
    ///
    /// * `point` - Position to search from
    ///
    /// # Returns
    ///
    /// The ID of the nearest location, or None if no locations exist
    pub fn nearest(&self, point: Point<f64>) -> crate::Result<Option<LocationId>> {
        if self.locations.num_rows() == 0 {
            return Ok(None);
        }

        // Convert point to H3 cell
        let latlng = LatLng::new(point.y(), point.x())
            .map_err(|e| crate::Error::H3(format!("Invalid coordinates: {}", e)))?;
        let search_cell = latlng.to_cell(self.resolution);

        // Get candidate rows from H3 cell + 1-ring neighbors
        let mut candidates = Vec::new();

        // Add rows from the cell itself
        if let Some(rows) = self.h3_index.get(&search_cell) {
            candidates.extend(rows.iter().copied());
        }

        // Add rows from neighboring cells (1-ring)
        for neighbor in search_cell.grid_disk::<Vec<_>>(1) {
            if let Some(rows) = self.h3_index.get(&neighbor) {
                candidates.extend(rows.iter().copied());
            }
        }

        if candidates.is_empty() {
            // Fallback to brute force if no H3 candidates
            return self.nearest_brute_force(point);
        }

        // Find nearest among candidates using RecordBatch coordinates
        let mut nearest_id = None;
        let mut min_distance = f64::MAX;

        for row_idx in candidates {
            if let Ok(loc_point) = self.get_point_from_batch(row_idx) {
                let distance = Haversine.distance(point.into(), loc_point.into());

                if distance < min_distance {
                    min_distance = distance;
                    if let Ok(id) = self.get_location_id_from_batch(row_idx) {
                        nearest_id = Some(id);
                    }
                }
            }
        }

        Ok(nearest_id)
    }

    /// Brute force nearest location (fallback when H3 index is empty)
    fn nearest_brute_force(&self, point: Point<f64>) -> crate::Result<Option<LocationId>> {
        let mut nearest_id = None;
        let mut min_distance = f64::MAX;

        for row_idx in 0..self.locations.num_rows() {
            if let Ok(loc_point) = self.get_point_from_batch(row_idx) {
                let distance = Haversine.distance(point.into(), loc_point.into());

                if distance < min_distance {
                    min_distance = distance;
                    if let Ok(id) = self.get_location_id_from_batch(row_idx) {
                        nearest_id = Some(id);
                    }
                }
            }
        }

        Ok(nearest_id)
    }

    /// Find all locations within a radius using H3 acceleration
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
        // Calculate H3 ring size needed for radius
        let ring_size = calculate_h3_ring_for_radius(radius_meters, self.resolution);

        // Get H3 cell for search point
        let latlng = LatLng::new(point.y(), point.x())
            .map_err(|e| crate::Error::H3(format!("Invalid coordinates: {}", e)))?;
        let search_cell = latlng.to_cell(self.resolution);

        // Get all candidate rows from H3 rings
        let mut candidates = Vec::new();
        for cell in search_cell.grid_disk::<Vec<_>>(ring_size) {
            if let Some(rows) = self.h3_index.get(&cell) {
                candidates.extend(rows.iter().copied());
            }
        }

        // Filter by exact distance using RecordBatch coordinates
        let mut results = Vec::new();
        for row_idx in candidates {
            if let Ok(loc_point) = self.get_point_from_batch(row_idx) {
                let distance = Haversine.distance(point.into(), loc_point.into());

                if distance <= radius_meters {
                    if let Ok(id) = self.get_location_id_from_batch(row_idx) {
                        results.push(id);
                    }
                }
            }
        }

        Ok(results)
    }

    /// Get all locations of a specific type
    pub fn get_by_type(&self, location_type: &str) -> crate::Result<Vec<LocationId>> {
        let mut results = Vec::new();

        for row_idx in 0..self.locations.num_rows() {
            if let Ok(Some(row_type)) = self.get_string_from_batch(row_idx, fields::LOCATION_TYPE) {
                if row_type == location_type {
                    if let Ok(id) = self.get_location_id_from_batch(row_idx) {
                        results.push(id);
                    }
                }
            }
        }

        Ok(results)
    }

    /// Get all locations in a specific H3 cell
    pub fn get_locations_in_h3_cell(&self, cell: CellIndex) -> Vec<LocationId> {
        self.h3_index
            .get(&cell)
            .map(|rows| {
                rows.iter()
                    .filter_map(|&row_idx| self.get_location_id_from_batch(row_idx).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Iterate over all location IDs
    pub fn iter_ids(&self) -> impl Iterator<Item = LocationId> + '_ {
        self.location_id_to_row.keys().copied()
    }

    /// Get the H3 resolution used by this handler
    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    /// Get reference to the underlying RecordBatch
    pub fn record_batch(&self) -> &RecordBatch {
        &self.locations
    }

    /// Export the locations RecordBatch
    ///
    /// Returns a clone of the internal RecordBatch for serialization or persistence.
    /// This includes all location data with GeoArrow WKB geometry encoding.
    ///
    /// # Returns
    ///
    /// A RecordBatch containing all location data
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let batch = handler.export_batch();
    /// // Serialize batch using Arrow IPC or Parquet
    /// ```
    pub fn export_batch(&self) -> RecordBatch {
        self.locations.clone()
    }

    /// Import a RecordBatch and rebuild indices
    ///
    /// Restores the LocationHandler state from a previously exported RecordBatch.
    /// This method:
    /// 1. Validates the schema matches the expected location schema
    /// 2. Replaces the internal RecordBatch
    /// 3. Rebuilds the LocationId index (HashMap<LocationId, row_idx>)
    /// 4. Rebuilds the H3 spatial index (HashMap<H3Cell, Vec<row_idx>>)
    ///
    /// # Arguments
    ///
    /// * `batch` - RecordBatch to import (must match location schema)
    ///
    /// # Returns
    ///
    /// Ok(()) on success, or an error if:
    /// - Schema validation fails
    /// - Index rebuilding encounters invalid data
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let batch = load_batch_from_disk()?;
    /// handler.import_batch(batch)?;
    /// // All indices are rebuilt, spatial queries work immediately
    /// ```
    pub fn import_batch(&mut self, batch: RecordBatch) -> crate::Result<()> {
        // Validate schema compatibility
        if batch.schema() != self.schema {
            return Err(crate::Error::Location(
                "Imported batch schema does not match location schema".to_string()
            ));
        }

        // Replace the RecordBatch
        self.locations = batch;

        // Rebuild both indices from the new batch
        self.rebuild_indices();

        Ok(())
    }

    // ===== Helper methods for RecordBatch access =====

    fn get_location_id_from_batch(&self, row_idx: usize) -> crate::Result<LocationId> {
        let id_array = self.locations
            .column_by_name(fields::ID)
            .ok_or_else(|| crate::Error::Location("ID column not found".to_string()))?
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| crate::Error::Location("ID column has wrong type".to_string()))?;

        let uuid_bytes = id_array.value(row_idx);
        if uuid_bytes.len() != 16 {
            return Err(crate::Error::Location(format!(
                "Invalid UUID bytes length: {}",
                uuid_bytes.len()
            )));
        }

        LocationId::from_bytes(uuid_bytes)
            .map_err(|e| crate::Error::Custom(format!("Invalid location ID: {}", e)))
    }

    fn get_point_from_batch(&self, row_idx: usize) -> crate::Result<Point<f64>> {
        let lat = self.get_f64_from_batch(row_idx, fields::LATITUDE)?
            .ok_or_else(|| crate::Error::Location("Latitude is null".to_string()))?;

        let lon = self.get_f64_from_batch(row_idx, fields::LONGITUDE)?
            .ok_or_else(|| crate::Error::Location("Longitude is null".to_string()))?;

        Ok(Point::new(lon, lat))
    }

    fn get_f64_from_batch(&self, row_idx: usize, field_name: &str) -> crate::Result<Option<f64>> {
        let array = self.locations
            .column_by_name(field_name)
            .ok_or_else(|| crate::Error::Location(format!("{} column not found", field_name)))?
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| crate::Error::Location(format!("{} column has wrong type", field_name)))?;

        if array.is_null(row_idx) {
            Ok(None)
        } else {
            Ok(Some(array.value(row_idx)))
        }
    }

    fn get_u64_from_batch(&self, row_idx: usize, field_name: &str) -> crate::Result<Option<u64>> {
        let array = self.locations
            .column_by_name(field_name)
            .ok_or_else(|| crate::Error::Location(format!("{} column not found", field_name)))?
            .as_any()
            .downcast_ref::<UInt64Array>()
            .ok_or_else(|| crate::Error::Location(format!("{} column has wrong type", field_name)))?;

        if array.is_null(row_idx) {
            Ok(None)
        } else {
            Ok(Some(array.value(row_idx)))
        }
    }

    fn get_string_from_batch(&self, row_idx: usize, field_name: &str) -> crate::Result<Option<String>> {
        let array = self.locations
            .column_by_name(field_name)
            .ok_or_else(|| crate::Error::Location(format!("{} column not found", field_name)))?
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| crate::Error::Location(format!("{} column has wrong type", field_name)))?;

        if array.is_null(row_idx) {
            Ok(None)
        } else {
            Ok(Some(array.value(row_idx).to_string()))
        }
    }

    fn get_location_data(&self, row_idx: usize) -> crate::Result<LocationData> {
        Ok(LocationData {
            id: self.get_location_id_from_batch(row_idx)?,
            position: self.get_point_from_batch(row_idx)?,
            location_type: self.get_string_from_batch(row_idx, fields::LOCATION_TYPE)?
                .unwrap_or_default(),
            name: self.get_string_from_batch(row_idx, fields::NAME)?,
        })
    }

    fn append_to_batch(
        &self,
        id: &LocationId,
        name: Option<&str>,
        location_type: &str,
        geometry: &[u8],
        latitude: f64,
        longitude: f64,
        h3_index: u64,
        metadata: &str,
    ) -> crate::Result<RecordBatch> {
        // Get current arrays
        let current_count = self.locations.num_rows();

        // Build new arrays with appended data
        let mut id_builder = BinaryBuilder::new();
        let mut name_builder = StringBuilder::new();
        let mut type_builder = StringBuilder::new();
        let mut geom_builder = BinaryBuilder::new();
        let mut lat_builder = Float64Builder::new();
        let mut lon_builder = Float64Builder::new();
        let mut h3_builder = UInt64Builder::new();
        let mut meta_builder = StringBuilder::new();

        // Copy existing data
        if current_count > 0 {
            let id_array = self.locations.column(0).as_any().downcast_ref::<BinaryArray>().unwrap();
            let name_array = self.locations.column(1).as_any().downcast_ref::<StringArray>().unwrap();
            let type_array = self.locations.column(2).as_any().downcast_ref::<StringArray>().unwrap();
            let geom_array = self.locations.column(3).as_any().downcast_ref::<BinaryArray>().unwrap();
            let lat_array = self.locations.column(4).as_any().downcast_ref::<Float64Array>().unwrap();
            let lon_array = self.locations.column(5).as_any().downcast_ref::<Float64Array>().unwrap();
            let h3_array = self.locations.column(6).as_any().downcast_ref::<UInt64Array>().unwrap();
            let meta_array = self.locations.column(7).as_any().downcast_ref::<StringArray>().unwrap();

            for i in 0..current_count {
                id_builder.append_value(id_array.value(i));
                if name_array.is_null(i) {
                    name_builder.append_null();
                } else {
                    name_builder.append_value(name_array.value(i));
                }
                type_builder.append_value(type_array.value(i));
                geom_builder.append_value(geom_array.value(i));
                lat_builder.append_value(lat_array.value(i));
                lon_builder.append_value(lon_array.value(i));
                h3_builder.append_value(h3_array.value(i));
                if meta_array.is_null(i) {
                    meta_builder.append_null();
                } else {
                    meta_builder.append_value(meta_array.value(i));
                }
            }
        }

        // Append new data
        id_builder.append_value(id.as_bytes());
        if let Some(n) = name {
            name_builder.append_value(n);
        } else {
            name_builder.append_null();
        }
        type_builder.append_value(location_type);
        geom_builder.append_value(geometry);
        lat_builder.append_value(latitude);
        lon_builder.append_value(longitude);
        h3_builder.append_value(h3_index);
        meta_builder.append_value(metadata);

        // Build arrays
        let id_array: ArrayRef = Arc::new(id_builder.finish());
        let name_array: ArrayRef = Arc::new(name_builder.finish());
        let type_array: ArrayRef = Arc::new(type_builder.finish());
        let geom_array: ArrayRef = Arc::new(geom_builder.finish());
        let lat_array: ArrayRef = Arc::new(lat_builder.finish());
        let lon_array: ArrayRef = Arc::new(lon_builder.finish());
        let h3_array: ArrayRef = Arc::new(h3_builder.finish());
        let meta_array: ArrayRef = Arc::new(meta_builder.finish());

        let batch = RecordBatch::try_new(
            self.schema.clone(),
            vec![
                id_array,
                name_array,
                type_array,
                geom_array,
                lat_array,
                lon_array,
                h3_array,
                meta_array,
            ],
        )?;

        Ok(batch)
    }

    fn rebuild_batch_without_row(&mut self, skip_row: usize) -> crate::Result<()> {
        let current_count = self.locations.num_rows();

        if current_count == 0 {
            return Ok(());
        }

        // Build new arrays without the skipped row
        let mut id_builder = BinaryBuilder::new();
        let mut name_builder = StringBuilder::new();
        let mut type_builder = StringBuilder::new();
        let mut geom_builder = BinaryBuilder::new();
        let mut lat_builder = Float64Builder::new();
        let mut lon_builder = Float64Builder::new();
        let mut h3_builder = UInt64Builder::new();
        let mut meta_builder = StringBuilder::new();

        let id_array = self.locations.column(0).as_any().downcast_ref::<BinaryArray>().unwrap();
        let name_array = self.locations.column(1).as_any().downcast_ref::<StringArray>().unwrap();
        let type_array = self.locations.column(2).as_any().downcast_ref::<StringArray>().unwrap();
        let geom_array = self.locations.column(3).as_any().downcast_ref::<BinaryArray>().unwrap();
        let lat_array = self.locations.column(4).as_any().downcast_ref::<Float64Array>().unwrap();
        let lon_array = self.locations.column(5).as_any().downcast_ref::<Float64Array>().unwrap();
        let h3_array = self.locations.column(6).as_any().downcast_ref::<UInt64Array>().unwrap();
        let meta_array = self.locations.column(7).as_any().downcast_ref::<StringArray>().unwrap();

        for i in 0..current_count {
            if i == skip_row {
                continue;
            }

            id_builder.append_value(id_array.value(i));
            if name_array.is_null(i) {
                name_builder.append_null();
            } else {
                name_builder.append_value(name_array.value(i));
            }
            type_builder.append_value(type_array.value(i));
            geom_builder.append_value(geom_array.value(i));
            lat_builder.append_value(lat_array.value(i));
            lon_builder.append_value(lon_array.value(i));
            h3_builder.append_value(h3_array.value(i));
            if meta_array.is_null(i) {
                meta_builder.append_null();
            } else {
                meta_builder.append_value(meta_array.value(i));
            }
        }

        // Build new batch
        let id_array: ArrayRef = Arc::new(id_builder.finish());
        let name_array: ArrayRef = Arc::new(name_builder.finish());
        let type_array: ArrayRef = Arc::new(type_builder.finish());
        let geom_array: ArrayRef = Arc::new(geom_builder.finish());
        let lat_array: ArrayRef = Arc::new(lat_builder.finish());
        let lon_array: ArrayRef = Arc::new(lon_builder.finish());
        let h3_array: ArrayRef = Arc::new(h3_builder.finish());
        let meta_array: ArrayRef = Arc::new(meta_builder.finish());

        self.locations = RecordBatch::try_new(
            self.schema.clone(),
            vec![
                id_array,
                name_array,
                type_array,
                geom_array,
                lat_array,
                lon_array,
                h3_array,
                meta_array,
            ],
        )?;

        Ok(())
    }

    fn rebuild_indices(&mut self) {
        self.location_id_to_row.clear();
        self.h3_index.clear();

        for row_idx in 0..self.locations.num_rows() {
            // Rebuild LocationId index
            if let Ok(id) = self.get_location_id_from_batch(row_idx) {
                self.location_id_to_row.insert(id, row_idx);
            }

            // Rebuild H3 index
            if let Ok(Some(h3_value)) = self.get_u64_from_batch(row_idx, fields::H3_INDEX) {
                if let Ok(cell) = CellIndex::try_from(h3_value) {
                    self.h3_index
                        .entry(cell)
                        .or_insert_with(Vec::new)
                        .push(row_idx);
                }
            }
        }
    }
}

/// Location data retrieved from RecordBatch
#[derive(Debug, Clone)]
pub struct LocationData {
    pub id: LocationId,
    pub position: Point<f64>,
    pub location_type: String,
    pub name: Option<String>,
}

/// Convert a Point to WKB (Well-Known Binary) format
fn point_to_wkb(point: &Point<f64>) -> crate::Result<Vec<u8>> {
    // Simple WKB encoding for Point geometry
    // WKB format: [byte order][type][x][y]
    // byte order: 1 = little endian
    // type: 1 = Point (4 bytes)
    // x, y: 8 bytes each (f64)

    let mut wkb = Vec::with_capacity(21); // 1 + 4 + 8 + 8

    // Byte order (little endian)
    wkb.push(1u8);

    // Geometry type (Point = 1)
    wkb.extend_from_slice(&1u32.to_le_bytes());

    // X coordinate
    wkb.extend_from_slice(&point.x().to_le_bytes());

    // Y coordinate
    wkb.extend_from_slice(&point.y().to_le_bytes());

    Ok(wkb)
}

/// Calculate the H3 ring size needed to cover a radius
///
/// # Arguments
///
/// * `radius_meters` - Radius in meters
/// * `resolution` - H3 resolution
///
/// # Returns
///
/// Number of H3 rings needed
fn calculate_h3_ring_for_radius(radius_meters: f64, resolution: Resolution) -> u32 {
    // Approximate H3 cell edge length in meters by resolution
    let edge_length_m = match resolution {
        Resolution::Zero => 1_107_712.0,
        Resolution::One => 418_676.0,
        Resolution::Two => 158_244.0,
        Resolution::Three => 59_810.0,
        Resolution::Four => 22_606.0,
        Resolution::Five => 8_544.0,
        Resolution::Six => 3_229.0,
        Resolution::Seven => 1_220.0,
        Resolution::Eight => 461.0,
        Resolution::Nine => 174.0,
        Resolution::Ten => 66.0,
        Resolution::Eleven => 25.0,
        Resolution::Twelve => 9.4,
        Resolution::Thirteen => 3.5,
        Resolution::Fourteen => 1.3,
        Resolution::Fifteen => 0.5,
    };

    // Calculate rings needed (with safety margin)
    let rings = (radius_meters / edge_length_m * 1.5).ceil() as u32;
    rings.max(1) // At least 1 ring
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

        let warehouses = handler.get_by_type("warehouse").unwrap();
        assert_eq!(warehouses.len(), 1);

        let stores = handler.get_by_type("store").unwrap();
        assert_eq!(stores.len(), 1);
    }

    #[test]
    fn test_export_batch() {
        let mut handler = LocationHandler::new(Resolution::Nine);

        // Add multiple locations
        let loc1 = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.4194, 37.7749),
            location_type: "warehouse".to_string(),
        };
        let loc2 = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.0842, 37.4224),
            location_type: "store".to_string(),
        };

        handler.add_location(loc1).unwrap();
        handler.add_location(loc2).unwrap();

        // Export batch
        let batch = handler.export_batch();

        // Verify batch has correct number of rows
        assert_eq!(batch.num_rows(), 2);

        // Verify schema matches
        assert_eq!(batch.schema(), handler.schema);

        // Verify batch has all expected columns
        assert_eq!(batch.num_columns(), 8);
        assert!(batch.column_by_name(fields::ID).is_some());
        assert!(batch.column_by_name(fields::GEOMETRY).is_some());
        assert!(batch.column_by_name(fields::H3_INDEX).is_some());
    }

    #[test]
    fn test_import_batch_basic() {
        let mut handler1 = LocationHandler::new(Resolution::Nine);

        // Add locations to first handler
        let loc1 = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.4194, 37.7749),
            location_type: "warehouse".to_string(),
        };
        let loc2 = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.0842, 37.4224),
            location_type: "store".to_string(),
        };
        let id1 = loc1.id();
        let id2 = loc2.id();

        handler1.add_location(loc1).unwrap();
        handler1.add_location(loc2).unwrap();

        // Export from first handler
        let batch = handler1.export_batch();

        // Import into second handler
        let mut handler2 = LocationHandler::<TestLocation>::new(Resolution::Nine);
        handler2.import_batch(batch).unwrap();

        // Verify location count
        assert_eq!(handler2.count(), 2);

        // Verify locations are accessible by ID
        assert!(handler2.get(id1).is_some());
        assert!(handler2.get(id2).is_some());

        // Verify location data
        let data1 = handler2.get(id1).unwrap();
        assert_eq!(data1.location_type, "warehouse");
        assert!((data1.position.x() - (-122.4194)).abs() < 0.0001);
        assert!((data1.position.y() - 37.7749).abs() < 0.0001);

        let data2 = handler2.get(id2).unwrap();
        assert_eq!(data2.location_type, "store");
    }

    #[test]
    fn test_import_batch_rebuilds_h3_index() {
        let mut handler1 = LocationHandler::new(Resolution::Nine);

        // Add locations in San Francisco area
        let loc1 = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.4194, 37.7749), // San Francisco
            location_type: "warehouse".to_string(),
        };
        let loc2 = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.4, 37.78), // Near San Francisco
            location_type: "store".to_string(),
        };
        let loc3 = TestLocation {
            id: LocationId::new(),
            position: Point::new(-118.2437, 34.0522), // Los Angeles (far away)
            location_type: "warehouse".to_string(),
        };

        let id1 = loc1.id();
        let id2 = loc2.id();
        let id3 = loc3.id();

        handler1.add_location(loc1).unwrap();
        handler1.add_location(loc2).unwrap();
        handler1.add_location(loc3).unwrap();

        // Export and import
        let batch = handler1.export_batch();
        let mut handler2 = LocationHandler::<TestLocation>::new(Resolution::Nine);
        handler2.import_batch(batch).unwrap();

        // Test nearest location query (uses H3 index)
        let sf_point = Point::new(-122.42, 37.77);
        let nearest = handler2.nearest(sf_point).unwrap().unwrap();

        // Should find one of the SF locations, not LA
        assert!(nearest == id1 || nearest == id2);
        assert_ne!(nearest, id3);
    }

    #[test]
    fn test_import_batch_spatial_queries_work() {
        let mut handler1 = LocationHandler::new(Resolution::Nine);

        // Add multiple locations
        let loc1 = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.4194, 37.7749),
            location_type: "warehouse".to_string(),
        };
        let loc2 = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.4, 37.78), // ~2km from loc1
            location_type: "warehouse".to_string(),
        };
        let loc3 = TestLocation {
            id: LocationId::new(),
            position: Point::new(-122.0, 37.5), // ~50km from loc1
            location_type: "warehouse".to_string(),
        };

        handler1.add_location(loc1).unwrap();
        handler1.add_location(loc2).unwrap();
        handler1.add_location(loc3).unwrap();

        // Export and import
        let batch = handler1.export_batch();
        let mut handler2 = LocationHandler::<TestLocation>::new(Resolution::Nine);
        handler2.import_batch(batch).unwrap();

        // Test within_radius query (uses H3 index)
        let center = Point::new(-122.4194, 37.7749);
        let within = handler2.within_radius(center, 5000.0).unwrap(); // 5km radius

        // Should find loc1 and loc2, but not loc3
        assert_eq!(within.len(), 2);
    }

    #[test]
    fn test_import_batch_preserves_geometry_wkb() {
        let mut handler1 = LocationHandler::new(Resolution::Nine);

        // Add location with specific coordinates
        let precise_lon = -122.41941234567890;
        let precise_lat = 37.77491234567890;

        let loc = TestLocation {
            id: LocationId::new(),
            position: Point::new(precise_lon, precise_lat),
            location_type: "warehouse".to_string(),
        };
        let id = loc.id();

        handler1.add_location(loc).unwrap();

        // Export and import
        let batch = handler1.export_batch();
        let mut handler2 = LocationHandler::<TestLocation>::new(Resolution::Nine);
        handler2.import_batch(batch).unwrap();

        // Verify geometry precision is preserved
        let position = handler2.get_position(id).unwrap();
        assert!((position.x() - precise_lon).abs() < 1e-10);
        assert!((position.y() - precise_lat).abs() < 1e-10);
    }

    #[test]
    fn test_import_batch_empty() {
        // Test importing empty batch
        let handler1 = LocationHandler::<TestLocation>::new(Resolution::Nine);
        let batch = handler1.export_batch();

        let mut handler2 = LocationHandler::<TestLocation>::new(Resolution::Nine);
        handler2.import_batch(batch).unwrap();

        assert_eq!(handler2.count(), 0);
    }

    #[test]
    fn test_import_batch_schema_validation() {
        // Create a batch with wrong schema
        use arrow::datatypes::{DataType, Field, Schema};

        let wrong_schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Binary, false),
            Field::new("name", DataType::Utf8, true),
        ]));

        let wrong_batch = RecordBatch::new_empty(wrong_schema);

        let mut handler = LocationHandler::<TestLocation>::new(Resolution::Nine);
        let result = handler.import_batch(wrong_batch);

        // Should fail with schema mismatch error
        assert!(result.is_err());
        if let Err(crate::Error::Location(msg)) = result {
            assert!(msg.contains("schema does not match"));
        } else {
            panic!("Expected Location error with schema mismatch message");
        }
    }

    #[test]
    fn test_export_import_roundtrip() {
        let mut handler1 = LocationHandler::new(Resolution::Nine);

        // Add diverse set of locations
        let locations = vec![
            TestLocation {
                id: LocationId::new(),
                position: Point::new(-122.4194, 37.7749),
                location_type: "warehouse".to_string(),
            },
            TestLocation {
                id: LocationId::new(),
                position: Point::new(-118.2437, 34.0522),
                location_type: "store".to_string(),
            },
            TestLocation {
                id: LocationId::new(),
                position: Point::new(-87.6298, 41.8781),
                location_type: "depot".to_string(),
            },
            TestLocation {
                id: LocationId::new(),
                position: Point::new(-74.0060, 40.7128),
                location_type: "warehouse".to_string(),
            },
        ];

        let ids: Vec<_> = locations.iter().map(|l| l.id()).collect();

        for loc in locations {
            handler1.add_location(loc).unwrap();
        }

        // Export
        let batch = handler1.export_batch();

        // Import into new handler
        let mut handler2 = LocationHandler::<TestLocation>::new(Resolution::Nine);
        handler2.import_batch(batch).unwrap();

        // Verify all locations are present
        assert_eq!(handler2.count(), 4);

        for id in &ids {
            assert!(handler2.get(*id).is_some());
        }

        // Verify type filtering still works
        let warehouses = handler2.get_by_type("warehouse").unwrap();
        assert_eq!(warehouses.len(), 2);

        let stores = handler2.get_by_type("store").unwrap();
        assert_eq!(stores.len(), 1);

        let depots = handler2.get_by_type("depot").unwrap();
        assert_eq!(depots.len(), 1);
    }
}
