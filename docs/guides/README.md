# Framework User Guides

Comprehensive guides for building domain simulations with the agent-based simulation framework. All guides use `sim-logistics` as the primary reference implementation, which was built **without any framework modifications** (Phase 11 validation).

## Quick Start

New to the framework? Start here:

1. **[Building a Domain Simulation](./building-domain.md)** - Complete guide from concept to implementation
2. **[Custom Agents](./custom-agents.md)** - Implementing agents with state management
3. **[Testing Strategies](./testing-strategies.md)** - Comprehensive testing approach

## Complete Guide Index

### Core Implementation Guides

#### 1. [Building a Domain Simulation](./building-domain.md)

Complete guide for creating a new domain simulation from scratch.

**Topics Covered:**
- Domain model definition (agents, resources, workflows)
- Crate structure and module organization
- Step-by-step implementation process
- Integration with framework components
- Testing at each stage
- Complete checklist for domain completion

**Target Audience:** Developers creating a new domain simulation

**Prerequisites:** Basic Rust knowledge, understanding of agent-based modeling

**Estimated Time:** 2-4 hours for initial implementation

---

#### 2. [Custom Agents Guide](./custom-agents.md)

Advanced patterns for implementing the `Agent` trait.

**Topics Covered:**
- Agent trait overview and requirements
- State management patterns (enum-based, struct-based, composite)
- Lifecycle methods (initialization, tick, serialization)
- Position tracking semantics
- Testing strategies for agents
- Examples: Package (passive), Driver (active)

**Target Audience:** Developers implementing custom agent types

**Prerequisites:** [Building a Domain Simulation](./building-domain.md)

**Estimated Time:** 1-2 hours

---

#### 3. [Custom Resources Guide](./custom-resources.md)

Implementing capacity-constrained processors with the `Resource` trait.

**Topics Covered:**
- Resource trait overview
- Task processing patterns (FIFO, slot-based, priority, batch)
- State transitions and capacity management
- Testing strategies for resources
- Examples: SortingStation (FIFO), LoadingDock (slot-based)

**Target Audience:** Developers implementing resource processors

**Prerequisites:** [Building a Domain Simulation](./building-domain.md)

**Estimated Time:** 1-2 hours

---

#### 4. [Custom Workflows Guide](./custom-workflows.md)

Multi-step processes with the `Workflow` trait.

**Topics Covered:**
- Workflow trait overview
- Step-based processing
- Workflow state management
- Task factory pattern
- WorkflowEngine integration
- Testing workflow definitions and progression
- Example: PackageShipmentWorkflow (5-step pipeline)

**Target Audience:** Developers implementing multi-step processes

**Prerequisites:** [Building a Domain Simulation](./building-domain.md), [Custom Resources](./custom-resources.md)

**Estimated Time:** 1-2 hours

---

### Quality and Performance Guides

#### 5. [Testing Strategies Guide](./testing-strategies.md)

Comprehensive testing patterns and practices.

**Topics Covered:**
- Testing philosophy and test pyramid
- Test organization and naming
- Unit testing patterns (state transitions, capacity, timing, serialization)
- Integration testing
- Mocking and fixtures
- Coverage goals (> 85% overall, > 90% critical paths)
- Examples from the workspace test suite

**Target Audience:** All developers (essential reading)

**Prerequisites:** Basic testing knowledge

**Estimated Time:** 1 hour

---

#### 6. [Performance Tuning Guide](./performance-tuning.md)

Optimization strategies for high-performance simulations.

**Topics Covered:**
- Performance goals (10,000+ agents, 100+ ticks/sec)
- Optimization strategies (batching, avoiding clones, data structures)
- Batch operations with Arrow RecordBatch
- Memory management and layout
- Profiling techniques (flamegraph, heaptrack, criterion)
- Benchmark results and scaling analysis

**Target Audience:** Developers optimizing performance

**Prerequisites:** Working simulation implementation

**Estimated Time:** 2-3 hours for full optimization

