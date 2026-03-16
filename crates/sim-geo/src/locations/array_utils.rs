//! Utility functions for Arrow array manipulation in location handling
//!
//! This module provides helper functions to reduce code duplication when working
//! with RecordBatch array operations.

use arrow::array::{
    Array, ArrayRef, BinaryArray, BinaryBuilder, Float64Array, Float64Builder, RecordBatch,
    StringArray, StringBuilder, UInt64Array, UInt64Builder,
};
use std::sync::Arc;

/// Collection of typed array builders for location data
pub struct LocationArrayBuilders {
    pub id: BinaryBuilder,
    pub name: StringBuilder,
    pub location_type: StringBuilder,
    pub geometry: BinaryBuilder,
    pub latitude: Float64Builder,
    pub longitude: Float64Builder,
    pub h3_index: UInt64Builder,
    pub metadata: StringBuilder,
}

impl LocationArrayBuilders {
    /// Create new empty builders
    pub fn new() -> Self {
        Self {
            id: BinaryBuilder::new(),
            name: StringBuilder::new(),
            location_type: StringBuilder::new(),
            geometry: BinaryBuilder::new(),
            latitude: Float64Builder::new(),
            longitude: Float64Builder::new(),
            h3_index: UInt64Builder::new(),
            metadata: StringBuilder::new(),
        }
    }

    /// Build all arrays and return as ArrayRef vector
    pub fn finish(mut self) -> Vec<ArrayRef> {
        vec![
            Arc::new(self.id.finish()),
            Arc::new(self.name.finish()),
            Arc::new(self.location_type.finish()),
            Arc::new(self.geometry.finish()),
            Arc::new(self.latitude.finish()),
            Arc::new(self.longitude.finish()),
            Arc::new(self.h3_index.finish()),
            Arc::new(self.metadata.finish()),
        ]
    }
}

impl Default for LocationArrayBuilders {
    fn default() -> Self {
        Self::new()
    }
}

/// Collection of typed arrays from a RecordBatch
pub struct LocationArrays<'a> {
    pub id: &'a BinaryArray,
    pub name: &'a StringArray,
    pub location_type: &'a StringArray,
    pub geometry: &'a BinaryArray,
    pub latitude: &'a Float64Array,
    pub longitude: &'a Float64Array,
    pub h3_index: &'a UInt64Array,
    pub metadata: &'a StringArray,
}

impl<'a> LocationArrays<'a> {
    /// Extract typed arrays from a RecordBatch
    ///
    /// # Panics
    ///
    /// Panics if the batch doesn't have the expected schema or column types.
    /// This is intentional - if the schema is wrong, it's a programming error.
    pub fn from_batch(batch: &'a RecordBatch) -> Self {
        Self {
            id: batch
                .column(0)
                .as_any()
                .downcast_ref::<BinaryArray>()
                .unwrap(),
            name: batch
                .column(1)
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap(),
            location_type: batch
                .column(2)
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap(),
            geometry: batch
                .column(3)
                .as_any()
                .downcast_ref::<BinaryArray>()
                .unwrap(),
            latitude: batch
                .column(4)
                .as_any()
                .downcast_ref::<Float64Array>()
                .unwrap(),
            longitude: batch
                .column(5)
                .as_any()
                .downcast_ref::<Float64Array>()
                .unwrap(),
            h3_index: batch
                .column(6)
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap(),
            metadata: batch
                .column(7)
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap(),
        }
    }

    /// Copy a single row from the arrays into builders
    pub fn copy_row_to_builders(&self, row_idx: usize, builders: &mut LocationArrayBuilders) {
        builders.id.append_value(self.id.value(row_idx));

        if self.name.is_null(row_idx) {
            builders.name.append_null();
        } else {
            builders.name.append_value(self.name.value(row_idx));
        }

        builders
            .location_type
            .append_value(self.location_type.value(row_idx));
        builders.geometry.append_value(self.geometry.value(row_idx));
        builders.latitude.append_value(self.latitude.value(row_idx));
        builders
            .longitude
            .append_value(self.longitude.value(row_idx));
        builders.h3_index.append_value(self.h3_index.value(row_idx));

        if self.metadata.is_null(row_idx) {
            builders.metadata.append_null();
        } else {
            builders.metadata.append_value(self.metadata.value(row_idx));
        }
    }

    /// Copy all rows from the arrays into builders
    #[allow(dead_code)]
    pub fn copy_all_to_builders(&self, builders: &mut LocationArrayBuilders) {
        let num_rows = self.id.len();
        for i in 0..num_rows {
            self.copy_row_to_builders(i, builders);
        }
    }

    /// Copy all rows except one into builders
    pub fn copy_all_except_to_builders(
        &self,
        skip_row: usize,
        builders: &mut LocationArrayBuilders,
    ) {
        let num_rows = self.id.len();
        for i in 0..num_rows {
            if i != skip_row {
                self.copy_row_to_builders(i, builders);
            }
        }
    }
}

