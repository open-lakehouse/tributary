# sim-core Troubleshooting

## Common Issues and Solutions

### Issue 1: Agent Tick Never Called

**Symptom**: Agents added but `tick()` method never executes.

**Cause**: PopulationHandler not set on SimulationRunner.

**Solution**:
```rust
let mut runner = SimulationRunner::new(config);
let mut population = PopulationHandler::new();
population.add_agent(my_agent)?;

// Must set population before running!
runner.set_population_handler(population);

runner.run_for_steps(10)?;
```

**Verification**:
```rust
assert!(runner.population().is_some());
println!("Agent count: {}", runner.population().unwrap().count());
```

### Issue 2: Tasks Not Getting Assigned

**Symptom**: Tasks added to allocator but no assignments returned.

**Common Causes**:

**A. No Resources Added**
```rust
// Problem
let mut allocator = ResourceAllocator::new();
allocator.add_task(task)?;
let assignments = allocator.assign_tasks(&strategy, current_time)?;
// assignments is empty!

// Solution: Add resources first
allocator.add_resource(resource1)?;
allocator.add_resource(resource2)?;
allocator.add_task(task)?;
let assignments = allocator.assign_tasks(&strategy, current_time)?;
```

**B. Resource Type Mismatch**
```rust
// Problem: Task requires "kitchen" but resources are "server"
impl Task for MyTask {
    fn required_resource_type(&self) -> &str {
        "kitchen"  // No matching resources!
    }
}

impl Resource for MyResource {
    fn resource_type(&self) -> &str {
        "server"  // Different type
    }
}

// Solution: Match resource types
impl Resource for MyResource {
    fn resource_type(&self) -> &str {
        "kitchen"  // Now matches task requirement
    }
}
```

**C. All Resources at Capacity**
```rust
// Check resource capacity
for resource in allocator.resources() {
    println!("Resource {} load: {}/{}",
        resource.id(),
        resource.current_load(),
        resource.capacity()
    );
}

// Solution: Wait for tasks to complete or add more resources
allocator.complete_task(task_id)?;  // Frees capacity
// Or
allocator.add_resource(additional_resource)?;
```

### Issue 3: Time Not Advancing

**Symptom**: `ctx.current_time()` returns same value every tick.

**Cause**: Using manual tick instead of runner methods, or time_step is zero.

**Solution**:
```rust
// Option 1: Use runner methods (recommended)
runner.run_for_steps(100)?;

// Option 2: Manual tick - context advances automatically
for _ in 0..100 {
    runner.tick()?;  // Context advances internally
}

// Check time step configuration
let config = SimulationConfig {
    time_step: Duration::seconds(30),  // Must be > 0
    ..Default::default()
};
```

### Issue 4: Agent IDs Collision

**Symptom**: Multiple agents with same ID, or deterministic IDs when randomness expected.

**Cause**: Using `new_deterministic()` with same seed.

**Solution**:
```rust
// For unique IDs: Use new() (UUID v7, time-ordered)
let id1 = AgentId::new();
let id2 = AgentId::new();
assert_ne!(id1, id2);

// For deterministic IDs: Vary the seed
let id1 = AgentId::new_deterministic("agent-001");
let id2 = AgentId::new_deterministic("agent-002");
assert_ne!(id1, id2);

// Don't do this:
for _ in 0..10 {
    let id = AgentId::new_deterministic("agent");  // Same ID every time!
}

// Do this instead:
for i in 0..10 {
    let id = AgentId::new_deterministic(&format!("agent-{}", i));
}
```

### Issue 5: Workflow Not Progressing

**Symptom**: Workflows stuck in same step.

**Common Causes**:

**A. Workflow Engine Not Advanced**
```rust
// Problem: Engine not called
let mut engine = WorkflowEngine::new();
engine.add_workflow(workflow)?;
// Simulation runs but workflows don't progress

// Solution: Call advance_workflows in hooks
impl SimulationHooks for MyHooks {
    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        self.workflow_engine.advance_workflows(ctx.time_step())?;
        Ok(())
    }
}
```

**B. Resources Not Available**
```rust
// Check which step workflow is on
let active = engine.active_workflows();
for workflow in active {
    println!("Workflow {} on step {} (needs {})",
        workflow.id(),
        workflow.current_step_index(),
        workflow.steps()[workflow.current_step_index()].required_resource_type
    );
}

// Ensure resources exist for current step
let step = workflow.steps()[current_step];
let has_resource = allocator.resources()
    .any(|r| r.resource_type() == step.required_resource_type);
assert!(has_resource, "No resource for step type: {}", step.required_resource_type);
```

