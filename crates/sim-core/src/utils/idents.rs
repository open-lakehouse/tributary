//! Identifier types for simulation entities
//!
//! This module provides type-safe wrappers around UUIDs for different entity types.
//! Using distinct types prevents accidentally mixing up IDs from different domains.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Macro to define a UUID-based identifier type
macro_rules! define_id {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(Uuid);

        impl $name {
            /// Create a new random identifier using UUID v7 (time-ordered)
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            /// Create a deterministic identifier using UUID v5 (namespace + name)
            pub fn from_name(namespace: Uuid, name: &str) -> Self {
                Self(Uuid::new_v5(&namespace, name.as_bytes()))
            }

            /// Create from an existing UUID
            pub fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// Get the underlying UUID
            pub fn as_uuid(&self) -> Uuid {
                self.0
            }

            /// Get as bytes (16 bytes)
            pub fn as_bytes(&self) -> &[u8; 16] {
                self.0.as_bytes()
            }

            /// Create from bytes (16 bytes)
            pub fn from_bytes(bytes: &[u8]) -> Result<Self, uuid::Error> {
                Ok(Self(Uuid::from_slice(bytes)?))
            }

            /// Parse from string
            pub fn parse(s: &str) -> Result<Self, uuid::Error> {
                Ok(Self(Uuid::parse_str(s)?))
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<Uuid> for $name {
            fn from(uuid: Uuid) -> Self {
                Self(uuid)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }
    };
}

// Define identifier types
define_id!(AgentId, "Unique identifier for an agent in the simulation");
define_id!(LocationId, "Unique identifier for a location");
define_id!(ResourceId, "Unique identifier for a resource");
define_id!(TaskId, "Unique identifier for a task");
define_id!(WorkflowId, "Unique identifier for a workflow definition");
define_id!(WorkflowInstanceId, "Unique identifier for a workflow instance");
define_id!(JourneyId, "Unique identifier for a journey");
define_id!(EventId, "Unique identifier for an event");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_id_creation() {
        let id1 = AgentId::new();
        let id2 = AgentId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_deterministic_id() {
        let namespace = Uuid::new_v4();
        let id1 = LocationId::from_name(namespace, "kitchen-1");
        let id2 = LocationId::from_name(namespace, "kitchen-1");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_id_display() {
        let id = AgentId::new();
        let s = id.to_string();
        let parsed = AgentId::parse(&s).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_id_serialization() {
        let id = ResourceId::new();
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: ResourceId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }
}
