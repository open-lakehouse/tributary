# Changelog

All notable changes to `sim-core` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.1] - 2026-03-16

### Added

#### Core Abstractions
- **Agent trait**: Generic agent abstraction with tick-based behavior
- **Resource trait**: Capacity-constrained resource processing
- **Task trait**: Unit of work to be processed by resources
- **Workflow trait**: Multi-stage processing pipelines
- **AgentStatus trait**: Type-safe agent state representation

#### Population Management
- **PopulationHandler**: Generic agent lifecycle management
- Agent addition, removal, and retrieval by ID
- Bulk agent tick processing with result aggregation
- Agent statistics tracking (total ticks, active agents)

#### Resource Allocation
- **ResourceAllocator**: Manage resources and task assignment
- **AllocationStrategy trait**: Pluggable allocation algorithms
- Built-in strategies:
  - **RoundRobin**: Distribute tasks evenly across resources
  - **LeastLoaded**: Assign to resource with lowest current load
  - **Random**: Random task assignment
- Automatic capacity tracking and enforcement
- Task completion handling with capacity updates

#### Workflow Engine
- **WorkflowEngine**: Multi-stage workflow orchestration
- **WorkflowStep**: Individual processing steps with resource requirements
- Workflow progression tracking across multiple steps
- Step timing and duration estimates
- Custom metadata support per workflow step

#### Simulation Runner
- **SimulationRunner**: Complete simulation orchestration
- **SimulationHooks trait**: Lifecycle callbacks for custom logic
- Multiple execution modes:
  - Fixed step count (`run_for_steps`)
  - Duration-based (`run_for_duration`)
  - Conditional (`run_until`)
  - Manual tick-by-tick
- Component integration (population, resources, workflows, hooks)
- Comprehensive statistics tracking

#### Context and Time Management
- **SimulationContext**: Runtime state (time, step, config)
- **SimulationConfig**: Simulation parameters and custom settings
- Configurable time step duration
- Start time with UTC timezone support
- Elapsed time tracking

#### Demand Generation
- **DemandGenerator trait**: Flexible demand pattern generation
- **PoissonDemandGenerator**: Poisson process-based demand
- **TimeOfDayDemandGenerator**: Time-dependent demand patterns
- **SpatialDemandGenerator**: Location-based demand
- **CompositeDemandGenerator**: Combine multiple generators

#### Type-Safe Identifiers
- UUID v7-based IDs (time-ordered)
- UUID v5-based IDs (deterministic)
- Type-safe wrappers:
  - `AgentId`
  - `ResourceId`
  - `TaskId`
  - `LocationId`
  - `WorkflowId`

#### Snapshot System
- **SnapshotManager**: Save and restore simulation state
- **SnapshotBackend trait**: Pluggable storage backends
- Arrow RecordBatch-based serialization
- Incremental and full snapshot support
- Metadata tracking (timestamp, version, description)

#### Data Structures
- Arrow RecordBatch integration for efficient data handling
- Journey representation for agent movement
- Demand representation for resource requests
- AgentTickResult aggregation

### Dependencies
- `arrow 57.0`: Columnar data format
- `arrow-schema 57.0`: Arrow schema definitions
- `datafusion 51.0`: Query engine
- `chrono 0.4`: Time handling
- `geo 0.31`: Geospatial types
- `h3o 0.9.3`: H3 hexagonal indexing
- `uuid 1.0`: Unique identifiers (v5, v7)
- `serde 1.0`: Serialization
- `rand 0.8`: Random number generation

### Testing
- Comprehensive unit test coverage
- Integration tests for workflow patterns
- Benchmark suite for performance validation

### Performance
- Efficient agent tick processing
- Minimal allocation overhead
- RecordBatch-based batch operations

[0.0.1]: https://github.com/yourusername/repo/releases/tag/v0.0.1
