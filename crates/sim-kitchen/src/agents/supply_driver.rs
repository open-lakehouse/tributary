//! Supply Driver Agent for Supply Chain Logistics
//!
//! Supply drivers are responsible for picking up inventory from distribution sites
//! and delivering them to kitchens. This agent implements a state machine that handles
//! the complete delivery lifecycle.
//!
//! # State Machine
//!
//! ```text
//!     ┌──────┐
//!     │ Idle │◄──────────────────┐
//!     └──┬───┘                   │
//!        │ task assigned         │
//!        ▼                       │
//!   ┌──────────┐                │
//!   │ PickingUp│                │
//!   └────┬─────┘                │
//!        │ arrived + loaded      │
//!        ▼                       │
//!  ┌────────────┐               │
//!  │ Delivering │               │
//!  └─────┬──────┘               │
//!        │ delivered             │
//!        ▼                       │
//!   ┌──────────┐                │
//!   │ Returning│────────────────┘
//!   └──────────┘    back home
//! ```
//!
//! # Example Usage
//!
//! ```rust
//! use sim_kitchen::agents::SupplyDriver;
//! use geo::Point;
//! use sim_core::{Agent, SimulationContext, SimulationConfig};
//!
//! // Create a driver at a home location
//! let home = Point::new(-122.4, 37.8);
//! let mut driver = SupplyDriver::new(home);
//!
//! // Driver starts in Idle state
//! assert!(driver.is_idle());
//!
//! // Assign a delivery task
//! let site = Point::new(-122.5, 37.9);
//! let kitchen = Point::new(-122.3, 37.7);
//! driver.assign_delivery(site, kitchen, vec!["flour".to_string(), "sugar".to_string()]);
//!
//! // Run simulation
//! let ctx = SimulationContext::from_now(SimulationConfig::default());
//! driver.tick(&ctx).unwrap();
//! ```

use geo::Point;
use serde_json::Value;
use sim_core::{
    Agent, AgentId, AgentStatus, Result, SimulationContext, move_towards,
    population::{AgentTickResult, ArrowAgentRow},
};
use uuid::Uuid;

/// Supply driver status states
///
/// The driver progresses through these states in sequence:
/// Idle → PickingUp → Delivering → Returning → Idle
#[derive(Debug, Clone, PartialEq)]
pub enum DriverState {
    /// Driver is idle and available for assignments
    Idle,

    /// Driver is traveling to distribution site to pick up cargo
    PickingUp {
        /// ID of the distribution site
        site_id: String,
        /// Location of the distribution site
        site_location: Point<f64>,
    },

    /// Driver has cargo and is delivering to kitchen
    Delivering {
        /// ID of the target kitchen
        kitchen_id: String,
        /// Location of the target kitchen
        kitchen_location: Point<f64>,
        /// Cargo being transported (inventory items)
        cargo: Vec<String>,
    },

    /// Driver is returning to home location after delivery
    Returning {
        /// Home location to return to
        home_location: Point<f64>,
    },
}

