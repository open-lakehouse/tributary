//! Driver Agent for Package Delivery
//!
//! Drivers transport packages from warehouses to delivery addresses.
//! They maintain autonomous state machines for traveling, loading, and delivering.
//!
//! # State Machine
//!
//! ```text
//!     ┌──────┐
//!     │ Idle │◄────────────────────┐
//!     └──┬───┘                     │
//!        │ assigned packages       │
//!        ▼                         │
//! ┌─────────────────┐             │
//! │ TravelingToWarehouse │        │
//! └──────┬──────────┘             │
//!        │ arrived at warehouse    │
//!        ▼                         │
//! ┌─────────────────┐             │
//! │ LoadingPackages │             │
//! └──────┬──────────┘             │
//!        │ loading complete        │
//!        ▼                         │
//! ┌──────────────────┐            │
//! │ DeliveringPackages│            │
//! └──────┬───────────┘            │
//!        │ all delivered           │
//!        ▼                         │
//! ┌─────────────┐                 │
//! │  Returning  │─────────────────┘
//! └─────────────┘      back to base
//! ```

use geo::Point;
use serde_json::Value;
use sim_core::{
    Agent, AgentId, AgentStatus, Result, SimulationContext,
    population::{AgentTickResult, ArrowAgentRow},
};
use uuid::Uuid;

/// Driver status states
#[derive(Debug, Clone, PartialEq)]
pub enum DriverState {
    /// Driver is idle and available for assignments
    Idle,

    /// Driver is traveling to warehouse to pick up packages
    TravelingToWarehouse {
        warehouse_location: Point<f64>,
        warehouse_id: String,
    },

    /// Driver is at warehouse loading packages
    LoadingPackages {
        warehouse_id: String,
        package_count: u32,
        loading_started_at: u64, // tick count
    },

    /// Driver is delivering packages to destinations
    DeliveringPackages {
        current_destination: Point<f64>,
        remaining_stops: u32,
        packages_on_board: Vec<String>,
    },

    /// Driver is returning to home base
    Returning { home_location: Point<f64> },
}

impl AgentStatus for DriverState {
    fn as_str(&self) -> &str {
        match self {
            Self::Idle => "idle",
            Self::TravelingToWarehouse { .. } => "traveling_to_warehouse",
            Self::LoadingPackages { .. } => "loading_packages",
            Self::DeliveringPackages { .. } => "delivering_packages",
            Self::Returning { .. } => "returning",
        }
    }

    fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    fn is_active(&self) -> bool {
        !matches!(self, Self::Idle)
    }
}

/// Driver Agent
///
/// Autonomous agent that picks up and delivers packages. Maintains position
/// tracking with movement and handles multi-stop delivery routes.
#[derive(Debug, Clone)]
pub struct Driver {
    pub id: AgentId,
    pub state: DriverState,
    pub location: Point<f64>,
    pub home_location: Point<f64>,
    pub movement_speed: f64, // meters per second
    pub capacity: u32,       // max packages per trip
    pub tick_count: u64,     // internal tick counter for timing
    // Temporary storage for state transitions
    metadata_destinations: Vec<Point<f64>>,
    metadata_package_ids: Vec<String>,
}

impl Driver {
    /// Create a new driver at a home location
    ///
    /// # Arguments
    ///
    /// * `home_location` - Starting location for the driver
    pub fn new(home_location: Point<f64>) -> Self {
        Self {
            id: AgentId::new(),
            state: DriverState::Idle,
            location: home_location,
            home_location,
            movement_speed: 15.0, // 15 m/s ≈ 54 km/h
            capacity: 20,
            tick_count: 0,
            metadata_destinations: Vec::new(),
            metadata_package_ids: Vec::new(),
        }
    }

    /// Create a driver with custom parameters
    pub fn with_capacity(home_location: Point<f64>, capacity: u32, speed: f64) -> Self {
        Self {
            id: AgentId::new(),
            state: DriverState::Idle,
            location: home_location,
            home_location,
            movement_speed: speed,
            capacity,
            tick_count: 0,
            metadata_destinations: Vec::new(),
            metadata_package_ids: Vec::new(),
        }
    }

