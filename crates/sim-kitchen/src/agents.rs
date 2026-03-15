//! Customer agents for the ghost kitchen simulation

use chrono::{DateTime, Duration, Utc};
use geo::Point;
use rand::Rng;
use serde_json::Value;
use sim_core::{
    Agent, AgentId, AgentStatus, SimulationContext,
    population::{AgentTickResult, Demand},
};
use std::collections::HashMap;

/// Customer status states
#[derive(Debug, Clone, PartialEq)]
pub enum CustomerStatus {
    /// Customer is idle, may decide to order
    Idle,
    /// Customer is placing an order
    Ordering,
    /// Customer is waiting for food to be prepared
    WaitingForFood { order_id: String },
    /// Customer is eating
    Eating { finish_time: DateTime<Utc> },
    /// Customer has finished and will leave
    Done,
}

impl AgentStatus for CustomerStatus {
    fn as_str(&self) -> &str {
        match self {
            Self::Idle => "idle",
            Self::Ordering => "ordering",
            Self::WaitingForFood { .. } => "waiting_for_food",
            Self::Eating { .. } => "eating",
            Self::Done => "done",
        }
    }

    fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    fn is_active(&self) -> bool {
        !matches!(self, Self::Idle | Self::Done)
    }
}

/// Customer agent
///
/// Customers go through a simple lifecycle:
/// 1. Idle → decide to order based on probability
/// 2. Ordering → create demand for food
/// 3. WaitingForFood → passive state
/// 4. Eating → wait for eating duration
/// 5. Done → ready for removal
#[derive(Debug, Clone)]
pub struct Customer {
    pub id: AgentId,
    pub status: CustomerStatus,
    pub location: Point<f64>,
    pub order_probability: f64,
    pub eating_duration: Duration,
}

impl Customer {
    /// Create a new customer
    pub fn new(location: Point<f64>) -> Self {
        Self {
            id: AgentId::new(),
            status: CustomerStatus::Idle,
            location,
            order_probability: 0.1, // 10% chance to order each tick
            eating_duration: Duration::minutes(20),
        }
    }

    /// Create a customer with custom parameters
    pub fn with_params(
        location: Point<f64>,
        order_probability: f64,
        eating_duration: Duration,
    ) -> Self {
        Self {
            id: AgentId::new(),
            status: CustomerStatus::Idle,
            location,
            order_probability,
            eating_duration,
        }
    }

    /// Check if customer is done and can be removed
    pub fn is_done(&self) -> bool {
        matches!(self.status, CustomerStatus::Done)
    }

    /// Start eating (called externally when order is ready)
    pub fn start_eating(&mut self, current_time: DateTime<Utc>) {
        self.status = CustomerStatus::Eating {
            finish_time: current_time + self.eating_duration,
        };
    }
}

impl Agent for Customer {
    fn id(&self) -> AgentId {
        self.id
    }

    fn agent_type(&self) -> &str {
        "customer"
    }

    fn status(&self) -> &dyn AgentStatus {
        &self.status
    }

    fn position(&self) -> Option<Point<f64>> {
        Some(self.location)
    }

    fn tick(&mut self, ctx: &SimulationContext) -> sim_core::Result<AgentTickResult> {
        let mut result = AgentTickResult::default();

        match &self.status {
            CustomerStatus::Idle => {
                // Decide whether to order
                let mut rng = rand::rng();
                if rng.random::<f64>() < self.order_probability {
                    self.status = CustomerStatus::Ordering;
                }
            }

            CustomerStatus::Ordering => {
                // Create demand for food
                let order_id = format!("order-{}", self.id);

                let mut metadata = HashMap::new();
                metadata.insert("order_id".to_string(), Value::String(order_id.clone()));
                metadata.insert(
                    "customer_id".to_string(),
                    Value::String(self.id.to_string()),
                );
                metadata.insert(
                    "items".to_string(),
                    Value::Array(vec![Value::String("burger".to_string())]),
                );

                result.demands.push(Demand {
                    demand_type: "food_order".to_string(),
                    location: Some(self.location),
                    metadata,
                });

                self.status = CustomerStatus::WaitingForFood { order_id };
            }

            CustomerStatus::WaitingForFood { .. } => {
                // Passive state - waiting for order to be ready
                // External system will call start_eating() when ready
            }

            CustomerStatus::Eating { finish_time } => {
                // Check if finished eating
                if ctx.current_time() >= *finish_time {
                    self.status = CustomerStatus::Done;

                    // Add state change
                    result.state_changes.insert(
                        "finished_eating".to_string(),
                        Value::Bool(true),
                    );
                }
            }

            CustomerStatus::Done => {
                // Terminal state
            }
        }

        Ok(result)
    }

