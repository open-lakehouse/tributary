//! KitchenStation resource — represents a physical cooking station within a kitchen

use crate::recipes::types::StationKind;
use sim_core::ResourceId;
use uuid::Uuid;

/// A physical station within a kitchen (e.g., stove, fryer, workstation).
///
/// Multiple stations of a given kind may exist in a single kitchen. Each
/// station has its own capacity, allowing concurrent work at the same kind
/// of station.
#[derive(Debug, Clone)]
pub struct KitchenStation {
    pub id: ResourceId,
    pub kitchen_id: Uuid,
    pub station_kind: StationKind,
    pub capacity: usize,
}

impl KitchenStation {
    /// Create a new kitchen station associated with the given kitchen.
    pub fn new(kitchen_id: Uuid, station_kind: StationKind, capacity: usize) -> Self {
        Self {
            id: ResourceId::new(),
            kitchen_id,
            station_kind,
            capacity,
        }
    }
}
