//! Courier Agent for Food Delivery
//!
//! Couriers are responsible for picking up prepared food orders from kitchens
//! and delivering them to customers. This agent implements a state machine that handles
//! the complete delivery lifecycle.
//!
//! # State Machine
//!
//! ```text
//!     ┌──────┐
//!     │ Idle │◄──────────────────┐
//!     └──┬───┘                   │
//!        │ order assigned        │
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
//! use sim_kitchen::agents::Courier;
//! use geo::Point;
//! use sim_core::{Agent, SimulationContext, SimulationConfig};
//!
//! // Create a courier at a home location
//! let home = Point::new(-122.4, 37.8);
//! let mut courier = Courier::new(home);
//!
//! // Courier starts in Idle state
//! assert!(courier.is_idle());
//!
//! // Assign a delivery task
//! let kitchen = Point::new(-122.5, 37.9);
//! let customer = Point::new(-122.3, 37.7);
//! courier.assign_delivery(kitchen, customer, "order-123".to_string());
//!
//! // Run simulation
//! let ctx = SimulationContext::from_now(SimulationConfig::default());
//! courier.tick(&ctx).unwrap();
//! ```

use geo::Point;
use serde_json::Value;
use sim_core::{
    Agent, AgentId, AgentStatus, Result, SimulationContext,
    population::{AgentTickResult, ArrowAgentRow},
};
use uuid::Uuid;

/// Courier status states
///
/// The courier progresses through these states in sequence:
/// Idle → PickingUp → Delivering → Returning → Idle
#[derive(Debug, Clone, PartialEq)]
pub enum CourierState {
    /// Courier is idle and available for assignments
    Idle,

    /// Courier is traveling to kitchen to pick up order
    PickingUp {
        /// ID of the order being picked up
        order_id: String,
        /// Location of the kitchen
        kitchen_location: Point<f64>,
    },

    /// Courier has order and is delivering to customer
    Delivering {
        /// ID of the order being delivered
        order_id: String,
        /// ID of the customer
        customer_id: String,
        /// Location of the customer
        customer_location: Point<f64>,
    },

    /// Courier is returning to home location after delivery
    Returning {
        /// Home location to return to
        home_location: Point<f64>,
    },
}

impl AgentStatus for CourierState {
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

/// Courier Agent
///
/// Autonomous agent that handles food delivery by picking up prepared orders
/// from kitchens and delivering to customers. The courier maintains its
/// own state machine and location tracking.
///
/// # Fields
///
/// - `id`: Unique identifier for this courier
/// - `state`: Current state in the delivery lifecycle
/// - `location`: Current geographic position
/// - `home_location`: Base location where courier starts and returns
/// - `movement_speed`: Speed in meters per second (default: 12.0 m/s ≈ 43 km/h)
#[derive(Debug, Clone)]
pub struct Courier {
    pub id: AgentId,
    pub state: CourierState,
    pub location: Point<f64>,
    pub home_location: Point<f64>,
    pub movement_speed: f64, // meters per second
    // Temporary storage for state transitions
    metadata_customer_id: Option<String>,
    metadata_customer_location: Option<Point<f64>>,
}

impl Courier {
    /// Create a new courier at a home location
    ///
    /// # Arguments
    ///
    /// * `home_location` - Starting location for the courier
    ///
    /// # Example
    ///
    /// ```rust
    /// use sim_kitchen::agents::Courier;
    /// use geo::Point;
    ///
    /// let courier = Courier::new(Point::new(-122.4, 37.8));
    /// assert!(courier.is_idle());
    /// ```
    pub fn new(home_location: Point<f64>) -> Self {
        Self {
            id: AgentId::new(),
            state: CourierState::Idle,
            location: home_location,
            home_location,
            movement_speed: 12.0, // 12 m/s ≈ 43 km/h (slightly faster than supply driver)
            metadata_customer_id: None,
            metadata_customer_location: None,
        }
    }

    /// Create a courier with custom parameters
    ///
    /// # Arguments
    ///
    /// * `home_location` - Starting location
    /// * `movement_speed` - Speed in meters per second
    pub fn with_speed(home_location: Point<f64>, movement_speed: f64) -> Self {
        Self {
            id: AgentId::new(),
            state: CourierState::Idle,
            location: home_location,
            home_location,
            movement_speed,
            metadata_customer_id: None,
            metadata_customer_location: None,
        }
    }

    /// Check if courier is idle and available for assignment
    pub fn is_idle(&self) -> bool {
        matches!(self.state, CourierState::Idle)
    }

