//! Package Agent for Logistics Operations
//!
//! Packages are entities tracked through the delivery lifecycle from creation
//! through sorting, loading, transit, and final delivery.
//!
//! # State Machine
//!
//! ```text
//!     ┌─────────┐
//!     │ Created │
//!     └────┬────┘
//!          │ arrives at warehouse
//!          ▼
//!     ┌────────┐
//!     │ Sorted │
//!     └────┬───┘
//!          │ loaded on vehicle
//!          ▼
//!     ┌────────┐
//!     │ Loaded │
//!     └────┬───┘
//!          │ vehicle departs
//!          ▼
//!     ┌───────────┐
//!     │ InTransit │
//!     └─────┬─────┘
//!           │ delivered
//!           ▼
//!     ┌───────────┐
//!     │ Delivered │
//!     └───────────┘
//! ```

use chrono::{DateTime, Utc};
use geo::Point;
use serde_json::Value;
use sim_core::{
    Agent, AgentId, AgentStatus, SimulationContext,
    population::{AgentTickResult, ArrowAgentRow},
    Result,
};
use uuid::Uuid;

/// Package status states
#[derive(Debug, Clone, PartialEq)]
pub enum PackageState {
    /// Package created but not yet at warehouse
    Created {
        origin: Point<f64>,
        destination: Point<f64>,
    },

    /// Package arrived at warehouse and being sorted
    Sorted {
        warehouse_id: String,
        destination: Point<f64>,
    },

    /// Package loaded onto delivery vehicle
    Loaded {
        driver_id: String,
        destination: Point<f64>,
    },

    /// Package in transit to destination
    InTransit {
        driver_id: String,
        destination: Point<f64>,
        estimated_arrival: DateTime<Utc>,
    },

    /// Package successfully delivered
    Delivered {
        delivery_time: DateTime<Utc>,
        destination: Point<f64>,
    },
}

impl AgentStatus for PackageState {
    fn as_str(&self) -> &str {
        match self {
            Self::Created { .. } => "created",
            Self::Sorted { .. } => "sorted",
            Self::Loaded { .. } => "loaded",
            Self::InTransit { .. } => "in_transit",
            Self::Delivered { .. } => "delivered",
        }
    }

    fn is_idle(&self) -> bool {
        matches!(self, Self::Delivered { .. })
    }

    fn is_active(&self) -> bool {
        !matches!(self, Self::Delivered { .. })
    }
}

/// Package Agent
///
/// Represents a package being delivered through the logistics system.
/// Tracks state transitions and maintains timestamps for SLA monitoring.
#[derive(Debug, Clone)]
pub struct Package {
    pub id: AgentId,
    pub state: PackageState,
    pub tracking_number: String,
    pub created_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub weight_kg: f64,
    pub priority: PackagePriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackagePriority {
    Standard,
    Express,
    Overnight,
}

impl Package {
    /// Create a new package
    pub fn new(
        origin: Point<f64>,
        destination: Point<f64>,
        weight_kg: f64,
        priority: PackagePriority,
    ) -> Self {
        Self {
            id: AgentId::new(),
            state: PackageState::Created { origin, destination },
            tracking_number: format!("PKG-{}", Uuid::new_v4().simple()),
            created_at: Utc::now(),
            delivered_at: None,
            weight_kg,
            priority,
        }
    }

    /// Transition package to sorted state
    pub fn mark_sorted(&mut self, warehouse_id: String) {
        if let PackageState::Created { destination, .. } = &self.state {
            self.state = PackageState::Sorted {
                warehouse_id,
                destination: *destination,
            };
        }
    }

    /// Transition package to loaded state
    pub fn mark_loaded(&mut self, driver_id: String) {
        if let PackageState::Sorted { destination, .. } = &self.state {
            self.state = PackageState::Loaded {
                driver_id,
                destination: *destination,
            };
        }
    }

