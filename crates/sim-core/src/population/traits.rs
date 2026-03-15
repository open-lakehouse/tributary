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

/// A row representation of an agent for Arrow storage
///
/// This struct represents a single agent as it appears in a RecordBatch row.
/// It contains all the fields needed to reconstruct an agent from columnar storage.
///
/// # Example
///
/// ```rust,ignore
/// use sim_core::population::ArrowAgentRow;
///
/// let row = ArrowAgentRow {
///     id: agent_id.as_bytes().to_vec(),
///     agent_type: "customer".to_string(),
///     status: "idle".to_string(),
///     position_x: Some(10.5),
///     position_y: Some(20.3),
///     metadata: Some("{\"balance\": 100}".to_string()),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct ArrowAgentRow {
    /// Agent ID as 16-byte binary representation
    pub id: Vec<u8>,
    /// Agent type identifier
    pub agent_type: String,
    /// Agent status as string
    pub status: String,
    /// X coordinate (None for non-spatial agents)
    pub position_x: Option<f64>,
    /// Y coordinate (None for non-spatial agents)
    pub position_y: Option<f64>,
    /// JSON-encoded metadata for domain-specific fields
    pub metadata: Option<String>,
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
pub trait Agent: Clone + Send + Sync + 'static {
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

    /// Convert agent to Arrow row representation
    ///
    /// This method serializes the agent into a row format suitable for storage in
    /// an Apache Arrow RecordBatch. The default implementation uses the standard
    /// agent methods and JSON serialization for metadata.
    ///
    /// Override this method if you need custom serialization logic.
    ///
    /// # Returns
    ///
    /// An `ArrowAgentRow` containing all the agent's data
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn to_arrow_row(&self) -> ArrowAgentRow {
    ///     ArrowAgentRow {
    ///         id: self.id().as_bytes().to_vec(),
    ///         agent_type: self.agent_type().to_string(),
    ///         status: self.status().as_str().to_string(),
    ///         position_x: self.position().map(|p| p.x()),
    ///         position_y: self.position().map(|p| p.y()),
    ///         metadata: serde_json::to_string(&self.state_data()).ok(),
    ///     }
    /// }
    /// ```
    fn to_arrow_row(&self) -> ArrowAgentRow {
        ArrowAgentRow {
            id: self.id().as_bytes().to_vec(),
            agent_type: self.agent_type().to_string(),
            status: self.status().as_str().to_string(),
            position_x: self.position().map(|p| p.x()),
            position_y: self.position().map(|p| p.y()),
            metadata: serde_json::to_string(&self.state_data()).ok(),
        }
    }

    /// Reconstruct agent from Arrow row representation
    ///
    /// This method deserializes an agent from its row representation in a RecordBatch.
    /// You must implement this method to support Arrow storage.
    ///
    /// # Arguments
    ///
    /// * `row` - The Arrow row containing agent data
    ///
    /// # Returns
    ///
    /// Result containing the reconstructed agent or an error
    ///
    /// # Errors
    ///
    /// Returns an error if the row data cannot be deserialized into the agent type.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn from_arrow_row(row: &ArrowAgentRow) -> Result<Self> {
    ///     let id = AgentId::from_uuid(
    ///         Uuid::from_slice(&row.id)
    ///             .map_err(|e| Error::Agent(format!("Invalid UUID: {}", e)))?
    ///     );
    ///
    ///     let status = match row.status.as_str() {
    ///         "idle" => CustomerStatus::Idle,
    ///         "ordering" => CustomerStatus::Ordering,
    ///         _ => return Err(Error::Agent("Unknown status".into())),
    ///     };
    ///
    ///     Ok(Customer { id, status, ... })
    /// }
    /// ```
    fn from_arrow_row(row: &ArrowAgentRow) -> Result<Self>;
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

    #[derive(Clone)]
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

        fn from_arrow_row(row: &ArrowAgentRow) -> Result<Self> {
            use crate::Error;
            let id = AgentId::from_uuid(
                uuid::Uuid::from_slice(&row.id)
                    .map_err(|e| Error::Agent(format!("Invalid UUID: {}", e)))?,
            );

            Ok(TestAgent {
                id,
                status: TestStatus::Idle,
                tick_count: 0,
            })
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
