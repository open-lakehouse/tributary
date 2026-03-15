//! Population handler implementation

use super::traits::{Agent, AgentTickResult};
use crate::context::SimulationContext;
use crate::utils::idents::AgentId;
use crate::Result;
use std::collections::HashMap;
use std::marker::PhantomData;

/// Manages the population of agents in the simulation
///
/// This is a generic handler that works with any type implementing the `Agent` trait.
/// It maintains agent state and orchestrates agent behavior through tick() calls.
///
/// # Type Parameters
///
/// * `A` - The agent type (must implement `Agent`)
///
/// # Example
///
/// ```rust,ignore
/// use sim_core::{PopulationHandler, SimulationContext};
///
/// // Create handler for your agent type
/// let mut population = PopulationHandler::<MyAgent>::new();
///
/// // Add agents
/// population.add_agent(agent1);
/// population.add_agent(agent2);
///
/// // Tick all agents
/// let ctx = SimulationContext::from_now(config);
/// let results = population.tick_agents(&ctx)?;
/// ```
pub struct PopulationHandler<A: Agent> {
    /// Map of agent ID to agent
    agents: HashMap<AgentId, A>,
    /// Phantom data to hold the agent type
    _phantom: PhantomData<A>,
}

impl<A: Agent> PopulationHandler<A> {
    /// Create a new population handler
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            _phantom: PhantomData,
        }
    }

    /// Add an agent to the population
    pub fn add_agent(&mut self, agent: A) {
        let id = agent.id();
        self.agents.insert(id, agent);
    }

    /// Remove an agent from the population
    pub fn remove_agent(&mut self, id: AgentId) -> Option<A> {
        self.agents.remove(&id)
    }

    /// Get an agent by ID
    pub fn get_agent(&self, id: AgentId) -> Option<&A> {
        self.agents.get(&id)
    }

    /// Get a mutable reference to an agent
    pub fn get_agent_mut(&mut self, id: AgentId) -> Option<&mut A> {
        self.agents.get_mut(&id)
    }

    /// Get the number of agents
    pub fn count(&self) -> usize {
        self.agents.len()
    }

    /// Tick all agents and collect results
    ///
    /// This calls the `tick()` method on each agent and aggregates the results.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Simulation context
    ///
    /// # Returns
    ///
    /// Aggregated tick results from all agents
    pub fn tick_agents(&mut self, ctx: &SimulationContext) -> Result<Vec<AgentTickResult>> {
        let mut all_results = Vec::new();

        for agent in self.agents.values_mut() {
            let result = agent.tick(ctx)?;
            all_results.push(result);
        }

        Ok(all_results)
    }

    /// Get all agents of a specific type
    pub fn get_agents_by_type(&self, agent_type: &str) -> Vec<&A> {
        self.agents
            .values()
            .filter(|a| a.agent_type() == agent_type)
            .collect()
    }

    /// Get iterator over all agents
    pub fn iter(&self) -> impl Iterator<Item = &A> {
        self.agents.values()
    }

    /// Get mutable iterator over all agents
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut A> {
        self.agents.values_mut()
    }
}

impl<A: Agent> Default for PopulationHandler<A> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::population::traits::AgentStatus;
    use geo_types::Point;

    #[derive(Debug, Clone)]
    enum TestStatus {
        Idle,
    }

    impl AgentStatus for TestStatus {
        fn as_str(&self) -> &str {
            "idle"
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
    fn test_population_handler() {
        let mut handler = PopulationHandler::new();

        let agent1 = TestAgent {
            id: AgentId::new(),
            status: TestStatus::Idle,
            tick_count: 0,
        };
        let agent2 = TestAgent {
            id: AgentId::new(),
            status: TestStatus::Idle,
            tick_count: 0,
        };

        let id1 = agent1.id();
        let id2 = agent2.id();

        handler.add_agent(agent1);
        handler.add_agent(agent2);

        assert_eq!(handler.count(), 2);
        assert!(handler.get_agent(id1).is_some());
        assert!(handler.get_agent(id2).is_some());
    }

    #[test]
    fn test_tick_agents() {
        let mut handler = PopulationHandler::new();

        let agent = TestAgent {
            id: AgentId::new(),
            status: TestStatus::Idle,
            tick_count: 0,
        };
        let id = agent.id();

        handler.add_agent(agent);

        let config = crate::context::SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        handler.tick_agents(&ctx).unwrap();

        // Agent should have been ticked
        assert_eq!(handler.get_agent(id).unwrap().tick_count, 1);
    }

    #[test]
    fn test_remove_agent() {
        let mut handler = PopulationHandler::new();

        let agent = TestAgent {
            id: AgentId::new(),
            status: TestStatus::Idle,
            tick_count: 0,
        };
        let id = agent.id();

        handler.add_agent(agent);
        assert_eq!(handler.count(), 1);

        let removed = handler.remove_agent(id);
        assert!(removed.is_some());
        assert_eq!(handler.count(), 0);
    }
}