### Issue 6: Memory Growing Unbounded

**Symptom**: Memory usage continuously increases during simulation.

**Common Causes**:

**A. Events Not Flushed**
```rust
// Problem: Events accumulate in memory
impl SimulationHooks for MyHooks {
    fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) {
        for result in results {
            self.dispatcher.emit(event);  // Never flushed!
        }
    }
}

// Solution: Periodic flushing
impl SimulationHooks for MyHooks {
    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        if ctx.step() % 1000 == 0 {
            let count = self.dispatcher.flush_to_parquet(
                format!("events_step_{}.parquet", ctx.step())
            )?;
            println!("Flushed {} events", count);
        }
        Ok(())
    }
}
```

**B. Agent State Not Cleaned Up**
```rust
// Problem: Agents accumulate unbounded data
struct MyAgent {
    id: AgentId,
    history: Vec<StateChange>,  // Grows forever!
}

impl Agent for MyAgent {
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        self.history.push(StateChange { ... });  // Memory leak
        Ok(Default::default())
    }
}

// Solution: Bounded history or periodic cleanup
struct MyAgent {
    id: AgentId,
    history: VecDeque<StateChange>,  // Use VecDeque for efficient pop_front
}

impl Agent for MyAgent {
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        self.history.push_back(StateChange { ... });

        // Keep only last 100 entries
        while self.history.len() > 100 {
            self.history.pop_front();
        }

        Ok(Default::default())
    }
}
```

**C. Completed Tasks Not Removed**
```rust
// Check pending task count
println!("Pending tasks: {}", allocator.pending_tasks().len());

// Ensure tasks are marked complete
allocator.complete_task(task_id)?;

// Or set up automatic cleanup in hooks
impl SimulationHooks for MyHooks {
    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        // Complete tasks that have finished
        let now = ctx.current_time();
        let completed: Vec<TaskId> = self.allocator
            .assigned_tasks()
            .filter(|t| t.finish_time <= now)
            .map(|t| t.task_id)
            .collect();

        for task_id in completed {
            self.allocator.complete_task(task_id)?;
        }

        Ok(())
    }
}
```

### Issue 7: Hooks Not Called

**Symptom**: Hook methods never execute.

**Cause**: Hooks not set on runner, or wrong lifecycle method overridden.

**Solution**:
```rust
struct MyHooks;

impl SimulationHooks for MyHooks {
    fn on_start(&mut self, ctx: &SimulationContext) -> Result<()> {
        println!("Starting!");  // Will this print?
        Ok(())
    }
}

// Must set hooks on runner
let mut runner = SimulationRunner::new(config);
runner.set_hooks(Box::new(MyHooks));  // Required!

runner.run_for_steps(10)?;
```

**Verification**:
```rust
// Check hooks are set
assert!(runner.has_hooks());

// Add debug prints to verify calls
impl SimulationHooks for MyHooks {
    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        eprintln!("Hook called at step {}", ctx.step());
        Ok(())
    }
}
```

### Issue 8: Arrow/DataFusion Errors

**Symptom**: Arrow schema errors or DataFusion panics.

**Common Causes**:

**A. Schema Mismatch in Snapshots**
```rust
// Problem: Schema changed between save and load
let data1 = SnapshotData { /* old schema */ };
manager.save_snapshot(data1)?;

// Code changes, schema changes
let data2 = manager.load_snapshot(path)?;  // Error!

// Solution: Version your schemas
pub struct SnapshotData {
    pub version: u32,  // Schema version
    pub step: usize,
    pub agents: Vec<u8>,  // Serialized data
}

impl SnapshotData {
    fn to_record_batch(&self) -> Result<RecordBatch> {
        match self.version {
            1 => self.to_record_batch_v1(),
            2 => self.to_record_batch_v2(),
            _ => Err(Error::Custom(format!("Unknown version: {}", self.version))),
        }
    }
}
```

**B. Invalid Arrow Schema**
```rust
// Check schema before writing
let batch = data.to_record_batch()?;
println!("Schema: {:?}", batch.schema());

// Ensure all columns have valid types
for field in batch.schema().fields() {
    println!("Field: {} ({})", field.name(), field.data_type());
}
```

### Issue 9: Performance Degradation

**Symptom**: Simulation slows down over time.

**Common Causes**:

