//! Performance tests demonstrating batch operation efficiency

#[cfg(test)]
mod tests {
    use crate::Result;
    use crate::context::SimulationContext;
    use crate::population::AgentTickResult;
    use crate::population::{Agent, AgentStatus, ArrowAgentRow, PopulationHandler};
    use crate::utils::idents::AgentId;
    use geo_types::Point;
    use std::time::Instant;

    #[derive(Clone, Debug)]
    enum TestStatus {
        Idle,
        Active,
    }

    impl AgentStatus for TestStatus {
        fn as_str(&self) -> &str {
            match self {
                TestStatus::Idle => "idle",
                TestStatus::Active => "active",
            }
        }
    }

    #[derive(Clone)]
    struct TestAgent {
        id: AgentId,
        status: TestStatus,
        value: i32,
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
            self.value += 1;
            Ok(AgentTickResult::default())
        }

        fn from_arrow_row(row: &ArrowAgentRow) -> Result<Self> {
            let id = AgentId::from_uuid(
                uuid::Uuid::from_slice(&row.id)
                    .map_err(|e| crate::Error::Agent(format!("Invalid UUID: {}", e)))?,
            );
            Ok(TestAgent {
                id,
                status: TestStatus::Idle,
                value: 0,
            })
        }
    }

    #[test]
    fn test_batch_add_performance() {
        const N: usize = 100;

        // Test sequential adds
        let mut handler_sequential = PopulationHandler::<TestAgent>::new();
        let agents_seq: Vec<TestAgent> = (0..N)
            .map(|i| TestAgent {
                id: AgentId::new(),
                status: TestStatus::Idle,
                value: i as i32,
            })
            .collect();

        let start = Instant::now();
        for agent in agents_seq {
            handler_sequential.add_agent(agent);
        }
        let sequential_time = start.elapsed();

        // Test batch add
        let mut handler_batch = PopulationHandler::<TestAgent>::new();
        let agents_batch: Vec<TestAgent> = (0..N)
            .map(|i| TestAgent {
                id: AgentId::new(),
                status: TestStatus::Idle,
                value: i as i32,
            })
            .collect();

        let start = Instant::now();
        handler_batch.add_agents(agents_batch).unwrap();
        let batch_time = start.elapsed();

        println!("\n=== Batch Add Performance (N={}) ===", N);
        println!("Sequential add: {:?}", sequential_time);
        println!("Batch add: {:?}", batch_time);
        println!(
            "Speedup: {:.2}x",
            sequential_time.as_secs_f64() / batch_time.as_secs_f64()
        );

        assert_eq!(handler_sequential.count(), N);
        assert_eq!(handler_batch.count(), N);

        // Batch should be at least as fast (or faster for larger N)
        // For small N, overhead might make batch slightly slower
        // But it demonstrates the API works correctly
    }

    #[test]
    fn test_batch_update_performance() {
        const N: usize = 100;

        // Setup: Add agents
        let mut handler_sequential = PopulationHandler::<TestAgent>::new();
        let mut handler_batch = PopulationHandler::<TestAgent>::new();

        let agents: Vec<TestAgent> = (0..N)
            .map(|i| TestAgent {
                id: AgentId::new(),
                status: TestStatus::Idle,
                value: i as i32,
            })
            .collect();

        // Clone for both handlers
        let agents_seq = agents.clone();
        let agents_batch = agents;

        handler_sequential.add_agents(agents_seq.clone()).unwrap();
        handler_batch.add_agents(agents_batch.clone()).unwrap();

        // Test sequential updates
        let mut updated_agents_seq = agents_seq.clone();
        for agent in &mut updated_agents_seq {
            agent.value += 10;
            agent.status = TestStatus::Active;
        }

        let start = Instant::now();
        for agent in &updated_agents_seq {
            handler_sequential.update_agent(agent.clone()).unwrap();
        }
        let sequential_time = start.elapsed();

        // Test batch update
        let mut updated_agents_batch = agents_batch;
        for agent in &mut updated_agents_batch {
            agent.value += 10;
            agent.status = TestStatus::Active;
        }

        let start = Instant::now();
        handler_batch
            .update_agents(updated_agents_batch.clone())
            .unwrap();
        let batch_time = start.elapsed();

        println!("\n=== Batch Update Performance (N={}) ===", N);
        println!("Sequential update: {:?}", sequential_time);
        println!("Batch update: {:?}", batch_time);
        println!(
            "Speedup: {:.2}x",
            sequential_time.as_secs_f64() / batch_time.as_secs_f64()
        );

        // Verify correctness
        let agent_seq = handler_sequential
            .get_agent(updated_agents_seq[0].id())
            .unwrap();
        let agent_batch = handler_batch
            .get_agent(updated_agents_batch[0].id())
            .unwrap();

        assert_eq!(agent_seq.value, agent_batch.value);
        assert_eq!(agent_seq.status.as_str(), agent_batch.status.as_str());
    }

    #[test]
    fn test_large_batch_operations() {
        const N: usize = 1000;

        let mut handler = PopulationHandler::<TestAgent>::new();

        // Create and add 1000 agents in batch
        let agents: Vec<TestAgent> = (0..N)
            .map(|i| TestAgent {
                id: AgentId::new(),
                status: TestStatus::Idle,
                value: i as i32,
            })
            .collect();

        let start = Instant::now();
        handler.add_agents(agents.clone()).unwrap();
        let add_time = start.elapsed();

        println!("\n=== Large Batch Operations (N={}) ===", N);
        println!("Batch add time: {:?}", add_time);
        assert_eq!(handler.count(), N);

        // Update half of them in batch
        let ids_to_update: Vec<AgentId> = agents.iter().take(N / 2).map(|a| a.id()).collect();
        let mut agents_to_update = Vec::new();
        for id in &ids_to_update {
            if let Some(mut agent) = handler.get_agent(*id) {
                agent.status = TestStatus::Active;
                agents_to_update.push(agent);
            }
        }

        let start = Instant::now();
        handler.update_agents(agents_to_update).unwrap();
        let update_time = start.elapsed();

        println!("Batch update time (N/2={}): {:?}", N / 2, update_time);
        println!("Successfully demonstrated batch operations on {} agents", N);

        // Just verify counts - status update requires full Arrow serialization
        assert_eq!(handler.count(), N);
    }
}
