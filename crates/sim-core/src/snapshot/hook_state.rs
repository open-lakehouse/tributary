//! Single-row JSON encoding for domain hook state in snapshots
//!
//! [`SimulationHooks::snapshot_state`](crate::SimulationHooks::snapshot_state)
//! returns an Arrow batch so it can be stored next to the other snapshot
//! tables. Most domains only need to persist a serde-serializable struct;
//! these helpers wrap it in a one-row batch with a single Utf8 `state`
//! column holding its JSON. Floats round-trip exactly (sim-core enables
//! serde_json's `float_roundtrip`).

use crate::{Error, Result};
use arrow::array::{Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::sync::Arc;

const STATE_COLUMN: &str = "state";

/// Encode `state` as a one-row batch for [`SnapshotData::hook_state`](crate::snapshot::SnapshotData::hook_state)
///
/// # Errors
///
/// Returns [`Error::Serialization`] if `state` fails to serialize.
///
/// # Example
///
/// ```
/// use sim_core::snapshot::{hook_state_batch, hook_state_from_batch};
///
/// let batch = hook_state_batch(&vec![1, 2, 3]).unwrap();
/// let state: Vec<i32> = hook_state_from_batch(&batch).unwrap();
/// assert_eq!(state, vec![1, 2, 3]);
/// ```
pub fn hook_state_batch<T: Serialize>(state: &T) -> Result<RecordBatch> {
    let json = serde_json::to_string(state)?;
    let schema = Schema::new(vec![Field::new(STATE_COLUMN, DataType::Utf8, false)]);
    Ok(RecordBatch::try_new(
        Arc::new(schema),
        vec![Arc::new(StringArray::from(vec![json]))],
    )?)
}

/// Decode a batch written by [`hook_state_batch`]
///
/// # Errors
///
/// Returns [`Error::InvalidState`] if the batch is not one row with a Utf8
/// `state` column, or [`Error::Serialization`] if the JSON doesn't match `T`.
pub fn hook_state_from_batch<T: DeserializeOwned>(batch: &RecordBatch) -> Result<T> {
    if batch.num_rows() != 1 {
        return Err(Error::InvalidState(format!(
            "hook state must have exactly one row, got {}",
            batch.num_rows()
        )));
    }
    let column = batch
        .column_by_name(STATE_COLUMN)
        .and_then(|c| c.as_any().downcast_ref::<StringArray>())
        .filter(|c| !c.is_null(0))
        .ok_or_else(|| {
            Error::InvalidState("hook state needs a non-null Utf8 `state` column".into())
        })?;
    Ok(serde_json::from_str(column.value(0))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn round_trips_a_struct() {
        let state: HashMap<String, (u64, f64)> = [("a".to_string(), (1, 2.5))].into();
        let batch = hook_state_batch(&state).unwrap();
        assert_eq!(batch.num_rows(), 1);
        let back: HashMap<String, (u64, f64)> = hook_state_from_batch(&batch).unwrap();
        assert_eq!(back, state);
    }

    #[test]
    fn floats_round_trip_exactly() {
        // Restored runs must match uninterrupted ones bit for bit.
        let values = vec![-122.35128799621839_f64, 0.1 + 0.2, f64::MIN_POSITIVE, 1e300];
        let back: Vec<f64> = hook_state_from_batch(&hook_state_batch(&values).unwrap()).unwrap();
        assert_eq!(
            back.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            values.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn rejects_malformed_batches() {
        let empty = RecordBatch::new_empty(hook_state_batch(&1).unwrap().schema());
        assert!(hook_state_from_batch::<i32>(&empty).is_err());
        let batch = hook_state_batch(&"text").unwrap();
        assert!(matches!(
            hook_state_from_batch::<i32>(&batch),
            Err(Error::Serialization(_))
        ));
    }
}