impl AgentStatus for DriverState {
    fn as_str(&self) -> &str {
        match self {
            Self::Idle => "idle",
            Self::PickingUp { .. } => "picking_up",
            Self::Delivering { .. } => "delivering",
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

/// Supply Driver Agent
///
/// Autonomous agent that handles supply chain logistics by picking up inventory
/// from distribution sites and delivering to kitchens. The driver maintains its
/// own state machine and location tracking.
///
/// # Fields
///
/// - `id`: Unique identifier for this driver
/// - `state`: Current state in the delivery lifecycle
/// - `location`: Current geographic position
/// - `home_location`: Base location where driver starts and returns
/// - `movement_speed`: Speed in meters per second (default: 10.0 m/s ≈ 36 km/h)
#[derive(Debug, Clone)]
pub struct SupplyDriver {
    pub id: AgentId,
    pub state: DriverState,
    pub location: Point<f64>,
    pub home_location: Point<f64>,
    pub movement_speed: f64, // meters per second
    // Temporary storage for state transitions
    metadata_kitchen_location: Option<Point<f64>>,
    metadata_cargo: Option<Vec<String>>,
}

impl SupplyDriver {
    /// Create a new supply driver at a home location
    ///
    /// # Arguments
    ///
    /// * `home_location` - Starting location for the driver
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::agents::SupplyDriver;
    /// use geo::Point;
    ///
    /// let driver = SupplyDriver::new(Point::new(-122.4, 37.8));
    /// assert!(driver.is_idle());
    /// ```
    pub fn new(home_location: Point<f64>) -> Self {
        Self {
            id: AgentId::new(),
            state: DriverState::Idle,
            location: home_location,
            home_location,
            movement_speed: 10.0, // 10 m/s ≈ 36 km/h
            metadata_kitchen_location: None,
            metadata_cargo: None,
        }
    }

    /// Create a driver with custom parameters
    ///
    /// # Arguments
    ///
    /// * `home_location` - Starting location
    /// * `movement_speed` - Speed in meters per second
    pub fn with_speed(home_location: Point<f64>, movement_speed: f64) -> Self {
        Self {
            id: AgentId::new(),
            state: DriverState::Idle,
            location: home_location,
            home_location,
            movement_speed,
            metadata_kitchen_location: None,
            metadata_cargo: None,
        }
    }

    /// Check if driver is idle and available for assignment
    pub fn is_idle(&self) -> bool {
        matches!(self.state, DriverState::Idle)
    }

    /// Assign a delivery task to this driver
    ///
    /// Transitions from Idle to PickingUp state.
    ///
    /// # Arguments
    ///
    /// * `site_location` - Location of the distribution site
    /// * `kitchen_location` - Location of the target kitchen
    /// * `cargo` - List of inventory items to transport
    ///
    /// # Panics
    ///
    /// Panics if driver is not in Idle state
    pub fn assign_delivery(
        &mut self,
        site_location: Point<f64>,
        kitchen_location: Point<f64>,
        cargo: Vec<String>,
    ) {
        assert!(self.is_idle(), "Driver must be idle to accept assignment");

        self.state = DriverState::PickingUp {
            site_id: format!("site-{}", Uuid::new_v4()),
            site_location,
        };

        // Store kitchen info for later transition
        self.metadata_kitchen_location = Some(kitchen_location);
        self.metadata_cargo = Some(cargo);
    }

    /// Move towards a target location
    ///
    /// Updates driver's location based on movement speed and simulation step duration.
    /// Returns true if arrived at target.
    ///
    /// Delegates to [`sim_core::move_towards`] for the shared movement calculation.
    fn move_towards(&mut self, target: Point<f64>, step_duration_secs: f64) -> bool {
        move_towards(
            &mut self.location,
            target,
            self.movement_speed,
            step_duration_secs,
        )
    }
}

impl Agent for SupplyDriver {
    fn id(&self) -> AgentId {
        self.id
    }

    fn agent_type(&self) -> &str {
        "supply_driver"
    }

    fn status(&self) -> &dyn AgentStatus {
        &self.state
    }

    fn position(&self) -> Option<Point<f64>> {
        Some(self.location)
    }

    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        let mut result = AgentTickResult::default();

        let step_duration = ctx.time_step().num_seconds() as f64;

        match &self.state.clone() {
            DriverState::Idle => {
                // Waiting for assignment
                // External system will call assign_delivery()
            }

            DriverState::PickingUp { site_location, .. } => {
                // Move towards distribution site
                let arrived = self.move_towards(*site_location, step_duration);

                if arrived {
                    // Load cargo and transition to Delivering
                    let kitchen_location = self
                        .metadata_kitchen_location
                        .expect("Kitchen location must be set");
                    let cargo = self.metadata_cargo.take().expect("Cargo must be set");

                    self.state = DriverState::Delivering {
                        kitchen_id: format!("kitchen-{}", Uuid::new_v4()),
                        kitchen_location,
                        cargo: cargo.clone(),
                    };

                    // Record state change
                    result.state_changes.insert(
                        "loaded_cargo".to_string(),
                        Value::Array(cargo.iter().map(|s| Value::String(s.clone())).collect()),
                    );
                    result.state_changes.insert(
                        "state_transition".to_string(),
                        Value::String("picking_up_to_delivering".to_string()),
                    );
                }
            }

            DriverState::Delivering {
                kitchen_location,
                cargo,
                ..
            } => {
                // Move towards kitchen
                let arrived = self.move_towards(*kitchen_location, step_duration);

                if arrived {
                    // Unload cargo and transition to Returning
                    self.state = DriverState::Returning {
                        home_location: self.home_location,
                    };

                    // Record state change
                    result.state_changes.insert(
                        "delivered_cargo".to_string(),
                        Value::Array(cargo.iter().map(|s| Value::String(s.clone())).collect()),
                    );
                    result.state_changes.insert(
                        "state_transition".to_string(),
                        Value::String("delivering_to_returning".to_string()),
                    );
                }
            }

            DriverState::Returning { home_location, .. } => {
                // Move towards home
                let arrived = self.move_towards(*home_location, step_duration);

                if arrived {
                    // Back to idle, ready for next assignment
                    self.state = DriverState::Idle;

                    // Clear metadata
                    self.metadata_kitchen_location = None;
                    self.metadata_cargo = None;

                    // Record state change
                    result.state_changes.insert(
                        "state_transition".to_string(),
                        Value::String("returning_to_idle".to_string()),
                    );
                    result
                        .state_changes
                        .insert("returned_home".to_string(), Value::Bool(true));
                }
            }
        }

        Ok(result)
    }

    fn state_data(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        match &self.state {
            DriverState::PickingUp {
                site_id,
                site_location,
            } => {
                map.insert("site_id".to_string(), Value::String(site_id.clone()));
                map.insert(
                    "site_x".to_string(),
                    Value::Number(serde_json::Number::from_f64(site_location.x()).unwrap()),
                );
                map.insert(
                    "site_y".to_string(),
                    Value::Number(serde_json::Number::from_f64(site_location.y()).unwrap()),
                );
            }
            DriverState::Delivering {
                kitchen_id,
                kitchen_location,
                cargo,
            } => {
                map.insert("kitchen_id".to_string(), Value::String(kitchen_id.clone()));
                map.insert(
                    "kitchen_x".to_string(),
                    Value::Number(serde_json::Number::from_f64(kitchen_location.x()).unwrap()),
                );
                map.insert(
                    "kitchen_y".to_string(),
                    Value::Number(serde_json::Number::from_f64(kitchen_location.y()).unwrap()),
                );
                map.insert(
                    "cargo".to_string(),
                    Value::Array(cargo.iter().map(|s| Value::String(s.clone())).collect()),
                );
            }
            DriverState::Returning { home_location } => {
                map.insert(
                    "home_x".to_string(),
                    Value::Number(serde_json::Number::from_f64(home_location.x()).unwrap()),
                );
                map.insert(
                    "home_y".to_string(),
                    Value::Number(serde_json::Number::from_f64(home_location.y()).unwrap()),
                );
            }
            DriverState::Idle => {}
        }

        // Add home location
        map.insert(
            "home_x".to_string(),
            Value::Number(serde_json::Number::from_f64(self.home_location.x()).unwrap()),
        );
        map.insert(
            "home_y".to_string(),
            Value::Number(serde_json::Number::from_f64(self.home_location.y()).unwrap()),
        );

        Value::Object(map)
    }

    fn from_arrow_row(row: &ArrowAgentRow) -> Result<Self> {
        use sim_core::Error;

        // Parse UUID from binary ID
        let id = AgentId::from_uuid(
            Uuid::from_slice(&row.id).map_err(|e| Error::Agent(format!("Invalid UUID: {}", e)))?,
        );

        // Parse location from position
        let location = if let (Some(x), Some(y)) = (row.position_x, row.position_y) {
            Point::new(x, y)
        } else {
            Point::new(0.0, 0.0)
        };

        // Parse metadata
        let metadata: serde_json::Value = if let Some(ref m) = row.metadata {
            serde_json::from_str(m).unwrap_or_default()
        } else {
            serde_json::Value::Null
        };

        // Parse home location from metadata
        let home_x = metadata
            .get("home_x")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let home_y = metadata
            .get("home_y")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let home_location = Point::new(home_x, home_y);

        // Parse state from status string
        let state = match row.status.as_str() {
            "idle" => DriverState::Idle,
            "picking_up" => {
                let site_id = metadata
                    .get("site_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let site_x = metadata
                    .get("site_x")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let site_y = metadata
                    .get("site_y")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                DriverState::PickingUp {
                    site_id,
                    site_location: Point::new(site_x, site_y),
                }
            }
            "delivering" => {
                let kitchen_id = metadata
                    .get("kitchen_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let kitchen_x = metadata
                    .get("kitchen_x")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let kitchen_y = metadata
                    .get("kitchen_y")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let cargo = metadata
                    .get("cargo")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                DriverState::Delivering {
                    kitchen_id,
                    kitchen_location: Point::new(kitchen_x, kitchen_y),
                    cargo,
                }
            }
            "returning" => DriverState::Returning { home_location },
            _ => DriverState::Idle,
        };

        Ok(SupplyDriver {
            id,
            state,
            location,
            home_location,
            movement_speed: 10.0,
            metadata_kitchen_location: None,
            metadata_cargo: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::SimulationConfig;

    const STEP_SECS: f64 = 30.0;

    #[test]
    fn test_driver_creation() {
        let home = Point::new(-122.4, 37.8);
        let driver = SupplyDriver::new(home);

        assert_eq!(driver.agent_type(), "supply_driver");
        assert!(driver.is_idle());
        assert_eq!(driver.location, home);
        assert_eq!(driver.home_location, home);
    }

    #[test]
    fn test_driver_state_idle() {
        let driver = SupplyDriver::new(Point::new(0.0, 0.0));
        let state = &driver.state;

        assert_eq!(state.as_str(), "idle");
        assert!(state.is_idle());
        assert!(!state.is_active());
    }

    #[test]
    fn test_transition_idle_to_picking_up() {
        let home = Point::new(0.0, 0.0);
        let mut driver = SupplyDriver::new(home);

        let site = Point::new(1.0, 1.0);
        let kitchen = Point::new(2.0, 2.0);
        let cargo = vec!["flour".to_string(), "sugar".to_string()];

        driver.assign_delivery(site, kitchen, cargo.clone());

        // Should transition to PickingUp
        assert!(!driver.is_idle());
        assert_eq!(driver.state.as_str(), "picking_up");

        match &driver.state {
            DriverState::PickingUp { site_location, .. } => {
                assert_eq!(*site_location, site);
            }
            _ => panic!("Expected PickingUp state"),
        }
    }

    #[test]
    fn test_transition_picking_up_to_delivering() {
        let home = Point::new(0.0, 0.0);
        let mut driver = SupplyDriver::new(home);

        // Very close site for immediate arrival
        let site = Point::new(0.001, 0.001);
        let kitchen = Point::new(2.0, 2.0);
        let cargo = vec!["flour".to_string()];

        driver.assign_delivery(site, kitchen, cargo.clone());

        // Tick to move towards site
        let ctx = SimulationContext::from_now(SimulationConfig::default());
        let result = driver.tick(&ctx).unwrap();

        // Should have transitioned to Delivering
        assert_eq!(driver.state.as_str(), "delivering");
        assert!(result.state_changes.contains_key("loaded_cargo"));

        match &driver.state {
            DriverState::Delivering {
                kitchen_location,
                cargo: delivered_cargo,
                ..
            } => {
                assert_eq!(*kitchen_location, kitchen);
                assert_eq!(delivered_cargo, &cargo);
            }
            _ => panic!("Expected Delivering state"),
        }
    }

    #[test]
    fn test_transition_delivering_to_returning() {
        let home = Point::new(0.0, 0.0);
        let mut driver = SupplyDriver::new(home);

        // Manually set state to Delivering at a very close location
        driver.state = DriverState::Delivering {
            kitchen_id: "kitchen-1".to_string(),
            kitchen_location: Point::new(0.001, 0.001),
            cargo: vec!["flour".to_string()],
        };

        let ctx = SimulationContext::from_now(SimulationConfig::default());
        let result = driver.tick(&ctx).unwrap();

        // Should transition to Returning
        assert_eq!(driver.state.as_str(), "returning");
        assert!(result.state_changes.contains_key("delivered_cargo"));
    }

    #[test]
    fn test_transition_returning_to_idle() {
        let home = Point::new(0.0, 0.0);
        let mut driver = SupplyDriver::new(home);

        // Set state to Returning at a very close location
        driver.state = DriverState::Returning {
            home_location: Point::new(0.001, 0.001),
        };

        let ctx = SimulationContext::from_now(SimulationConfig::default());
        let result = driver.tick(&ctx).unwrap();

        // Should transition back to Idle
        assert!(driver.is_idle());
        assert_eq!(driver.state.as_str(), "idle");
        assert!(result.state_changes.contains_key("returned_home"));
    }

    #[test]
    fn test_movement_towards_target() {
        let home = Point::new(0.0, 0.0);
        let mut driver = SupplyDriver::new(home);

        let target = Point::new(100.0, 0.0);

        // Move towards target (30 second step, 10 m/s = 300m travel)
        let arrived = driver.move_towards(target, STEP_SECS);

        // Should arrive (distance is 100, can travel 300)
        assert!(arrived);
        assert_eq!(driver.location, target);
    }

    #[test]
    fn test_movement_partial_progress() {
        let home = Point::new(0.0, 0.0);
        let mut driver = SupplyDriver::new(home);

        let target = Point::new(1000.0, 0.0);

        // Move towards target (can only travel 300m)
        let arrived = driver.move_towards(target, STEP_SECS);

        // Should not arrive yet
        assert!(!arrived);
        assert!(driver.location.x() > 0.0);
        assert!(driver.location.x() < 1000.0);
    }

    #[test]
    fn test_cargo_tracking() {
        let home = Point::new(0.0, 0.0);
        let mut driver = SupplyDriver::new(home);

        let cargo = vec!["flour".to_string(), "sugar".to_string(), "eggs".to_string()];

        // Set to Delivering state with cargo
        driver.state = DriverState::Delivering {
            kitchen_id: "k1".to_string(),
            kitchen_location: Point::new(1.0, 1.0),
            cargo: cargo.clone(),
        };

        match &driver.state {
            DriverState::Delivering {
                cargo: current_cargo,
                ..
            } => {
                assert_eq!(current_cargo.len(), 3);
                assert_eq!(current_cargo, &cargo);
            }
            _ => panic!("Expected Delivering state"),
        }
    }

    #[test]
    fn test_driver_arrow_round_trip() {
        let home = Point::new(1.0, 2.0);

        // Test all state variants
        let test_cases = vec![
            SupplyDriver {
                id: AgentId::new(),
                state: DriverState::Idle,
                location: home,
                home_location: home,
                movement_speed: 10.0,
                metadata_kitchen_location: None,
                metadata_cargo: None,
            },
            SupplyDriver {
                id: AgentId::new(),
                state: DriverState::PickingUp {
                    site_id: "site-123".to_string(),
                    site_location: Point::new(3.0, 4.0),
                },
                location: Point::new(2.0, 3.0),
                home_location: home,
                movement_speed: 10.0,
                metadata_kitchen_location: None,
                metadata_cargo: None,
            },
        ];

        for driver in test_cases {
            // Convert to Arrow row
            let row = driver.to_arrow_row();

            // Convert back
            let restored = SupplyDriver::from_arrow_row(&row).unwrap();

            // Verify key fields
            assert_eq!(driver.id, restored.id);
            assert_eq!(driver.state.as_str(), restored.state.as_str());
            assert_eq!(driver.home_location, restored.home_location);
        }
    }
}