---

## Reference Implementation

### sim-logistics

The **logistics domain simulation** serves as the primary reference for all guides:

**Location:** `/crates/sim-logistics`

**Characteristics:**
- Built with **ZERO framework modifications**
- Validates framework generality
- Comprehensive test suite
- Complete documentation
- Reusable implementation patterns

**Components:**

**Agents:**
- `Package` - Passive agent tracking delivery lifecycle (5 states)
- `Driver` - Active agent with autonomous behavior (5 states)

**Resources:**
- `SortingStation` - FIFO queue processing (12 packages/hour)
- `LoadingDock` - Slot-based vehicle loading (5 packages/hour per slot)

**Workflows:**
- `PackageShipmentWorkflow` - 5-step delivery pipeline (42 minutes total)

**Performance:**
- 10,000+ agents
- 686 ticks/second
- 125 MB memory usage
- 96% test coverage

**Study sim-logistics to see all patterns in action!**

## Learning Paths

### Path 1: Building Your First Simulation

```
1. Building a Domain Simulation (full guide)
   ↓
2. Custom Agents (focus on passive agents)
   ↓
3. Testing Strategies (unit tests)
   ↓
4. Create your domain with 1 agent type
```

**Time:** 4-6 hours

**Output:** Working simulation with tested agent

---

### Path 2: Complete Domain Implementation

```
1. Building a Domain Simulation
   ↓
2. Custom Agents (all patterns)
   ↓
3. Custom Resources (FIFO pattern)
   ↓
4. Custom Workflows (linear workflow)
   ↓
5. Testing Strategies (full suite)
   ↓
6. Implement complete domain
```

**Time:** 1-2 days

**Output:** Working domain simulation

---

### Path 3: Advanced Optimization

```
1. Review existing simulation
   ↓
2. Testing Strategies (benchmark setup)
   ↓
3. Performance Tuning (profiling)
   ↓
4. Performance Tuning (optimization)
   ↓
5. Optimize and validate improvements
```

**Time:** 4-8 hours

**Output:** Optimized high-performance simulation

---

## Common Tasks

### Adding a New Agent Type

1. Read [Custom Agents](./custom-agents.md) - State management patterns
2. Define state enum implementing `AgentStatus`
3. Create agent struct
4. Implement `Agent` trait
5. Write unit tests following [Testing Strategies](./testing-strategies.md)
6. Test serialization round-trip

**Files to create:**
- `src/agents/my_agent.rs` - Implementation
- Inline `#[cfg(test)]` module - Unit tests

---

### Adding a New Resource

1. Read [Custom Resources](./custom-resources.md) - Processing patterns
2. Choose pattern (FIFO, slot-based, priority, batch)
3. Create resource struct with queue/processing state
4. Implement `Resource` trait
5. Implement `process_tick()` method
6. Write unit tests

**Files to create:**
- `src/resources/my_resource.rs` - Implementation
- Inline `#[cfg(test)]` module - Unit tests

---

### Adding a Multi-Step Process

1. Read [Custom Workflows](./custom-workflows.md) - Workflow patterns
2. Define workflow steps with durations
3. Implement `Workflow` trait
4. Create task type implementing `Task`
5. Create task factory implementing `TaskFactory`
6. Write workflow tests

**Files to create:**
- `src/workflows/my_workflow.rs` - Workflow and task definitions

---

### Optimizing Performance

1. Profile with flamegraph: `cargo flamegraph`
2. Read [Performance Tuning](./performance-tuning.md) - Optimization strategies
3. Apply relevant patterns:
   - Batch processing
   - Pre-allocation
   - Efficient data structures
4. Benchmark: `cargo bench`
5. Verify improvements

**Tools needed:**
- `cargo-flamegraph`
- `criterion` (benchmarking)
- `heaptrack` (memory profiling)

---

## Testing Your Implementation

### Quick Test Checklist

From [Testing Strategies](./testing-strategies.md):