    /// Check if driver is idle and available
    pub fn is_idle(&self) -> bool {
        matches!(self.state, DriverState::Idle)
    }

    /// Assign a delivery task to this driver
    ///
    /// # Arguments
    ///
    /// * `warehouse_location` - Location of warehouse for pickup
    /// * `warehouse_id` - Warehouse identifier
    /// * `destinations` - List of delivery addresses
    /// * `package_ids` - List of package tracking numbers
    ///
    /// # Panics
    ///
    /// Panics if driver is not in Idle state or if package count exceeds capacity
    pub fn assign_delivery(
        &mut self,
        warehouse_location: Point<f64>,
        warehouse_id: String,
        destinations: Vec<Point<f64>>,
        package_ids: Vec<String>,
    ) {
        assert!(self.is_idle(), "Driver must be idle to accept assignment");
        assert!(
            package_ids.len() <= self.capacity as usize,
            "Package count exceeds driver capacity"
        );

        self.state = DriverState::TravelingToWarehouse {
            warehouse_location,
            warehouse_id,
        };

        // Store metadata for later transitions
        self.metadata_destinations = destinations;
        self.metadata_package_ids = package_ids;
    }

    /// Move towards a target location
    ///
    /// Returns true if arrived at target
    fn move_towards(&mut self, target: Point<f64>, step_duration_secs: f64) -> bool {
        let dx = target.x() - self.location.x();
        let dy = target.y() - self.location.y();
        let distance = (dx * dx + dy * dy).sqrt();

        let max_travel = self.movement_speed * step_duration_secs;

        if distance <= max_travel {
            self.location = target;
            true
        } else {
            let ratio = max_travel / distance;
            let new_x = self.location.x() + dx * ratio;
            let new_y = self.location.y() + dy * ratio;
            self.location = Point::new(new_x, new_y);
            false
        }
    }

    /// Get current package count
    pub fn current_package_count(&self) -> u32 {
        match &self.state {
            DriverState::LoadingPackages { package_count, .. } => *package_count,
            DriverState::DeliveringPackages {
                packages_on_board, ..
            } => packages_on_board.len() as u32,
            _ => 0,
        }
    }
}

impl Agent for Driver {
    fn id(&self) -> AgentId {
        self.id
    }

    fn agent_type(&self) -> &str {
        "driver"
    }

    fn status(&self) -> &dyn AgentStatus {
        &self.state
    }

    fn position(&self) -> Option<Point<f64>> {
        Some(self.location)
    }

