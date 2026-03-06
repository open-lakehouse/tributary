//! Internal state management for the simulation.
//!
//! This module provides structures and utilities to manage the internal state of the simulation.
//! Whenever feasible, state is tracked as Arrow RecordBatches for seamless introp with
//! external data storages that might be used to store the state.

use arrow::array::cast::AsArray as _;
use uuid::Uuid;

use crate::{Error, Result};
use crate::idents::*;

pub(crate) use self::movement::{RoutingData, Transport, next::MovementHandler};
pub use self::objects::{ObjectData, ObjectLabel};
pub use self::orders::OrderData;
pub(crate) use self::orders::{OrderLineStatus, OrderStatus};
pub(crate) use self::parse_json::parse_json;
pub use self::population::{
    PersonRole, PersonState, PersonStatus, PersonStatusFlag, PopulationData,
};

mod movement;
mod objects;
mod orders;
mod parse_json;
mod population;

#[derive(Debug, thiserror::Error)]
enum StateError {
    #[error("Inconsistent data")]
    InconsistentData,
}

impl From<StateError> for Error {
    fn from(err: StateError) -> Self {
        Error::InternalError(err.to_string())
    }
}

pub trait EntityView {
    type Id: TypedId;
    type Properties: serde::de::DeserializeOwned;

    fn data(&self) -> &ObjectData;

    fn valid_index(&self) -> usize;

    fn id(&self) -> Self::Id {
        Uuid::from_slice(
            self.data()
                .objects()
                .column_by_name("id")
                .expect("object data schema should be validated")
                .as_fixed_size_binary()
                .value(self.valid_index()),
        )
        .unwrap()
        .into()
    }

    fn properties(&self) -> Result<Self::Properties> {
        let raw = self
            .data()
            .objects()
            .column_by_name("properties")
            .ok_or(StateError::InconsistentData)?
            .as_string::<i64>()
            .value(self.valid_index());
        Ok(serde_json::from_str(raw)?)
    }
}