- [ ] Unit tests for all agents (> 90% coverage)
- [ ] Unit tests for all resources (> 90% coverage)
- [ ] Unit tests for workflows
- [ ] Integration test (24-hour simulation)
- [ ] Framework generality test
- [ ] Serialization round-trip tests
- [ ] Edge case tests
- [ ] Error handling tests

### Running Tests

```bash
# All tests
cargo test

# Specific package
cargo test --package sim-mydomain

# With output
cargo test -- --nocapture

# Coverage
cargo tarpaulin --out Html
```

---

## Performance Targets

From [Performance Tuning](./performance-tuning.md):

**Minimum Targets:**
- Scale: 1,000+ agents
- Throughput: 50+ ticks/second
- Memory: < 10MB per 1,000 agents

**Good Targets:**
- Scale: 10,000+ agents
- Throughput: 100+ ticks/second
- Memory: < 1GB for 10,000 agents
- Latency: < 10ms per tick

**Achieved by sim-logistics:**
- Scale: 10,000+ agents
- Throughput: 686 ticks/second
- Memory: 125 MB for 10,000 agents
- Latency: 1.2ms median per tick

---

## Troubleshooting

### Common Issues

**"My agent position is always None"**

→ Read [Custom Agents](./custom-agents.md) - Position Tracking section
- Return `Some()` only when agent independently tracks position
- Return `None` when position tracked by container

**"Resource never processes tasks"**

→ Read [Custom Resources](./custom-resources.md) - State Transitions section
- Ensure `process_tick()` is called in simulation loop
- Verify capacity constraints are correct

**"Workflow doesn't progress"**

→ Read [Custom Workflows](./custom-workflows.md) - WorkflowEngine Integration
- Call `WorkflowEngine.complete_task()` when tasks finish
- Verify `Task::workflow_instance_id()` returns `Some()`

**"Tests are slow"**

→ Read [Testing Strategies](./testing-strategies.md) - Performance section
- Use mocks for expensive operations
- Avoid real I/O in unit tests
- Use smaller datasets for tests

**"Simulation is slow"**

→ Read [Performance Tuning](./performance-tuning.md)
- Profile with flamegraph
- Apply batch processing patterns
- Check for O(n²) algorithms

---

## Additional Resources

### Framework Documentation

- **Architecture Overview:** `/docs/ARCHITECTURE.md`
- **Framework Core:** `/crates/sim-core/README.md`
- **Events System:** `/crates/sim-events/README.md`
- **Geo Utilities:** `/crates/sim-geo/README.md`

### Example Implementations

- **Logistics:** `/crates/sim-logistics` (complete reference)
- **Kitchen:** `/crates/sim-kitchen` (work in progress)

### Tools and Utilities

- **CLI:** `/crates/cli` - Command-line interface
- **Telemetry:** `/crates/sim-telemetry` - Metrics and observability

---

## Contributing

Found an issue or have a suggestion for improving the guides?

1. Check existing issues
2. Create detailed bug report or enhancement request
3. Include guide name and section
4. Provide example code if applicable

**Guide Maintainers:** These guides are maintained as part of the framework documentation. Keep them updated when framework changes occur.

---

## Summary

**Essential Guides:**
1. [Building a Domain Simulation](./building-domain.md) - Start here
2. [Custom Agents](./custom-agents.md) - Core abstraction
3. [Testing Strategies](./testing-strategies.md) - Quality assurance

**Advanced Guides:**
4. [Custom Resources](./custom-resources.md) - Capacity constraints
5. [Custom Workflows](./custom-workflows.md) - Multi-step processes
6. [Performance Tuning](./performance-tuning.md) - Optimization

**Reference Implementation:** `/crates/sim-logistics` - Study this!

**Time Investment:**
- Quick Start: 4-6 hours
- Complete Domain: 1-2 days
- Advanced Optimization: 4-8 hours

**Support:** Review the guides, study sim-logistics, and experiment!

Happy simulating!