    /// Transition package to in-transit state
    pub fn mark_in_transit(&mut self, estimated_arrival: DateTime<Utc>) {
        if let PackageState::Loaded { driver_id, destination } = &self.state {
            self.state = PackageState::InTransit {
                driver_id: driver_id.clone(),
                destination: *destination,
                estimated_arrival,
            };
        }
    }

    /// Transition package to delivered state
    pub fn mark_delivered(&mut self) {
        if let PackageState::InTransit { destination, .. } = &self.state {
            let now = Utc::now();
            self.state = PackageState::Delivered {
                delivery_time: now,
                destination: *destination,
            };
            self.delivered_at = Some(now);
        }
    }

    /// Calculate delivery time in seconds (for SLA monitoring)
    pub fn delivery_time_seconds(&self) -> Option<i64> {
        self.delivered_at.map(|delivered| {
            (delivered - self.created_at).num_seconds()
        })
    }

    /// Check if package is delivered
    pub fn is_delivered(&self) -> bool {
        matches!(self.state, PackageState::Delivered { .. })
    }
}

impl Agent for Package {
    fn id(&self) -> AgentId {
        self.id
    }

    fn agent_type(&self) -> &str {
        "package"
    }

    fn status(&self) -> &dyn AgentStatus {
        &self.state
    }

    fn position(&self) -> Option<Point<f64>> {
        match &self.state {
            PackageState::Created { origin, .. } => Some(*origin),
            PackageState::Sorted { .. } => None, // At warehouse, position tracked by warehouse
            PackageState::Loaded { .. } => None, // On vehicle, position tracked by driver
            PackageState::InTransit { .. } => None, // On vehicle, position tracked by driver
            PackageState::Delivered { destination, .. } => Some(*destination),
        }
    }

    fn tick(&mut self, _ctx: &SimulationContext) -> Result<AgentTickResult> {
        // Packages are passive agents - state transitions driven externally
        // by sorting stations, drivers, etc.
        Ok(AgentTickResult::default())
    }

    fn state_data(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        map.insert("tracking_number".to_string(), Value::String(self.tracking_number.clone()));
        map.insert("weight_kg".to_string(), Value::Number(
            serde_json::Number::from_f64(self.weight_kg).unwrap()
        ));
        map.insert("priority".to_string(), Value::String(
            match self.priority {
                PackagePriority::Standard => "standard",
                PackagePriority::Express => "express",
                PackagePriority::Overnight => "overnight",
            }.to_string()
        ));

        match &self.state {
            PackageState::Created { origin, destination } => {
                map.insert("origin_x".to_string(), Value::Number(
                    serde_json::Number::from_f64(origin.x()).unwrap()
                ));
                map.insert("origin_y".to_string(), Value::Number(
                    serde_json::Number::from_f64(origin.y()).unwrap()
                ));
                map.insert("dest_x".to_string(), Value::Number(
                    serde_json::Number::from_f64(destination.x()).unwrap()
                ));
                map.insert("dest_y".to_string(), Value::Number(
                    serde_json::Number::from_f64(destination.y()).unwrap()
                ));
            }
            PackageState::Sorted { warehouse_id, destination } => {
                map.insert("warehouse_id".to_string(), Value::String(warehouse_id.clone()));
                map.insert("dest_x".to_string(), Value::Number(
                    serde_json::Number::from_f64(destination.x()).unwrap()
                ));
                map.insert("dest_y".to_string(), Value::Number(
                    serde_json::Number::from_f64(destination.y()).unwrap()
                ));
            }
            PackageState::Loaded { driver_id, destination } => {
                map.insert("driver_id".to_string(), Value::String(driver_id.clone()));
                map.insert("dest_x".to_string(), Value::Number(
                    serde_json::Number::from_f64(destination.x()).unwrap()
                ));
                map.insert("dest_y".to_string(), Value::Number(
                    serde_json::Number::from_f64(destination.y()).unwrap()
                ));
            }
            PackageState::InTransit { driver_id, destination, estimated_arrival } => {
                map.insert("driver_id".to_string(), Value::String(driver_id.clone()));
                map.insert("dest_x".to_string(), Value::Number(
                    serde_json::Number::from_f64(destination.x()).unwrap()
                ));
                map.insert("dest_y".to_string(), Value::Number(
                    serde_json::Number::from_f64(destination.y()).unwrap()
                ));
                map.insert("eta".to_string(), Value::String(estimated_arrival.to_rfc3339()));
            }
            PackageState::Delivered { delivery_time, destination } => {
                map.insert("delivery_time".to_string(), Value::String(delivery_time.to_rfc3339()));
                map.insert("dest_x".to_string(), Value::Number(
                    serde_json::Number::from_f64(destination.x()).unwrap()
                ));
                map.insert("dest_y".to_string(), Value::Number(
                    serde_json::Number::from_f64(destination.y()).unwrap()
                ));
            }
        }

        Value::Object(map)
    }