    fn tick(&mut self, _ctx: &SimulationContext) -> Result<AgentTickResult> {
        let mut result = AgentTickResult::default();
        self.tick_count += 1;

        // Simulation step duration (30 seconds default)
        let step_duration = 30.0;

        match &self.state.clone() {
            DriverState::Idle => {
                // Waiting for assignment
            }

            DriverState::TravelingToWarehouse {
                warehouse_location,
                warehouse_id,
            } => {
                let arrived = self.move_towards(*warehouse_location, step_duration);

                if arrived {
                    // Start loading packages
                    let package_count = self.metadata_package_ids.len() as u32;
                    self.state = DriverState::LoadingPackages {
                        warehouse_id: warehouse_id.clone(),
                        package_count,
                        loading_started_at: self.tick_count,
                    };

                    result.state_changes.insert(
                        "arrived_at_warehouse".to_string(),
                        Value::String(warehouse_id.clone()),
                    );
                }
            }

            DriverState::LoadingPackages {
                warehouse_id,
                package_count,
                loading_started_at,
            } => {
                // Loading takes 12 ticks (6 minutes at 30 sec/tick)
                let loading_duration = 12;
                let elapsed_ticks = self.tick_count - loading_started_at;

                if elapsed_ticks >= loading_duration {
                    // Loading complete, start delivering
                    let destinations = self.metadata_destinations.clone();
                    let package_ids = self.metadata_package_ids.clone();

                    if let Some(first_dest) = destinations.first() {
                        self.state = DriverState::DeliveringPackages {
                            current_destination: *first_dest,
                            remaining_stops: destinations.len() as u32 - 1,
                            packages_on_board: package_ids,
                        };

                        // Clear metadata
                        self.metadata_destinations = destinations[1..].to_vec();
                        self.metadata_package_ids = Vec::new();

                        result.state_changes.insert(
                            "loading_complete".to_string(),
                            Value::Number((*package_count).into()),
                        );
                        result.state_changes.insert(
                            "warehouse_id".to_string(),
                            Value::String(warehouse_id.clone()),
                        );
                    }
                }
            }

            DriverState::DeliveringPackages {
                current_destination,
                remaining_stops,
                packages_on_board,
            } => {
                let arrived = self.move_towards(*current_destination, step_duration);

                if arrived {
                    // Deliver current package
                    result.state_changes.insert(
                        "package_delivered".to_string(),
                        Value::String(format!("{:?}", current_destination)),
                    );

                    if *remaining_stops > 0 {
                        // More deliveries to make
                        if let Some(next_dest) = self.metadata_destinations.first() {
                            self.state = DriverState::DeliveringPackages {
                                current_destination: *next_dest,
                                remaining_stops: remaining_stops - 1,
                                packages_on_board: packages_on_board.clone(),
                            };
                            self.metadata_destinations = self.metadata_destinations[1..].to_vec();
                        }
                    } else {
                        // All deliveries complete, return to base
                        self.state = DriverState::Returning {
                            home_location: self.home_location,
                        };

                        result
                            .state_changes
                            .insert("all_deliveries_complete".to_string(), Value::Bool(true));
                    }
                }
            }

            DriverState::Returning { home_location } => {
                let arrived = self.move_towards(*home_location, step_duration);

                if arrived {
                    // Back to idle
                    self.state = DriverState::Idle;

                    // Clear all metadata
                    self.metadata_destinations.clear();
                    self.metadata_package_ids.clear();

                    result
                        .state_changes
                        .insert("returned_to_base".to_string(), Value::Bool(true));
                }
            }
        }

        Ok(result)
    }

    fn state_data(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        map.insert(
            "home_x".to_string(),
            Value::Number(serde_json::Number::from_f64(self.home_location.x()).unwrap()),
        );
        map.insert(
            "home_y".to_string(),
            Value::Number(serde_json::Number::from_f64(self.home_location.y()).unwrap()),
        );
        map.insert("capacity".to_string(), Value::Number(self.capacity.into()));

        match &self.state {
            DriverState::TravelingToWarehouse {
                warehouse_id,
                warehouse_location,
            } => {
                map.insert(
                    "warehouse_id".to_string(),
                    Value::String(warehouse_id.clone()),
                );
                map.insert(
                    "warehouse_x".to_string(),
                    Value::Number(serde_json::Number::from_f64(warehouse_location.x()).unwrap()),
                );
                map.insert(
                    "warehouse_y".to_string(),
                    Value::Number(serde_json::Number::from_f64(warehouse_location.y()).unwrap()),
                );
            }
            DriverState::LoadingPackages {
                warehouse_id,
                package_count,
                ..
            } => {
                map.insert(
                    "warehouse_id".to_string(),
                    Value::String(warehouse_id.clone()),
                );
                map.insert(
                    "package_count".to_string(),
                    Value::Number((*package_count).into()),
                );
            }
            DriverState::DeliveringPackages {
                remaining_stops,
                packages_on_board,
                ..
            } => {
                map.insert(
                    "remaining_stops".to_string(),
                    Value::Number((*remaining_stops).into()),
                );
                map.insert(
                    "packages_on_board".to_string(),
                    Value::Number((packages_on_board.len() as u32).into()),
                );
            }
            _ => {}
        }

        Value::Object(map)
    }