**A. O(n²) Operations**
```rust
// Problem: Nested loops over large collections
for agent in agents {
    for resource in resources {  // O(n²)
        if can_assign(agent, resource) {
            assign(agent, resource);
        }
    }
}

// Solution: Use allocation strategies (O(n))
let assignments = allocator.assign_tasks(&LeastLoaded, current_time)?;
```

**B. Allocation Churn**
```rust
// Problem: Creating many temporary objects
impl Agent for MyAgent {
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        let mut result = AgentTickResult::default();
        for _ in 0..100 {
            result.demands.push(Demand { ... });  // Many allocations
        }
        Ok(result)
    }
}

// Solution: Pre-allocate or reuse
impl Agent for MyAgent {
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        let mut result = AgentTickResult::default();
        result.demands.reserve(100);  // Pre-allocate
        for _ in 0..100 {
            result.demands.push(Demand { ... });
        }
        Ok(result)
    }
}
```

**C. Inefficient Strategy**
```rust
// Benchmark your allocation strategy
use std::time::Instant;

let start = Instant::now();
let assignments = allocator.assign_tasks(&strategy, current_time)?;
let duration = start.elapsed();

println!("Assignment took {:?} for {} tasks", duration, assignments.len());

// Try different strategies
let strategies = vec![
    Box::new(RoundRobin) as Box<dyn AllocationStrategy<_, _>>,
    Box::new(LeastLoaded),
    Box::new(Random),
];

for strategy in strategies {
    let start = Instant::now();
    allocator.assign_tasks(&*strategy, current_time)?;
    println!("{:?}: {:?}", strategy, start.elapsed());
}
```

## Debugging Tips

### Enable Verbose Logging

```rust
let config = SimulationConfig {
    verbose: true,  // Enable framework logging
    ..Default::default()
};

let mut runner = SimulationRunner::new(config);
```

### Inspect Simulation State

```rust
// After any step
let ctx = runner.context();
println!("Step: {}", ctx.step());
println!("Time: {}", ctx.current_time());
println!("Elapsed: {:?}", ctx.elapsed());

let stats = runner.stats();
println!("Agent ticks: {}", stats.total_agent_ticks);
println!("Assignments: {}", stats.total_assignments);

if let Some(pop) = runner.population() {
    println!("Active agents: {}", pop.count());
}

if let Some(alloc) = runner.resources() {
    println!("Pending tasks: {}", alloc.pending_tasks().len());
}
```

### Add Debug Hooks

```rust
struct DebugHooks;

impl SimulationHooks for DebugHooks {
    fn on_agents_ticked(
        &mut self,
        ctx: &SimulationContext,
        results: &[AgentTickResult],
    ) -> Result<()> {
        eprintln!("[{}] {} agents ticked", ctx.step(), results.len());

        for result in results {
            eprintln!("  Agent {}: {} demands, {} journeys",
                result.agent_id,
                result.demands.len(),
                result.journeys.len()
            );
        }

        Ok(())
    }

    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        eprintln!("[{}] Tick complete at {}", ctx.step(), ctx.current_time());
        Ok(())
    }
}
```

### Profile with Criterion

```rust
// In benches/my_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_simulation(c: &mut Criterion) {
    c.bench_function("1000 steps", |b| {
        b.iter(|| {
            let mut runner = setup_simulation();
            runner.run_for_steps(black_box(1000)).unwrap();
        });
    });
}

criterion_group!(benches, benchmark_simulation);
criterion_main!(benches);
```

Run with:
```bash
cargo bench -p sim-core
```

### Memory Profiling

```bash
# Install heaptrack
# On macOS: brew install heaptrack
# On Linux: apt-get install heaptrack

# Run with profiling
heaptrack cargo run --release --example my_simulation

# Analyze results
heaptrack_gui heaptrack.my_simulation.XXXXX.gz
```

## FAQ

### Q: Can I add agents during simulation?

**A**: Yes, but you need mutable access to the population handler. Do this in hooks:

```rust
struct DynamicHooks {
    pending_agents: Vec<MyAgent>,
}

impl SimulationHooks for DynamicHooks {
    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        // Can't directly add to runner's population from here
        // Instead, collect pending agents and add them later
        if ctx.step() % 10 == 0 {
            self.pending_agents.push(MyAgent::new());
        }
        Ok(())
    }
}

// In main loop:
for step in 0..1000 {
    runner.tick()?;

    // Access hooks and population separately
    if let (Some(hooks), Some(population)) = (
        runner.hooks_mut(),
        runner.population_mut()
    ) {
        for agent in hooks.pending_agents.drain(..) {
            population.add_agent(agent)?;
        }
    }
}
```