    /// Assign a delivery task to this courier
    ///
    /// Transitions from Idle to PickingUp state.
    ///
    /// # Arguments
    ///
    /// * `kitchen_location` - Location of the kitchen with prepared food
    /// * `customer_location` - Location of the customer
    /// * `order_id` - Unique order identifier
    ///
    /// # Panics
    ///
    /// Panics if courier is not in Idle state
    pub fn assign_delivery(
        &mut self,
        kitchen_location: Point<f64>,
        customer_location: Point<f64>,
        order_id: String,
        customer_id: String,
    ) {
        assert!(self.is_idle(), "Courier must be idle to accept assignment");

        self.state = CourierState::PickingUp {
            order_id: order_id.clone(),
            kitchen_location,
        };

        // Store customer info for later transition
        self.metadata_customer_id = Some(customer_id);
        self.metadata_customer_location = Some(customer_location);
    }

    /// Move towards a target location
    ///
    /// Updates courier's location based on movement speed and simulation step duration.
    /// Returns true if arrived at target.
    fn move_towards(&mut self, target: Point<f64>, step_duration_secs: f64) -> bool {
        // Calculate distance to target (simplified Euclidean distance)
        let dx = target.x() - self.location.x();
        let dy = target.y() - self.location.y();
        let distance = (dx * dx + dy * dy).sqrt();

        // Distance we can travel this step
        let max_travel = self.movement_speed * step_duration_secs;

        if distance <= max_travel {
            // Arrived at target
            self.location = target;
            true
        } else {
            // Move towards target
            let ratio = max_travel / distance;
            let new_x = self.location.x() + dx * ratio;
            let new_y = self.location.y() + dy * ratio;
            self.location = Point::new(new_x, new_y);
            false
        }
    }
}

impl Agent for Courier {
    fn id(&self) -> AgentId {
        self.id
    }

    fn agent_type(&self) -> &str {
        "courier"
    }

    fn status(&self) -> &dyn AgentStatus {
        &self.state
    }

    fn position(&self) -> Option<Point<f64>> {
        Some(self.location)
    }

