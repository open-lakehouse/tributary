// Custom Agent Implementation Template
//
// This template provides a well-commented boilerplate for creating new agent types.
// Replace the placeholder types and logic with your domain-specific implementation.

use sim_core::{Agent, AgentId, AgentStatus, AgentTickResult, SimulationContext, Result, Error};
use sim_core::population::ArrowAgentRow;
use geo_types::Point;
use serde_json::{Value, json};
use std::collections::HashMap;

// =============================================================================
// Step 1: Define Your Agent Status Enum
// =============================================================================

/// Agent status represents the current state in your agent's state machine.
/// Use an enum with variants for each possible state.
#[derive(Clone, Debug)]
pub enum MyAgentStatus {
    /// Initial state - agent is idle and waiting for events
    Idle,

    /// Agent is actively processing work
    Processing {
        /// Example: Track when processing started
        started_at: chrono::DateTime<chrono::Utc>,
    },

    /// Agent is waiting for external event
    Waiting {
        /// Example: What are we waiting for?
        waiting_for: String,
    },

    /// Terminal state - agent has completed its lifecycle
    Done,
}

// Implement AgentStatus trait to integrate with framework
impl AgentStatus for MyAgentStatus {
    fn as_str(&self) -> &str {
        match self {
            MyAgentStatus::Idle => "idle",
            MyAgentStatus::Processing { .. } => "processing",
            MyAgentStatus::Waiting { .. } => "waiting",
            MyAgentStatus::Done => "done",
        }
    }

    fn is_idle(&self) -> bool {
        matches!(self, MyAgentStatus::Idle)
    }

    fn is_active(&self) -> bool {
        matches!(self, MyAgentStatus::Processing { .. })
    }
}

// =============================================================================
// Step 2: Define Your Agent Struct
// =============================================================================

/// Your custom agent type.
/// Include all fields needed to represent agent state.
#[derive(Clone)]
pub struct MyAgent {
    /// Unique identifier (required)
    id: AgentId,

    /// Current status (required)
    status: MyAgentStatus,

    /// Position in space (optional - return None in position() if non-spatial)
    position: Option<Point<f64>>,

    // Add your domain-specific fields here:
    /// Example: Counter for tracking behavior
    tick_count: usize,

    /// Example: Some configuration value
    configuration: String,

    /// Example: History or accumulated state
    history: Vec<String>,
}

// =============================================================================
// Step 3: Implement the Agent Trait
// =============================================================================

impl Agent for MyAgent {
    // -------------------------------------------------------------------------
    // Required: Unique identifier
    // -------------------------------------------------------------------------
    fn id(&self) -> AgentId {
        self.id
    }

    // -------------------------------------------------------------------------
    // Required: Agent type for grouping/filtering
    // -------------------------------------------------------------------------
    fn agent_type(&self) -> &str {
        "my_agent"  // Change to your domain-specific type
    }

    // -------------------------------------------------------------------------
    // Required: Current status
    // -------------------------------------------------------------------------
    fn status(&self) -> &dyn AgentStatus {
        &self.status
    }

    // -------------------------------------------------------------------------
    // Required: Position (return None for non-spatial agents)
    // -------------------------------------------------------------------------
    fn position(&self) -> Option<Point<f64>> {
        self.position
    }

    // -------------------------------------------------------------------------
    // Required: Core behavior - called each simulation step
    // -------------------------------------------------------------------------
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        // Initialize result to collect changes
        let mut result = AgentTickResult::default();

        // Increment counter (example domain logic)
        self.tick_count += 1;

        // State machine: Update status based on current state and context
        match &self.status {
            MyAgentStatus::Idle => {
                // Example: Probabilistically transition to processing
                if rand::random::<f64>() < 0.1 {
                    self.status = MyAgentStatus::Processing {
                        started_at: ctx.current_time(),
                    };

                    // Example: Emit a demand when transitioning
                    result.demands.push(sim_core::population::Demand {
                        demand_type: "my_work_item".to_string(),
                        location: self.position,
                        metadata: HashMap::from([
                            ("agent_id".to_string(), json!(self.id.to_string())),
                        ]),
                    });
                }
            }

            MyAgentStatus::Processing { started_at } => {
                // Example: Check if processing duration elapsed
                let duration = ctx.current_time() - *started_at;
                if duration.num_seconds() > 300 {  // 5 minutes
                    self.status = MyAgentStatus::Waiting {
                        waiting_for: "external_input".to_string(),
                    };

                    // Record in history
                    self.history.push(format!("Completed processing at {}", ctx.current_time()));
                }
            }

            MyAgentStatus::Waiting { .. } => {
                // Example: Transition to done after some condition
                if self.tick_count > 100 {
                    self.status = MyAgentStatus::Done;
                }
            }

            MyAgentStatus::Done => {
                // Terminal state - no action
            }
        }

