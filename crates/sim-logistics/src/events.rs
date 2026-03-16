//! Event payloads for logistics domain
//!
//! All events follow CloudEvents 1.0 specification

use chrono::{DateTime, Utc};
use geo::Point;
use serde_json::{Value, json};
use sim_events::EventPayload;

/// Event emitted when a package is received at warehouse
#[derive(Debug)]
pub struct PackageReceivedEvent {
    pub package_id: String,
    pub warehouse_id: String,
    pub destination: Point<f64>,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for PackageReceivedEvent {
    fn event_type(&self) -> &str {
        "io.logistics.package.received"
    }

    fn source(&self) -> String {
        format!("/warehouses/{}", self.warehouse_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.package_id.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "package_id": self.package_id,
            "warehouse_id": self.warehouse_id,
            "destination_x": self.destination.x(),
            "destination_y": self.destination.y(),
            "timestamp": self.timestamp.to_rfc3339(),
        })
    }
}

/// Event emitted when a package has been sorted
#[derive(Debug)]
pub struct PackageSortedEvent {
    pub package_id: String,
    pub sorting_station_id: String,
    pub warehouse_id: String,
}

impl EventPayload for PackageSortedEvent {
    fn event_type(&self) -> &str {
        "io.logistics.package.sorted"
    }

    fn source(&self) -> String {
        format!("/sorting_stations/{}", self.sorting_station_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.package_id.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "package_id": self.package_id,
            "sorting_station_id": self.sorting_station_id,
            "warehouse_id": self.warehouse_id,
        })
    }
}

/// Event emitted when a package is loaded onto a vehicle
#[derive(Debug)]
pub struct PackageLoadedEvent {
    pub package_id: String,
    pub driver_id: String,
    pub loading_dock_id: String,
}

impl EventPayload for PackageLoadedEvent {
    fn event_type(&self) -> &str {
        "io.logistics.package.loaded"
    }

    fn source(&self) -> String {
        format!("/loading_docks/{}", self.loading_dock_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.package_id.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "package_id": self.package_id,
            "driver_id": self.driver_id,
            "loading_dock_id": self.loading_dock_id,
        })
    }
}

/// Event emitted when a package is in transit
#[derive(Debug)]
pub struct PackageInTransitEvent {
    pub package_id: String,
    pub driver_id: String,
    pub estimated_arrival: DateTime<Utc>,
}

impl EventPayload for PackageInTransitEvent {
    fn event_type(&self) -> &str {
        "io.logistics.package.in_transit"
    }

    fn source(&self) -> String {
        format!("/drivers/{}", self.driver_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.package_id.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "package_id": self.package_id,
            "driver_id": self.driver_id,
            "estimated_arrival": self.estimated_arrival.to_rfc3339(),
        })
    }
}

/// Event emitted when a package is delivered
#[derive(Debug)]
pub struct PackageDeliveredEvent {
    pub package_id: String,
    pub driver_id: String,
    pub delivery_time: DateTime<Utc>,
    pub destination: Point<f64>,
}

impl EventPayload for PackageDeliveredEvent {
    fn event_type(&self) -> &str {
        "io.logistics.package.delivered"
    }

    fn source(&self) -> String {
        format!("/drivers/{}", self.driver_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.package_id.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "package_id": self.package_id,
            "driver_id": self.driver_id,
            "delivery_time": self.delivery_time.to_rfc3339(),
            "destination_x": self.destination.x(),
            "destination_y": self.destination.y(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_received_event() {
        let event = PackageReceivedEvent {
            package_id: "PKG-123".to_string(),
            warehouse_id: "W1".to_string(),
            destination: Point::new(1.0, 2.0),
            timestamp: Utc::now(),
        };

        assert_eq!(event.event_type(), "io.logistics.package.received");
        assert_eq!(event.source(), "/warehouses/W1");
        assert_eq!(event.subject(), Some("PKG-123".to_string()));

        let json = event.to_json();
        assert_eq!(json["package_id"], "PKG-123");
        assert_eq!(json["warehouse_id"], "W1");
    }

    #[test]
    fn test_package_sorted_event() {
        let event = PackageSortedEvent {
            package_id: "PKG-456".to_string(),
            sorting_station_id: "SS1".to_string(),
            warehouse_id: "W1".to_string(),
        };

        assert_eq!(event.event_type(), "io.logistics.package.sorted");
        assert!(event.source().contains("sorting_stations"));
    }

    #[test]
    fn test_package_loaded_event() {
        let event = PackageLoadedEvent {
            package_id: "PKG-789".to_string(),
            driver_id: "D1".to_string(),
            loading_dock_id: "LD1".to_string(),
        };

        assert_eq!(event.event_type(), "io.logistics.package.loaded");
        let json = event.to_json();
        assert_eq!(json["driver_id"], "D1");
    }

    #[test]
    fn test_package_in_transit_event() {
        let event = PackageInTransitEvent {
            package_id: "PKG-111".to_string(),
            driver_id: "D2".to_string(),
            estimated_arrival: Utc::now(),
        };

        assert_eq!(event.event_type(), "io.logistics.package.in_transit");
    }

    #[test]
    fn test_package_delivered_event() {
        let event = PackageDeliveredEvent {
            package_id: "PKG-999".to_string(),
            driver_id: "D3".to_string(),
            delivery_time: Utc::now(),
            destination: Point::new(5.0, 5.0),
        };

        assert_eq!(event.event_type(), "io.logistics.package.delivered");
        let json = event.to_json();
        assert_eq!(json["destination_x"], 5.0);
        assert_eq!(json["destination_y"], 5.0);
    }
}