/// Helper to get a typed column from a RecordBatch by name
pub fn get_typed_column<'a, T: Array + 'static>(
    batch: &'a RecordBatch,
    field_name: &str,
) -> crate::Result<&'a T> {
    let column = batch
        .column_by_name(field_name)
        .ok_or_else(|| crate::Error::Location(format!("{} column not found", field_name)))?;

    column
        .as_any()
        .downcast_ref::<T>()
        .ok_or_else(|| crate::Error::Location(format!("{} column has wrong type", field_name)))
}

/// Get an f64 value from a batch at a specific row
pub fn get_f64_value(
    batch: &RecordBatch,
    row_idx: usize,
    field_name: &str,
) -> crate::Result<Option<f64>> {
    let array = get_typed_column::<Float64Array>(batch, field_name)?;

    if array.is_null(row_idx) {
        Ok(None)
    } else {
        Ok(Some(array.value(row_idx)))
    }
}

/// Get a u64 value from a batch at a specific row
pub fn get_u64_value(
    batch: &RecordBatch,
    row_idx: usize,
    field_name: &str,
) -> crate::Result<Option<u64>> {
    let array = get_typed_column::<UInt64Array>(batch, field_name)?;

    if array.is_null(row_idx) {
        Ok(None)
    } else {
        Ok(Some(array.value(row_idx)))
    }
}

/// Get a string value from a batch at a specific row
pub fn get_string_value(
    batch: &RecordBatch,
    row_idx: usize,
    field_name: &str,
) -> crate::Result<Option<String>> {
    let array = get_typed_column::<StringArray>(batch, field_name)?;

    if array.is_null(row_idx) {
        Ok(None)
    } else {
        Ok(Some(array.value(row_idx).to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locations::schema::{fields, location_schema};

    #[test]
    fn test_builders_create_and_finish() {
        let mut builders = LocationArrayBuilders::new();

        // Add a row
        builders.id.append_value(b"test_id_12345678");
        builders.name.append_value("Test Location");
        builders.location_type.append_value("warehouse");
        builders.geometry.append_value(b"geometry_data");
        builders.latitude.append_value(37.7749);
        builders.longitude.append_value(-122.4194);
        builders.h3_index.append_value(12345);
        builders.metadata.append_value("{}");

        let arrays = builders.finish();
        assert_eq!(arrays.len(), 8);
        assert_eq!(arrays[0].len(), 1);
    }

    #[test]
    fn test_location_arrays_from_batch() {
        let schema = location_schema();
        let empty_batch = RecordBatch::new_empty(schema);

        // Should not panic with correct schema
        let _arrays = LocationArrays::from_batch(&empty_batch);
    }

    #[test]
    fn test_copy_row_to_builders() {
        // Create a batch with one row
        let mut builders = LocationArrayBuilders::new();
        builders.id.append_value(b"test_id_12345678");
        builders.name.append_value("Test");
        builders.location_type.append_value("warehouse");
        builders.geometry.append_value(b"geom");
        builders.latitude.append_value(37.7749);
        builders.longitude.append_value(-122.4194);
        builders.h3_index.append_value(12345);
        builders.metadata.append_value("{}");

        let arrays = builders.finish();
        let schema = location_schema();
        let batch = RecordBatch::try_new(schema, arrays).unwrap();

        // Extract arrays and copy to new builders
        let source_arrays = LocationArrays::from_batch(&batch);
        let mut dest_builders = LocationArrayBuilders::new();
        source_arrays.copy_row_to_builders(0, &mut dest_builders);

        let result_arrays = dest_builders.finish();
        assert_eq!(result_arrays[0].len(), 1);
    }

    #[test]
    fn test_get_typed_column() {
        let schema = location_schema();
        let empty_batch = RecordBatch::new_empty(schema);

        // Should succeed with correct type
        let result = get_typed_column::<BinaryArray>(&empty_batch, fields::ID);
        assert!(result.is_ok());

        // Should fail with wrong type
        let result = get_typed_column::<StringArray>(&empty_batch, fields::ID);
        assert!(result.is_err());

        // Should fail with non-existent field
        let result = get_typed_column::<StringArray>(&empty_batch, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_copy_all_except() {
        // Create a batch with 3 rows
        let mut builders = LocationArrayBuilders::new();
        for i in 0..3 {
            builders.id.append_value(format!("id_{}", i).as_bytes());
            builders.name.append_value(format!("Name {}", i));
            builders.location_type.append_value("warehouse");
            builders.geometry.append_value(b"geom");
            builders.latitude.append_value(37.0 + i as f64);
            builders.longitude.append_value(-122.0);
            builders.h3_index.append_value(12345 + i as u64);
            builders.metadata.append_value("{}");
        }

        let arrays = builders.finish();
        let schema = location_schema();
        let batch = RecordBatch::try_new(schema, arrays).unwrap();

        // Copy all except row 1
        let source_arrays = LocationArrays::from_batch(&batch);
        let mut dest_builders = LocationArrayBuilders::new();
        source_arrays.copy_all_except_to_builders(1, &mut dest_builders);

        let result_arrays = dest_builders.finish();
        assert_eq!(result_arrays[0].len(), 2);
    }
}