    fn tick(&mut self, _ctx: &SimulationContext) -> Result<AgentTickResult> {
        let mut result = AgentTickResult::default();

        // Simulation step duration (30 seconds default)
        let step_duration = 30.0;

        match &self.state.clone() {
            CourierState::Idle => {
                // Waiting for assignment
                // External system will call assign_delivery()
            }

            CourierState::PickingUp {
                kitchen_location,
                order_id,
            } => {
                // Move towards kitchen
                let arrived = self.move_towards(*kitchen_location, step_duration);

                if arrived {
                    // Pick up order and transition to Delivering
                    let customer_id = self
                        .metadata_customer_id
                        .take()
                        .expect("Customer ID must be set");
                    let customer_location = self
                        .metadata_customer_location
                        .expect("Customer location must be set");

                    self.state = CourierState::Delivering {
                        order_id: order_id.clone(),
                        customer_id: customer_id.clone(),
                        customer_location,
                    };

                    // Record state change
                    result.state_changes.insert(
                        "picked_up_order".to_string(),
                        Value::String(order_id.clone()),
                    );
                    result.state_changes.insert(
                        "state_transition".to_string(),
                        Value::String("picking_up_to_delivering".to_string()),
                    );
                }
            }

            CourierState::Delivering {
                customer_location,
                order_id,
                customer_id,
            } => {
                // Move towards customer
                let arrived = self.move_towards(*customer_location, step_duration);

                if arrived {
                    // Deliver order and transition to Returning
                    self.state = CourierState::Returning {
                        home_location: self.home_location,
                    };

                    // Record state change
                    result.state_changes.insert(
                        "delivered_order".to_string(),
                        Value::String(order_id.clone()),
                    );
                    result.state_changes.insert(
                        "customer_id".to_string(),
                        Value::String(customer_id.clone()),
                    );
                    result.state_changes.insert(
                        "state_transition".to_string(),
                        Value::String("delivering_to_returning".to_string()),
                    );
                }
            }

            CourierState::Returning { home_location, .. } => {
                // Move towards home
                let arrived = self.move_towards(*home_location, step_duration);

                if arrived {
                    // Back to idle, ready for next assignment
                    self.state = CourierState::Idle;

                    // Clear metadata
                    self.metadata_customer_id = None;
                    self.metadata_customer_location = None;

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
            CourierState::PickingUp {
                order_id,
                kitchen_location,
            } => {
                map.insert("order_id".to_string(), Value::String(order_id.clone()));
                map.insert(
                    "kitchen_x".to_string(),
                    Value::Number(serde_json::Number::from_f64(kitchen_location.x()).unwrap()),
                );
                map.insert(
                    "kitchen_y".to_string(),
                    Value::Number(serde_json::Number::from_f64(kitchen_location.y()).unwrap()),
                );
            }
            CourierState::Delivering {
                order_id,
                customer_id,
                customer_location,
            } => {
                map.insert("order_id".to_string(), Value::String(order_id.clone()));
                map.insert(
                    "customer_id".to_string(),
                    Value::String(customer_id.clone()),
                );
                map.insert(
                    "customer_x".to_string(),
                    Value::Number(serde_json::Number::from_f64(customer_location.x()).unwrap()),
                );
                map.insert(
                    "customer_y".to_string(),
                    Value::Number(serde_json::Number::from_f64(customer_location.y()).unwrap()),
                );
            }
            CourierState::Returning { home_location } => {
                map.insert(
                    "home_x".to_string(),
                    Value::Number(serde_json::Number::from_f64(home_location.x()).unwrap()),
                );
                map.insert(
                    "home_y".to_string(),
                    Value::Number(serde_json::Number::from_f64(home_location.y()).unwrap()),
                );
            }
            CourierState::Idle => {}
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
            "idle" => CourierState::Idle,
            "picking_up" => {
                let order_id = metadata
                    .get("order_id")
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
                CourierState::PickingUp {
                    order_id,
                    kitchen_location: Point::new(kitchen_x, kitchen_y),
                }
            }
            "delivering" => {
                let order_id = metadata
                    .get("order_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let customer_id = metadata
                    .get("customer_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let customer_x = metadata
                    .get("customer_x")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let customer_y = metadata
                    .get("customer_y")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                CourierState::Delivering {
                    order_id,
                    customer_id,
                    customer_location: Point::new(customer_x, customer_y),
                }
            }
            "returning" => CourierState::Returning { home_location },
            _ => CourierState::Idle,
        };

        Ok(Courier {
            id,
            state,
            location,
            home_location,
            movement_speed: 12.0,
            metadata_customer_id: None,
            metadata_customer_location: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::SimulationConfig;

    #[test]
    fn test_courier_creation() {
        let home = Point::new(-122.4, 37.8);
        let courier = Courier::new(home);

        assert_eq!(courier.agent_type(), "courier");
        assert!(courier.is_idle());
        assert_eq!(courier.location, home);
        assert_eq!(courier.home_location, home);
    }

    #[test]
    fn test_courier_state_idle() {
        let courier = Courier::new(Point::new(0.0, 0.0));
        let state = &courier.state;

        assert_eq!(state.as_str(), "idle");
        assert!(state.is_idle());
        assert!(!state.is_active());
    }

    #[test]
    fn test_transition_idle_to_picking_up() {
        let home = Point::new(0.0, 0.0);
        let mut courier = Courier::new(home);

        let kitchen = Point::new(1.0, 1.0);
        let customer = Point::new(2.0, 2.0);
        let order_id = "order-123".to_string();
        let customer_id = "customer-456".to_string();

        courier.assign_delivery(kitchen, customer, order_id.clone(), customer_id);

        // Should transition to PickingUp
        assert!(!courier.is_idle());
        assert_eq!(courier.state.as_str(), "picking_up");

        match &courier.state {
            CourierState::PickingUp {
                kitchen_location,
                order_id: oid,
            } => {
                assert_eq!(*kitchen_location, kitchen);
                assert_eq!(oid, &order_id);
            }
            _ => panic!("Expected PickingUp state"),
        }
    }

    #[test]
    fn test_transition_picking_up_to_delivering() {
        let home = Point::new(0.0, 0.0);
        let mut courier = Courier::new(home);

        // Very close kitchen for immediate arrival
        let kitchen = Point::new(0.001, 0.001);
        let customer = Point::new(2.0, 2.0);
        let order_id = "order-789".to_string();
        let customer_id = "customer-101".to_string();

        courier.assign_delivery(kitchen, customer, order_id.clone(), customer_id.clone());

        // Tick to move towards kitchen
        let ctx = SimulationContext::from_now(SimulationConfig::default());
        let result = courier.tick(&ctx).unwrap();

        // Should have transitioned to Delivering
        assert_eq!(courier.state.as_str(), "delivering");
        assert!(result.state_changes.contains_key("picked_up_order"));

        match &courier.state {
            CourierState::Delivering {
                customer_location,
                order_id: oid,
                customer_id: cid,
            } => {
                assert_eq!(*customer_location, customer);
                assert_eq!(oid, &order_id);
                assert_eq!(cid, &customer_id);
            }
            _ => panic!("Expected Delivering state"),
        }
    }

    #[test]
    fn test_transition_delivering_to_returning() {
        let home = Point::new(0.0, 0.0);
        let mut courier = Courier::new(home);

        // Manually set state to Delivering at a very close location
        courier.state = CourierState::Delivering {
            order_id: "order-111".to_string(),
            customer_id: "customer-222".to_string(),
            customer_location: Point::new(0.001, 0.001),
        };

        let ctx = SimulationContext::from_now(SimulationConfig::default());
        let result = courier.tick(&ctx).unwrap();

        // Should transition to Returning
        assert_eq!(courier.state.as_str(), "returning");
        assert!(result.state_changes.contains_key("delivered_order"));
    }

    #[test]
    fn test_transition_returning_to_idle() {
        let home = Point::new(0.0, 0.0);
        let mut courier = Courier::new(home);

        // Set state to Returning at a very close location
        courier.state = CourierState::Returning {
            home_location: Point::new(0.001, 0.001),
        };

        let ctx = SimulationContext::from_now(SimulationConfig::default());
        let result = courier.tick(&ctx).unwrap();

        // Should transition back to Idle
        assert!(courier.is_idle());
        assert_eq!(courier.state.as_str(), "idle");
        assert!(result.state_changes.contains_key("returned_home"));
    }

    #[test]
    fn test_movement_towards_target() {
        let home = Point::new(0.0, 0.0);
        let mut courier = Courier::new(home);

        let target = Point::new(100.0, 0.0);

        // Move towards target (30 second step, 12 m/s = 360m travel)
        let arrived = courier.move_towards(target, 30.0);

        // Should arrive (distance is 100, can travel 360)
        assert!(arrived);
        assert_eq!(courier.location, target);
    }

    #[test]
    fn test_movement_partial_progress() {
        let home = Point::new(0.0, 0.0);
        let mut courier = Courier::new(home);

        let target = Point::new(1000.0, 0.0);

        // Move towards target (can only travel 360m)
        let arrived = courier.move_towards(target, 30.0);

        // Should not arrive yet
        assert!(!arrived);
        assert!(courier.location.x() > 0.0);
        assert!(courier.location.x() < 1000.0);
    }

    #[test]
    fn test_order_tracking() {
        let home = Point::new(0.0, 0.0);
        let mut courier = Courier::new(home);

        let order_id = "order-special".to_string();
        let customer_id = "customer-vip".to_string();

        // Set to Delivering state with order
        courier.state = CourierState::Delivering {
            order_id: order_id.clone(),
            customer_id: customer_id.clone(),
            customer_location: Point::new(1.0, 1.0),
        };

        match &courier.state {
            CourierState::Delivering {
                order_id: oid,
                customer_id: cid,
                ..
            } => {
                assert_eq!(oid, &order_id);
                assert_eq!(cid, &customer_id);
            }
            _ => panic!("Expected Delivering state"),
        }
    }

    #[test]
    fn test_courier_arrow_round_trip() {
        let home = Point::new(1.0, 2.0);

        // Test all state variants
        let test_cases = vec![
            Courier {
                id: AgentId::new(),
                state: CourierState::Idle,
                location: home,
                home_location: home,
                movement_speed: 12.0,
                metadata_customer_id: None,
                metadata_customer_location: None,
            },
            Courier {
                id: AgentId::new(),
                state: CourierState::PickingUp {
                    order_id: "order-123".to_string(),
                    kitchen_location: Point::new(3.0, 4.0),
                },
                location: Point::new(2.0, 3.0),
                home_location: home,
                movement_speed: 12.0,
                metadata_customer_id: None,
                metadata_customer_location: None,
            },
        ];

        for courier in test_cases {
            // Convert to Arrow row
            let row = courier.to_arrow_row();

            // Convert back
            let restored = Courier::from_arrow_row(&row).unwrap();

            // Verify key fields
            assert_eq!(courier.id, restored.id);
            assert_eq!(courier.state.as_str(), restored.state.as_str());
            assert_eq!(courier.home_location, restored.home_location);
        }
    }
}