### Q: How do I model agent-to-agent interactions?

**A**: Agents don't directly interact. Instead, use resources or custom hooks:

**Pattern 1: Via Resources**
```rust
// Agents emit demands, framework allocates
result.demands.push(Demand {
    demand_type: "message",
    target_agent: Some(recipient_id),
    ...
});

// In hooks: route messages
```

**Pattern 2: Via Custom State**
```rust
struct InteractionHooks {
    messages: HashMap<AgentId, Vec<Message>>,
}

impl SimulationHooks for InteractionHooks {
    fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) {
        for result in results {
            if let Some(msg) = extract_message(&result.state_change) {
                self.messages.entry(msg.recipient)
                    .or_default()
                    .push(msg);
            }
        }
    }
}
```

### Q: How do I model probabilistic behavior?

**A**: Use rand in agent tick:

```rust
use rand::Rng;

impl Agent for MyAgent {
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        let mut rng = rand::thread_rng();
        let mut result = AgentTickResult::default();

        if rng.gen::<f64>() < 0.3 {  // 30% probability
            result.demands.push(self.create_demand());
        }

        Ok(result)
    }
}
```

For reproducibility, seed the RNG:
```rust
use rand::SeedableRng;
use rand::rngs::StdRng;

struct MyAgent {
    rng: StdRng,
}

impl MyAgent {
    fn new(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
        }
    }
}
```

### Q: Can I run simulations in parallel?

**A**: Not currently supported within a single `SimulationRunner`. For parallelism:

**Option 1: Run Multiple Simulations**
```rust
use rayon::prelude::*;

let results: Vec<_> = (0..10)
    .into_par_iter()
    .map(|i| {
        let mut runner = setup_simulation(i);
        runner.run_for_steps(1000).unwrap();
        runner.stats().clone()
    })
    .collect();
```

**Option 2: Parallel Agent Ticking (Future Work)**
```rust
// Not yet implemented, but planned:
let config = SimulationConfig {
    parallel_agent_ticking: true,
    ..Default::default()
};
```

### Q: How do I test my agents?

**A**: Unit test with a mock context:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_agent_behavior() {
        let mut agent = MyAgent::new();

        let ctx = SimulationContext::new(
            Utc::now(),
            Duration::seconds(30),
            0,
            SimulationConfig::default(),
        );

        let result = agent.tick(&ctx).unwrap();

        assert_eq!(result.demands.len(), 1);
        assert_eq!(result.demands[0].demand_type, "expected_type");
    }
}
```

### Q: How do I integrate with real data sources?

**A**: Load data in setup, use in demand generators:

```rust
use std::fs::File;
use serde::Deserialize;

#[derive(Deserialize)]
struct CustomerData {
    arrival_rate: f64,
    peak_hours: Vec<u8>,
}

fn main() -> Result<()> {
    // Load from CSV, JSON, etc.
    let data: CustomerData = serde_json::from_reader(
        File::open("customer_data.json")?
    )?;

    // Use in simulation
    let generator = PoissonDemandGenerator::new(
        data.arrival_rate,
        "customer",
        "order",
    );

    // ...
}
```

## Known Limitations

### 1. No Real-Time Execution
Simulation is tick-based, not real-time. Cannot guarantee wall-clock timing.

### 2. Single-Threaded Agent Ticking
All agents tick sequentially in a single thread. For large populations (>100k), consider domain-specific optimizations.

### 3. No Built-In Networking
Agents cannot directly communicate. Must use resources or custom hooks for message passing.

### 4. No Spatial Queries in Core
Use `sim-geo` for spatial operations (nearest neighbor, radius search, etc.).

### 5. No Built-In Visualization
Framework focuses on simulation logic. For visualization, export events to Parquet and use external tools (Plotly, Grafana, etc.).

## Getting Help

### Check the Examples
```bash
cargo run -p sim-core --example snapshot_demo
```

### Read the Tests
```bash
cargo test -p sim-core -- --nocapture
```

### Enable Debug Logging
```rust
env_logger::init();  // If using env_logger
log::debug!("Custom debug message");
```

### Ask Questions
- GitHub Issues: [Link to repo]
- Documentation: `cargo doc -p sim-core --open`

### Report Bugs
Include:
1. Minimal reproducible example
2. Expected vs actual behavior
3. Version information (`cargo --version`)
4. Platform (OS, architecture)