        // Example: Request a journey (for spatial agents)
        if let Some(current_pos) = self.position {
            // Generate random destination (example)
            if rand::random::<f64>() < 0.05 {
                let destination = Point::new(
                    current_pos.x() + (rand::random::<f64>() - 0.5) * 0.01,
                    current_pos.y() + (rand::random::<f64>() - 0.5) * 0.01,
                );

                result.new_journeys.push(sim_core::population::JourneyRequest {
                    id: None,
                    agent_id: self.id(),
                    origin: current_pos,
                    destination,
                    metadata: HashMap::new(),
                });
            }
        }

        // Example: Report state changes for persistence
        result.state_changes.insert(
            "tick_count".to_string(),
            json!(self.tick_count),
        );
        result.state_changes.insert(
            "configuration".to_string(),
            json!(self.configuration),
        );

        Ok(result)
    }

    // -------------------------------------------------------------------------
    // Optional: Custom state data for snapshots
    // -------------------------------------------------------------------------
    fn state_data(&self) -> Value {
        json!({
            "tick_count": self.tick_count,
            "configuration": self.configuration,
            "history": self.history,
        })
    }

    // -------------------------------------------------------------------------
    // Required: Serialize to Arrow storage
    // -------------------------------------------------------------------------
    fn to_arrow_row(&self) -> ArrowAgentRow {
        ArrowAgentRow {
            id: self.id.as_bytes().to_vec(),
            agent_type: self.agent_type().to_string(),
            status: self.status.as_str().to_string(),
            position_x: self.position.map(|p| p.x()),
            position_y: self.position.map(|p| p.y()),
            metadata: serde_json::to_string(&self.state_data()).ok(),
        }
    }

    // -------------------------------------------------------------------------
    // Required: Deserialize from Arrow storage
    // -------------------------------------------------------------------------
    fn from_arrow_row(row: &ArrowAgentRow) -> Result<Self> {
        // Parse ID
        let id = AgentId::from_uuid(
            uuid::Uuid::from_slice(&row.id)
                .map_err(|e| Error::Agent(format!("Invalid UUID: {}", e)))?
        );

        // Parse status
        let status = match row.status.as_str() {
            "idle" => MyAgentStatus::Idle,
            "processing" => MyAgentStatus::Processing {
                started_at: chrono::Utc::now(),  // Approximate
            },
            "waiting" => MyAgentStatus::Waiting {
                waiting_for: "unknown".to_string(),
            },
            "done" => MyAgentStatus::Done,
            _ => return Err(Error::Agent(format!("Unknown status: {}", row.status))),
        };

        // Parse position
        let position = match (row.position_x, row.position_y) {
            (Some(x), Some(y)) => Some(Point::new(x, y)),
            _ => None,
        };

        // Parse metadata
        let metadata = if let Some(meta_str) = &row.metadata {
            serde_json::from_str(meta_str)
                .unwrap_or_else(|_| json!({}))
        } else {
            json!({})
        };

        let tick_count = metadata.get("tick_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let configuration = metadata.get("configuration")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        let history = metadata.get("history")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        Ok(MyAgent {
            id,
            status,
            position,
            tick_count,
            configuration,
            history,
        })
    }
}

// =============================================================================
// Step 4: Constructor and Helper Methods
// =============================================================================

impl MyAgent {
    /// Create a new agent with default state
    pub fn new(id: AgentId) -> Self {
        Self {
            id,
            status: MyAgentStatus::Idle,
            position: None,
            tick_count: 0,
            configuration: "default".to_string(),
            history: Vec::new(),
        }
    }

    /// Create a spatial agent at a specific location
    pub fn at_location(id: AgentId, position: Point<f64>) -> Self {
        Self {
            id,
            status: MyAgentStatus::Idle,
            position: Some(position),
            tick_count: 0,
            configuration: "default".to_string(),
            history: Vec::new(),
        }
    }

    /// Example: Custom configuration
    pub fn with_config(mut self, config: String) -> Self {
        self.configuration = config;
        self
    }

    /// Example: Check if agent is complete
    pub fn is_complete(&self) -> bool {
        matches!(self.status, MyAgentStatus::Done)
    }
}

// =============================================================================
// Step 5: Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::SimulationConfig;

    #[test]
    fn test_agent_creation() {
        let agent = MyAgent::new(AgentId::new());
        assert_eq!(agent.agent_type(), "my_agent");
        assert!(agent.status().is_idle());
    }

    #[test]
    fn test_agent_tick() {
        let mut agent = MyAgent::new(AgentId::new());
        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let result = agent.tick(&ctx).unwrap();
        assert_eq!(agent.tick_count, 1);
        assert!(result.state_changes.contains_key("tick_count"));
    }

    #[test]
    fn test_arrow_serialization() {
        let agent = MyAgent::new(AgentId::new());

        // Serialize
        let row = agent.to_arrow_row();

        // Deserialize
        let restored = MyAgent::from_arrow_row(&row).unwrap();

        assert_eq!(restored.id(), agent.id());
        assert_eq!(restored.agent_type(), agent.agent_type());
    }
}