    fn from_arrow_row(row: &ArrowAgentRow) -> Result<Self> {
        use sim_core::Error;

        let id = AgentId::from_uuid(
            Uuid::from_slice(&row.id)
                .map_err(|e| Error::Agent(format!("Invalid UUID: {}", e)))?,
        );

        let metadata: serde_json::Value = if let Some(ref m) = row.metadata {
            serde_json::from_str(m).unwrap_or_default()
        } else {
            serde_json::Value::Null
        };

        let tracking_number = metadata.get("tracking_number")
            .and_then(|v| v.as_str())
            .unwrap_or("PKG-unknown")
            .to_string();

        let weight_kg = metadata.get("weight_kg")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);

        let priority = match metadata.get("priority").and_then(|v| v.as_str()) {
            Some("express") => PackagePriority::Express,
            Some("overnight") => PackagePriority::Overnight,
            _ => PackagePriority::Standard,
        };

        // Reconstruct state from status string and metadata
        let state = match row.status.as_str() {
            "created" => {
                let origin_x = metadata.get("origin_x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let origin_y = metadata.get("origin_y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let dest_x = metadata.get("dest_x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let dest_y = metadata.get("dest_y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                PackageState::Created {
                    origin: Point::new(origin_x, origin_y),
                    destination: Point::new(dest_x, dest_y),
                }
            }
            "sorted" => {
                let warehouse_id = metadata.get("warehouse_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let dest_x = metadata.get("dest_x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let dest_y = metadata.get("dest_y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                PackageState::Sorted {
                    warehouse_id,
                    destination: Point::new(dest_x, dest_y),
                }
            }
            "delivered" => {
                let delivery_time = metadata.get("delivery_time")
                    .and_then(|v| v.as_str())
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now);
                let dest_x = metadata.get("dest_x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let dest_y = metadata.get("dest_y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                PackageState::Delivered {
                    delivery_time,
                    destination: Point::new(dest_x, dest_y),
                }
            }
            _ => {
                let origin = Point::new(0.0, 0.0);
                let destination = Point::new(0.0, 0.0);
                PackageState::Created { origin, destination }
            }
        };

        Ok(Package {
            id,
            state,
            tracking_number,
            created_at: Utc::now(),
            delivered_at: None,
            weight_kg,
            priority,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_creation() {
        let origin = Point::new(-122.4, 37.8);
        let destination = Point::new(-122.5, 37.9);
        let pkg = Package::new(origin, destination, 2.5, PackagePriority::Standard);

        assert_eq!(pkg.agent_type(), "package");
        assert_eq!(pkg.state.as_str(), "created");
        assert_eq!(pkg.weight_kg, 2.5);
        assert!(!pkg.is_delivered());
    }

    #[test]
    fn test_package_state_transitions() {
        let origin = Point::new(0.0, 0.0);
        let destination = Point::new(1.0, 1.0);
        let mut pkg = Package::new(origin, destination, 1.0, PackagePriority::Standard);

        // Created -> Sorted
        pkg.mark_sorted("warehouse-1".to_string());
        assert_eq!(pkg.state.as_str(), "sorted");

        // Sorted -> Loaded
        pkg.mark_loaded("driver-1".to_string());
        assert_eq!(pkg.state.as_str(), "loaded");

        // Loaded -> InTransit
        let eta = Utc::now();
        pkg.mark_in_transit(eta);
        assert_eq!(pkg.state.as_str(), "in_transit");

        // InTransit -> Delivered
        pkg.mark_delivered();
        assert_eq!(pkg.state.as_str(), "delivered");
        assert!(pkg.is_delivered());
        assert!(pkg.delivered_at.is_some());
    }

    #[test]
    fn test_package_priority() {
        let origin = Point::new(0.0, 0.0);
        let destination = Point::new(1.0, 1.0);

        let standard = Package::new(origin, destination, 1.0, PackagePriority::Standard);
        assert!(matches!(standard.priority, PackagePriority::Standard));

        let express = Package::new(origin, destination, 1.0, PackagePriority::Express);
        assert!(matches!(express.priority, PackagePriority::Express));

        let overnight = Package::new(origin, destination, 1.0, PackagePriority::Overnight);
        assert!(matches!(overnight.priority, PackagePriority::Overnight));
    }

    #[test]
    fn test_delivery_time_calculation() {
        let origin = Point::new(0.0, 0.0);
        let destination = Point::new(1.0, 1.0);
        let mut pkg = Package::new(origin, destination, 1.0, PackagePriority::Standard);

        assert!(pkg.delivery_time_seconds().is_none());

        // Simulate delivery
        pkg.mark_sorted("w1".to_string());
        pkg.mark_loaded("d1".to_string());
        pkg.mark_in_transit(Utc::now());
        std::thread::sleep(std::time::Duration::from_millis(100));
        pkg.mark_delivered();

        assert!(pkg.delivery_time_seconds().is_some());
        assert!(pkg.delivery_time_seconds().unwrap() >= 0);
    }

    #[test]
    fn test_package_tracking_number() {
        let pkg = Package::new(
            Point::new(0.0, 0.0),
            Point::new(1.0, 1.0),
            1.0,
            PackagePriority::Standard,
        );

        assert!(pkg.tracking_number.starts_with("PKG-"));
        assert!(pkg.tracking_number.len() > 4);
    }

    #[test]
    fn test_package_state_data() {
        let mut pkg = Package::new(
            Point::new(0.0, 0.0),
            Point::new(1.0, 1.0),
            2.5,
            PackagePriority::Express,
        );

        let data = pkg.state_data();
        assert_eq!(data["priority"], "express");
        assert_eq!(data["weight_kg"], 2.5);

        pkg.mark_sorted("w1".to_string());
        let data = pkg.state_data();
        assert_eq!(data["warehouse_id"], "w1");
    }

    #[test]
    fn test_package_position() {
        let origin = Point::new(0.0, 0.0);
        let destination = Point::new(1.0, 1.0);
        let mut pkg = Package::new(origin, destination, 1.0, PackagePriority::Standard);

        // Created state has position at origin
        assert_eq!(pkg.position(), Some(origin));

        // Sorted/Loaded/InTransit have no position (tracked by warehouse/driver)
        pkg.mark_sorted("w1".to_string());
        assert_eq!(pkg.position(), None);

        pkg.mark_loaded("d1".to_string());
        assert_eq!(pkg.position(), None);

        pkg.mark_in_transit(Utc::now());
        assert_eq!(pkg.position(), None);

        // Delivered has position at destination
        pkg.mark_delivered();
        assert_eq!(pkg.position(), Some(destination));
    }

    #[test]
    fn test_package_arrow_round_trip() {
        let pkg = Package::new(
            Point::new(1.0, 2.0),
            Point::new(3.0, 4.0),
            2.5,
            PackagePriority::Express,
        );

        let row = pkg.to_arrow_row();
        let restored = Package::from_arrow_row(&row).unwrap();

        assert_eq!(pkg.id, restored.id);
        assert_eq!(pkg.state.as_str(), restored.state.as_str());
        assert_eq!(pkg.weight_kg, restored.weight_kg);
    }
}