    fn from_arrow_row(row: &ArrowAgentRow) -> Result<Self> {
        use sim_core::Error;

        let id = AgentId::from_uuid(
            Uuid::from_slice(&row.id).map_err(|e| Error::Agent(format!("Invalid UUID: {}", e)))?,
        );

        let location = if let (Some(x), Some(y)) = (row.position_x, row.position_y) {
            Point::new(x, y)
        } else {
            Point::new(0.0, 0.0)
        };

        let metadata: serde_json::Value = if let Some(ref m) = row.metadata {
            serde_json::from_str(m).unwrap_or_default()
        } else {
            serde_json::Value::Null
        };

        let home_x = metadata
            .get("home_x")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let home_y = metadata
            .get("home_y")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let home_location = Point::new(home_x, home_y);

        let capacity = metadata
            .get("capacity")
            .and_then(|v| v.as_u64())
            .unwrap_or(20) as u32;

        let state = match row.status.as_str() {
            "idle" => DriverState::Idle,
            "returning" => DriverState::Returning { home_location },
            _ => DriverState::Idle,
        };

        Ok(Driver {
            id,
            state,
            location,
            home_location,
            movement_speed: 15.0,
            capacity,
            tick_count: 0,
            metadata_destinations: Vec::new(),
            metadata_package_ids: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_driver_creation() {
        let home = Point::new(-122.4, 37.8);
        let driver = Driver::new(home);

        assert_eq!(driver.agent_type(), "driver");
        assert!(driver.is_idle());
        assert_eq!(driver.location, home);
        assert_eq!(driver.capacity, 20);
    }

    #[test]
    fn test_driver_with_custom_capacity() {
        let home = Point::new(0.0, 0.0);
        let driver = Driver::with_capacity(home, 10, 20.0);

        assert_eq!(driver.capacity, 10);
        assert_eq!(driver.movement_speed, 20.0);
    }

    #[test]
    fn test_driver_state_idle() {
        let driver = Driver::new(Point::new(0.0, 0.0));
        assert_eq!(driver.state.as_str(), "idle");
        assert!(driver.state.is_idle());
        assert!(!driver.state.is_active());
    }

    #[test]
    fn test_assign_delivery() {
        let home = Point::new(0.0, 0.0);
        let mut driver = Driver::new(home);

        let warehouse = Point::new(1.0, 1.0);
        let destinations = vec![Point::new(2.0, 2.0), Point::new(3.0, 3.0)];
        let package_ids = vec!["pkg-1".to_string(), "pkg-2".to_string()];

        driver.assign_delivery(
            warehouse,
            "warehouse-1".to_string(),
            destinations,
            package_ids,
        );

        assert_eq!(driver.state.as_str(), "traveling_to_warehouse");
        assert!(!driver.is_idle());
    }

    #[test]
    #[should_panic(expected = "Package count exceeds driver capacity")]
    fn test_assign_delivery_exceeds_capacity() {
        let mut driver = Driver::with_capacity(Point::new(0.0, 0.0), 2, 15.0);

        let warehouse = Point::new(1.0, 1.0);
        let destinations = vec![Point::new(2.0, 2.0); 5];
        let package_ids = vec!["pkg-1".to_string(); 5];

        driver.assign_delivery(warehouse, "w1".to_string(), destinations, package_ids);
    }

    #[test]
    fn test_movement_towards_target() {
        let home = Point::new(0.0, 0.0);
        let mut driver = Driver::new(home);

        let target = Point::new(100.0, 0.0);

        // Move towards target (30 second step, 15 m/s = 450m travel)
        let arrived = driver.move_towards(target, 30.0);

        assert!(arrived);
        assert_eq!(driver.location, target);
    }

    #[test]
    fn test_movement_partial_progress() {
        let home = Point::new(0.0, 0.0);
        let mut driver = Driver::new(home);

        let target = Point::new(1000.0, 0.0);

        // Move towards target (can only travel 450m)
        let arrived = driver.move_towards(target, 30.0);

        assert!(!arrived);
        assert!(driver.location.x() > 0.0);
        assert!(driver.location.x() < 1000.0);
    }

    #[test]
    fn test_transition_to_loading() {
        let home = Point::new(0.0, 0.0);
        let mut driver = Driver::new(home);

        // Very close warehouse for immediate arrival
        let warehouse = Point::new(0.001, 0.001);
        let destinations = vec![Point::new(2.0, 2.0)];
        let package_ids = vec!["pkg-1".to_string()];

        driver.assign_delivery(
            warehouse,
            "warehouse-1".to_string(),
            destinations,
            package_ids,
        );

        let ctx = SimulationContext::from_now(sim_core::SimulationConfig::default());
        let result = driver.tick(&ctx).unwrap();

        // Should transition to loading
        assert_eq!(driver.state.as_str(), "loading_packages");
        assert!(result.state_changes.contains_key("arrived_at_warehouse"));
    }

    #[test]
    fn test_loading_duration() {
        let mut driver = Driver::new(Point::new(0.0, 0.0));
        driver.state = DriverState::LoadingPackages {
            warehouse_id: "w1".to_string(),
            package_count: 5,
            loading_started_at: 0,
        };
        driver.metadata_destinations = vec![Point::new(1.0, 1.0)];
        driver.metadata_package_ids = vec!["pkg-1".to_string()];

        let ctx = SimulationContext::from_now(sim_core::SimulationConfig::default());

        // Loading takes 12 ticks
        for _ in 0..11 {
            driver.tick(&ctx).unwrap();
            assert_eq!(driver.state.as_str(), "loading_packages");
        }

        // 12th tick should complete loading
        let result = driver.tick(&ctx).unwrap();
        assert_eq!(driver.state.as_str(), "delivering_packages");
        assert!(result.state_changes.contains_key("loading_complete"));
    }

    #[test]
    fn test_delivery_completion() {
        let mut driver = Driver::new(Point::new(0.0, 0.0));

        // Set up delivering state with close destination
        driver.state = DriverState::DeliveringPackages {
            current_destination: Point::new(0.001, 0.001),
            remaining_stops: 0,
            packages_on_board: vec!["pkg-1".to_string()],
        };

        let ctx = SimulationContext::from_now(sim_core::SimulationConfig::default());
        let result = driver.tick(&ctx).unwrap();

        // Should transition to returning
        assert_eq!(driver.state.as_str(), "returning");
        assert!(result.state_changes.contains_key("all_deliveries_complete"));
    }

    #[test]
    fn test_return_to_base() {
        let home = Point::new(0.0, 0.0);
        let mut driver = Driver::new(home);

        // Set up returning state close to home
        driver.state = DriverState::Returning {
            home_location: Point::new(0.001, 0.001),
        };

        let ctx = SimulationContext::from_now(sim_core::SimulationConfig::default());
        let result = driver.tick(&ctx).unwrap();

        // Should transition back to idle
        assert!(driver.is_idle());
        assert!(result.state_changes.contains_key("returned_to_base"));
    }

    #[test]
    fn test_current_package_count() {
        let mut driver = Driver::new(Point::new(0.0, 0.0));

        assert_eq!(driver.current_package_count(), 0);

        driver.state = DriverState::LoadingPackages {
            warehouse_id: "w1".to_string(),
            package_count: 5,
            loading_started_at: 0,
        };
        assert_eq!(driver.current_package_count(), 5);

        driver.state = DriverState::DeliveringPackages {
            current_destination: Point::new(1.0, 1.0),
            remaining_stops: 2,
            packages_on_board: vec!["p1".to_string(), "p2".to_string(), "p3".to_string()],
        };
        assert_eq!(driver.current_package_count(), 3);
    }

    #[test]
    fn test_driver_arrow_round_trip() {
        let home = Point::new(1.0, 2.0);
        let driver = Driver::with_capacity(home, 15, 20.0);

        let row = driver.to_arrow_row();
        let restored = Driver::from_arrow_row(&row).unwrap();

        assert_eq!(driver.id, restored.id);
        assert_eq!(driver.state.as_str(), restored.state.as_str());
        assert_eq!(driver.home_location, restored.home_location);
        assert_eq!(driver.capacity, restored.capacity);
    }
}
