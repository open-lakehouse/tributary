//! Stations: every capacity-constrained resource in the shipment pipeline

use sim_core::{Resource, ResourceId};
use uuid::Uuid;

/// Namespace for deterministic logistics IDs (UUID v5)
const LOGISTICS_NAMESPACE: Uuid = Uuid::from_u128(0x0b9e_43d2_7a61_4f08_8c55_1e2a_6f93_d7b4);

/// What a station does; each kind serves one step of
/// [`PackageShipmentWorkflow`](crate::PackageShipmentWorkflow)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StationKind {
    /// Receives packages at the warehouse (step 0)
    Receiver,
    /// Sorts packages for delivery (step 1)
    Sorting,
    /// Loads packages onto vehicles (step 2)
    LoadingDock,
    /// A driver and vehicle, delivering one package at a time (step 3)
    Driver,
    /// Confirms delivery (step 4)
    DeliveryConfirmer,
}

impl StationKind {
    /// Kinds in workflow step order
    pub const PIPELINE: [StationKind; 5] = [
        Self::Receiver,
        Self::Sorting,
        Self::LoadingDock,
        Self::Driver,
        Self::DeliveryConfirmer,
    ];

    /// Resource type the allocator matches tasks on
    pub fn resource_type(self) -> &'static str {
        match self {
            Self::Receiver => "package_receiver",
            Self::Sorting => "sorting_station",
            Self::LoadingDock => "loading_dock",
            Self::Driver => "driver",
            Self::DeliveryConfirmer => "delivery_confirmer",
        }
    }
}

/// A station with a capacity (packages handled at once)
#[derive(Debug, Clone)]
pub struct Station {
    pub id: ResourceId,
    pub name: String,
    pub kind: StationKind,
    pub capacity: usize,
}

impl Station {
    /// A station named `name`; its ID is derived from the name
    pub fn new(name: impl Into<String>, kind: StationKind, capacity: usize) -> Self {
        let name = name.into();
        Self {
            id: ResourceId::from_name(LOGISTICS_NAMESPACE, &name),
            name,
            kind,
            capacity,
        }
    }
}

impl Resource for Station {
    fn id(&self) -> ResourceId {
        self.id
    }

    fn resource_type(&self) -> &str {
        self.kind.resource_type()
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn current_load(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable_and_types_match_kind() {
        let a = Station::new("Driver-1", StationKind::Driver, 1);
        assert_eq!(a.id, Station::new("Driver-1", StationKind::Driver, 1).id);
        assert_eq!(a.resource_type(), "driver");
        assert_ne!(a.id, Station::new("Driver-2", StationKind::Driver, 1).id);
    }
}
