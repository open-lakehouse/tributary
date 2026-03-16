//! Performance regression tests for batch operations
//!
//! These benchmarks ensure that batch operations maintain their performance
//! advantages over sequential operations. Run with:
//!
//! ```bash
//! cargo bench --bench batch_operations
//! ```

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use geo_types::Point;
use serde_json::Value;
use sim_core::{
    Result, SimulationConfig, SimulationContext,
    population::{Agent, AgentId, AgentStatus, AgentTickResult, ArrowAgentRow, PopulationHandler},
};
use uuid::Uuid;

// Simple test agent for benchmarking
#[derive(Debug, Clone)]
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

    fn is_idle(&self) -> bool {
        matches!(self, TestStatus::Idle)
    }
}

#[derive(Debug, Clone)]
struct TestAgent {
    id: AgentId,
    status: TestStatus,
    position: Point<f64>,
}

impl Agent for TestAgent {
    fn id(&self) -> AgentId {
        self.id
    }

    fn agent_type(&self) -> &str {
        "test_agent"
    }

    fn status(&self) -> &dyn AgentStatus {
        &self.status
    }

    fn position(&self) -> Option<Point<f64>> {
        Some(self.position)
    }

    fn tick(&mut self, _ctx: &SimulationContext) -> Result<AgentTickResult> {
        Ok(AgentTickResult::default())
    }

    fn state_data(&self) -> Value {
        serde_json::json!({})
    }

    fn from_arrow_row(row: &ArrowAgentRow) -> Result<Self> {
        let id = AgentId::from_uuid(
            Uuid::from_slice(&row.id)
                .map_err(|e| sim_core::Error::Agent(format!("Invalid UUID: {}", e)))?,
        );

        let status = match row.status.as_str() {
            "active" => TestStatus::Active,
            _ => TestStatus::Idle,
        };

        let position = if let (Some(x), Some(y)) = (row.position_x, row.position_y) {
            Point::new(x, y)
        } else {
            Point::new(0.0, 0.0)
        };

        Ok(TestAgent {
            id,
            status,
            position,
        })
    }
}

fn create_test_agent() -> TestAgent {
    TestAgent {
        id: AgentId::new(),
        status: TestStatus::Idle,
        position: Point::new(0.0, 0.0),
    }
}

fn create_test_agents(count: usize) -> Vec<TestAgent> {
    (0..count).map(|_| create_test_agent()).collect()
}

fn bench_batch_add_vs_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_addition");

    for size in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("sequential_add", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut handler = PopulationHandler::new();
                    for _ in 0..size {
                        handler.add_agent(black_box(create_test_agent())).unwrap();
                    }
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("batch_add", size), size, |b, &size| {
            b.iter(|| {
                let mut handler = PopulationHandler::new();
                let agents = create_test_agents(size);
                handler.add_agents(black_box(agents)).unwrap();
            });
        });
    }

    group.finish();
}

fn bench_batch_update_vs_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_update");

    for size in [10, 50, 100, 500].iter() {
        // Sequential update
        group.bench_with_input(
            BenchmarkId::new("sequential_update", size),
            size,
            |b, &size| {
                let mut handler = PopulationHandler::new();
                let agents = create_test_agents(size);
                handler.add_agents(agents.clone()).unwrap();

                b.iter(|| {
                    let mut modified_agents = agents.clone();
                    for agent in &mut modified_agents {
                        handler.update_agent(black_box(agent.clone())).unwrap();
                    }
                });
            },
        );

        // Batch update
        group.bench_with_input(BenchmarkId::new("batch_update", size), size, |b, &size| {
            let mut handler = PopulationHandler::new();
            let agents = create_test_agents(size);
            handler.add_agents(agents.clone()).unwrap();

            b.iter(|| {
                let modified_agents = agents.clone();
                handler.update_agents(black_box(modified_agents)).unwrap();
            });
        });
    }

    group.finish();
}

fn bench_tick_agents(c: &mut Criterion) {
    let mut group = c.benchmark_group("tick_agents");

    let ctx = SimulationContext::from_now(SimulationConfig::default());

    for size in [10, 50, 100, 500].iter() {
        group.bench_with_input(BenchmarkId::new("tick_all", size), size, |b, &size| {
            let mut handler = PopulationHandler::new();
            let agents = create_test_agents(size);
            handler.add_agents(agents).unwrap();

            b.iter(|| {
                handler.tick_agents(black_box(&ctx)).unwrap();
            });
        });
    }

    group.finish();
}

fn bench_get_agent(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_lookup");

    for size in [100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("get_agent", size), size, |b, &size| {
            let mut handler = PopulationHandler::new();
            let agents = create_test_agents(size);
            let first_id = agents[0].id();
            handler.add_agents(agents).unwrap();

            b.iter(|| {
                handler.get_agent(black_box(&first_id)).unwrap();
            });
        });
    }

    group.finish();
}

fn bench_agent_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_count");

    for size in [100, 500, 1000, 5000].iter() {
        group.bench_with_input(BenchmarkId::new("count", size), size, |b, &size| {
            let mut handler = PopulationHandler::new();
            let agents = create_test_agents(size);
            handler.add_agents(agents).unwrap();

            b.iter(|| {
                black_box(handler.agent_count());
            });
        });
    }

    group.finish();
}

fn bench_remove_agent(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_removal");

    for size in [100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("remove_agent", size), size, |b, &size| {
            b.iter_batched(
                || {
                    let mut handler = PopulationHandler::new();
                    let agents = create_test_agents(size);
                    let first_id = agents[0].id();
                    handler.add_agents(agents).unwrap();
                    (handler, first_id)
                },
                |(mut handler, id)| {
                    handler.remove_agent(black_box(&id)).unwrap();
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_batch_add_vs_sequential,
    bench_batch_update_vs_sequential,
    bench_tick_agents,
    bench_get_agent,
    bench_agent_count,
    bench_remove_agent
);
criterion_main!(benches);