    fn state_data(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        match &self.status {
            CustomerStatus::WaitingForFood { order_id } => {
                map.insert(
                    "order_id".to_string(),
                    serde_json::Value::String(order_id.clone()),
                );
            }
            CustomerStatus::Eating { finish_time } => {
                map.insert(
                    "finish_time".to_string(),
                    serde_json::Value::String(finish_time.to_rfc3339()),
                );
            }
            _ => {}
        }

        serde_json::Value::Object(map)
    }

    fn from_arrow_row(row: &sim_core::population::ArrowAgentRow) -> sim_core::Result<Self> {
        use uuid::Uuid;

        // Parse UUID from binary ID
        let id = AgentId::from_uuid(
            Uuid::from_slice(&row.id)
                .map_err(|e| sim_core::Error::Agent(format!("Invalid UUID: {}", e)))?,
        );

        // Deserialize status from string
        let status = match row.status.as_str() {
            "idle" => CustomerStatus::Idle,
            "ordering" => CustomerStatus::Ordering,
            "waiting_for_food" => {
                // Extract order_id from metadata JSON
                let metadata: serde_json::Value = if let Some(ref m) = row.metadata {
                    serde_json::from_str(m).unwrap_or_default()
                } else {
                    serde_json::Value::Null
                };
                let order_id = metadata
                    .get("order_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                CustomerStatus::WaitingForFood { order_id }
            }
            "eating" => {
                // Extract finish_time from metadata
                let metadata: serde_json::Value = if let Some(ref m) = row.metadata {
                    serde_json::from_str(m).unwrap_or_default()
                } else {
                    serde_json::Value::Null
                };
                let finish_time = metadata
                    .get("finish_time")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(chrono::Utc::now);
                CustomerStatus::Eating { finish_time }
            }
            "done" => CustomerStatus::Done,
            _ => CustomerStatus::Idle,
        };

        // Parse location from position_x/position_y
        let location = if let (Some(x), Some(y)) = (row.position_x, row.position_y) {
            geo::Point::new(x, y)
        } else {
            geo::Point::new(0.0, 0.0) // Default location
        };

        Ok(Customer {
            id,
            status,
            location,
            order_probability: 0.1,
            eating_duration: chrono::Duration::minutes(20),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::SimulationConfig;

    #[test]
    fn test_customer_creation() {
        let customer = Customer::new(Point::new(-122.4, 37.8));

        assert!(matches!(customer.status, CustomerStatus::Idle));
        assert_eq!(customer.agent_type(), "customer");
    }

    #[test]
    fn test_customer_status() {
        let status = CustomerStatus::Idle;
        assert_eq!(status.as_str(), "idle");
        assert!(status.is_idle());

        let status = CustomerStatus::Ordering;
        assert_eq!(status.as_str(), "ordering");
        assert!(status.is_active());

        let status = CustomerStatus::Done;
        assert_eq!(status.as_str(), "done");
        assert!(!status.is_active());
    }

    #[test]
    fn test_customer_eating_flow() {
        let mut customer = Customer::new(Point::new(-122.4, 37.8));
        let ctx = SimulationContext::from_now(SimulationConfig::default());

        customer.status = CustomerStatus::Eating {
            finish_time: ctx.current_time() - Duration::seconds(1),
        };

        let result = customer.tick(&ctx).unwrap();

        assert!(matches!(customer.status, CustomerStatus::Done));
        assert!(customer.is_done());
        assert_eq!(result.state_changes.get("finished_eating"), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_start_eating() {
        let mut customer = Customer::new(Point::new(-122.4, 37.8));
        let now = Utc::now();

        customer.start_eating(now);

        match customer.status {
            CustomerStatus::Eating { finish_time } => {
                assert_eq!(finish_time, now + customer.eating_duration);
            }
            _ => panic!("Expected Eating status"),
        }
    }

    #[test]
    fn test_customer_arrow_round_trip() {
        use sim_core::population::ArrowAgentRow;

        // Test all CustomerStatus variants
        let test_cases = vec![
            CustomerStatus::Idle,
            CustomerStatus::Ordering,
            CustomerStatus::WaitingForFood {
                order_id: "ORDER123".to_string(),
            },
            CustomerStatus::Eating {
                finish_time: chrono::Utc::now(),
            },
            CustomerStatus::Done,
        ];

        for status in test_cases {
            let customer = Customer {
                id: AgentId::new(),
                status,
                location: geo::Point::new(1.0, 2.0),
                order_probability: 0.1,
                eating_duration: chrono::Duration::minutes(20),
            };

            // Convert to Arrow row
            let row: ArrowAgentRow = customer.to_arrow_row();

            // Convert back
            let restored = Customer::from_arrow_row(&row).unwrap();

            // Verify key fields match
            assert_eq!(customer.id, restored.id);
            assert_eq!(
                format!("{:?}", customer.status),
                format!("{:?}", restored.status)
            );
        }
    }
}
