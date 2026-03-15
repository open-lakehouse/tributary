//! Core traits for agents

use crate::context::SimulationContext;
use crate::utils::idents::{AgentId, JourneyId};
use crate::Result;
use geo_types::Point;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;

/// Status of an agent (domain-specific)
///
/// Implement this trait for your domain-specific agent status enums.
/// This allows the framework to work with any status type while maintaining type safety.
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Clone, Debug)]
/// pub enum CustomerStatus {
///     Idle,
///     Ordering,
///     WaitingForDelivery,
/// }
///
/// impl AgentStatus for CustomerStatus {
///     fn as_str(&self) -> &str {
///         match self {
///             CustomerStatus::Idle => "idle",
///             CustomerStatus::Ordering => "ordering",
///             CustomerStatus::WaitingForDelivery => "waiting",
///         }
///     }
///
///     fn is_idle(&self) -> bool {
///         matches!(self, CustomerStatus::Idle)
///     }
/// }
/// ```
pub trait AgentStatus: Send + Sync + fmt::Debug {
    /// Get the status as a string representation
    fn as_str(&self) -> &str;

    /// Check if this status represents an idle/inactive state
    fn is_idle(&self) -> bool {
        false
    }

    /// Check if this status represents an active/busy state
    fn is_active(&self) -> bool {
        !self.is_idle()
    }
}

/// Request to start a new journey
#[derive(Debug, Clone)]
pub struct JourneyRequest {
    /// Identifier for this journey (will be assigned if None)
    pub id: Option<JourneyId>,
    /// Agent making the journey
    pub agent_id: AgentId,
    /// Starting point
    pub origin: Point<f64>,
    /// Destination point
    pub destination: Point<f64>,
    /// Optional metadata about the journey
    pub metadata: HashMap<String, String>,
}

/// A demand request (e.g., create order, spawn task)
#[derive(Debug, Clone)]
pub struct Demand {
    /// Type of demand (domain-specific, e.g., "order", "shipment", "ticket")
    pub demand_type: String,
    /// Optional location associated with this demand
    pub location: Option<Point<f64>>,
    /// Domain-specific metadata
    pub metadata: HashMap<String, Value>,
}

/// Result of an agent tick
///
/// This structure contains all the changes an agent wants to make during a simulation step.
#[derive(Debug, Clone, Default)]
pub struct AgentTickResult {
    /// New journeys to start
    pub new_journeys: Vec<JourneyRequest>,
    /// New demands to create
    pub demands: Vec<Demand>,
    /// State changes (field name -> new value)
    pub state_changes: HashMap<String, Value>,
}

/// An agent that exists in the simulation
///
/// This is the core trait that all agents must implement. Agents are entities with behavior
/// that evolve over time through the `tick()` method.
///
/// # Example
///
/// ```rust,ignore
/// use sim_core::{Agent, AgentId, AgentStatus, AgentTickResult, SimulationContext};
/// use geo_types::Point;
///
/// struct Customer {
///     id: AgentId,
///     status: CustomerStatus,
///     position: Point<f64>,
/// }
///
/// impl Agent for Customer {
///     fn id(&self) -> AgentId {
///         self.id
///     }
///
///     fn agent_type(&self) -> &str {
///         "customer"
///     }
///
///     fn status(&self) -> &dyn AgentStatus {
///         &self.status
///     }
///
///     fn position(&self) -> Option<Point<f64>> {
///         Some(self.position)
///     }
///
///     fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
///         // Update agent state based on simulation time
///         // Return any journeys, demands, or state changes
///         Ok(AgentTickResult::default())
///     }
/// }
/// ```
pub trait Agent: Send + Sync {
    /// Unique identifier for this agent
    fn id(&self) -> AgentId;

    /// Type of agent (e.g., "customer", "courier", "driver", "worker")
    ///
    /// This is used for grouping and filtering agents.
    fn agent_type(&self) -> &str;

    /// Current status of the agent
    ///
    /// Returns a reference to a trait object, allowing domain-specific status types.
    fn status(&self) -> &dyn AgentStatus;

    /// Current position in the world (if applicable)
    ///
    /// Returns None for agents that don't have a physical location.
    fn position(&self) -> Option<Point<f64>>;

    /// Update agent state for this simulation step
    ///
    /// This method is called once per simulation step for each agent. It should:
    /// - Update internal state based on the current simulation time
    /// - Return any journeys the agent wants to start
    /// - Return any demands the agent wants to create
    /// - Return any state changes that should be persisted
    ///
    /// # Arguments
    ///
    /// * `ctx` - Simulation context with current time, configuration, etc.
    ///
    /// # Returns
    ///
    /// Result containing the tick result with journeys, demands, and state changes.
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult>;

    /// Optional: Get custom state data for persistence
    ///
    /// This can be used to serialize additional state that should be saved in snapshots.
    /// The default implementation returns an empty object.
    fn state_data(&self) -> Value {
        Value::Object(serde_json::Map::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    enum TestStatus {
        Idle,
        #[allow(dead_code)]
        Active,
    }

    impl AgentStatus for TestStatus {
        fn as_str(&self) -> &str {
            match self {
                TestStatus::Idle => "idle",
                TestStatus::Active => "active",
            }
        }

        fn is_idle(&self) -> bool {
            matches!(self, TestStatus::Idle)
        }
    }

    struct TestAgent {
        id: AgentId,
        status: TestStatus,
        tick_count: usize,
    }

    impl Agent for TestAgent {
        fn id(&self) -> AgentId {
            self.id
        }

        fn agent_type(&self) -> &str {
            "test"
        }

        fn status(&self) -> &dyn AgentStatus {
            &self.status
        }

        fn position(&self) -> Option<Point<f64>> {
            None
        }

        fn tick(&mut self, _ctx: &SimulationContext) -> Result<AgentTickResult> {
            self.tick_count += 1;
            Ok(AgentTickResult::default())
        }
    }

    #[test]
    fn test_agent_status() {
        let status = TestStatus::Idle;
        assert_eq!(status.as_str(), "idle");
        assert!(status.is_idle());
        assert!(!status.is_active());
    }

    #[test]
    fn test_agent_creation() {
        let agent = TestAgent {
            id: AgentId::new(),
            status: TestStatus::Idle,
            tick_count: 0,
        };

        assert_eq!(agent.agent_type(), "test");
        assert!(agent.status().is_idle());
        assert_eq!(agent.position(), None);
    }
}
