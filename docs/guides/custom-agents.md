# Custom Agents Guide

This guide covers advanced patterns for implementing the `Agent` trait, state management, lifecycle methods, and testing strategies.

## Table of Contents

1. [Agent Trait Overview](#agent-trait-overview)
2. [State Management Patterns](#state-management-patterns)
3. [Lifecycle Methods](#lifecycle-methods)
4. [Testing Strategies](#testing-strategies)
5. [Common Patterns](#common-patterns)
6. [Examples from sim-logistics](#examples-from-sim-logistics)

## Agent Trait Overview

The `Agent` trait is the core abstraction for entities that exist in the simulation world. From `/crates/sim-core/src/population/traits.rs`:

```rust
pub trait Agent: Clone + Send + Sync + 'static {
    fn id(&self) -> AgentId;
    fn agent_type(&self) -> &str;
    fn status(&self) -> &dyn AgentStatus;
    fn position(&self) -> Option<Point<f64>>;
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult>;
    fn state_data(&self) -> Value;
    fn to_arrow_row(&self) -> ArrowAgentRow;
    fn from_arrow_row(row: &ArrowAgentRow) -> Result<Self>;
}
```

### Required Trait Bounds

- **`Clone`**: Agents must be cloneable for snapshots and parallel processing
- **`Send + Sync`**: Thread-safe for concurrent access
- **`'static`**: No non-static references (owned data only)

## State Management Patterns

### Pattern 1: Enum-Based State Machine

Best for agents with distinct states and state-specific data.

**Example: Package (Passive Agent)**

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum PackageState {
    Created { origin: Point<f64>, destination: Point<f64> },
    Sorted { warehouse_id: String, destination: Point<f64> },
    Loaded { driver_id: String, destination: Point<f64> },
    InTransit {
        driver_id: String,
        destination: Point<f64>,
        estimated_arrival: DateTime<Utc>
    },
    Delivered { delivery_time: DateTime<Utc>, destination: Point<f64> },
}

impl AgentStatus for PackageState {
    fn as_str(&self) -> &str {
        match self {
            Self::Created { .. } => "created",
            Self::Sorted { .. } => "sorted",
            Self::Loaded { .. } => "loaded",
            Self::InTransit { .. } => "in_transit",
            Self::Delivered { .. } => "delivered",
        }
    }

    fn is_idle(&self) -> bool {
        matches!(self, Self::Delivered { .. })
    }

    fn is_active(&self) -> bool {
        !matches!(self, Self::Delivered { .. })
    }
}
```

**Benefits:**

- Type-safe state transitions
- State-specific data encoded in variants
- Compiler ensures you handle all states
- Clear state machine documentation

**When to use:**

- Agents with 3-10 distinct states
- Each state has different data requirements
- Clear state transition rules

### Pattern 2: Struct-Based State

Best for agents with many numeric fields that change gradually.

**Example: Driver (Active Agent)**

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum DriverState {
    Idle,
    TravelingToWarehouse { warehouse_location: Point<f64>, warehouse_id: String },
    LoadingPackages { warehouse_id: String, package_count: u32, loading_started_at: u64 },
    DeliveringPackages {
        current_destination: Point<f64>,
        remaining_stops: u32,
        packages_on_board: Vec<String>,
    },
    Returning { home_location: Point<f64> },
}

#[derive(Debug, Clone)]
pub struct Driver {
    pub id: AgentId,
    pub state: DriverState,
    pub location: Point<f64>,
    pub home_location: Point<f64>,
    pub movement_speed: f64,
    pub capacity: u32,
    pub tick_count: u64,
    // Metadata for state transitions
    metadata_destinations: Vec<Point<f64>>,
    metadata_package_ids: Vec<String>,
}
```

**Benefits:**

- Separate state enum from agent struct
- Agent struct holds stable properties
- Easy to add computed properties
- Clear separation of state vs. configuration

**When to use:**

- Agents with both state and stable properties
- Agents that need intermediate transition data
- Complex agents with many fields

### Pattern 3: Composite State

Best for agents with multiple independent state dimensions.

```rust
#[derive(Debug, Clone)]
pub struct Worker {
    pub id: AgentId,
    pub work_state: WorkState,
    pub energy_state: EnergyState,
    pub location_state: LocationState,
}

#[derive(Debug, Clone)]
pub enum WorkState {
    Idle,
    Working { task_id: TaskId, started_at: u64 },
    OnBreak { break_type: BreakType },
}

#[derive(Debug, Clone)]
pub enum EnergyState {
    Rested { energy: f64 },
    Tired { energy: f64 },
    Exhausted,
}

#[derive(Debug, Clone)]
pub enum LocationState {
    AtStation { station_id: String },
    Moving { from: Point<f64>, to: Point<f64>, progress: f64 },
}
```

**Benefits:**

- Multiple orthogonal state dimensions
- Each dimension evolves independently
- Clear separation of concerns

**When to use:**

- Agents with multiple independent state aspects
- Complex agents with many state combinations
- Need to query state dimensions independently

## Lifecycle Methods

### Initialization

Agents are created via constructors, not trait methods:

```rust
impl Package {
    pub fn new(
        origin: Point<f64>,
        destination: Point<f64>,
        weight_kg: f64,
        priority: PackagePriority,
    ) -> Self {
        Self {
            id: AgentId::new(),
            state: PackageState::Created { origin, destination },
            tracking_number: format!("PKG-{}", Uuid::new_v4().simple()),
            created_at: Utc::now(),
            delivered_at: None,
            weight_kg,
            priority,
        }
    }

    pub fn with_custom_id(id: AgentId, /* ... */) -> Self {
        // For testing or deserialization
    }
}
```

**Best practices:**

- Provide multiple constructors for different use cases
- Use `AgentId::new()` for production
- Allow custom IDs for testing
- Initialize all fields explicitly
- Validate inputs in constructor

### Tick Method (Core Lifecycle)

The `tick()` method is called every simulation step:

**Passive Agent Pattern:**

```rust
impl Agent for Package {
    fn tick(&mut self, _ctx: &SimulationContext) -> Result<AgentTickResult> {
        // Passive agents don't update themselves
        // State changes driven externally by resources
        Ok(AgentTickResult::default())
    }
}
```

**Active Agent Pattern:**

```rust
impl Agent for Driver {
    fn tick(&mut self, _ctx: &SimulationContext) -> Result<AgentTickResult> {
        let mut result = AgentTickResult::default();
        self.tick_count += 1;

        let step_duration = 30.0; // seconds

        match &self.state.clone() {
            DriverState::Idle => {
                // Waiting for assignment
            }

            DriverState::TravelingToWarehouse { warehouse_location, warehouse_id } => {
                let arrived = self.move_towards(*warehouse_location, step_duration);

                if arrived {
                    // Transition to next state
                    let package_count = self.metadata_package_ids.len() as u32;
                    self.state = DriverState::LoadingPackages {
                        warehouse_id: warehouse_id.clone(),
                        package_count,
                        loading_started_at: self.tick_count,
                    };

                    result.state_changes.insert(
                        "arrived_at_warehouse".to_string(),
                        Value::String(warehouse_id.clone()),
                    );
                }
            }

            DriverState::LoadingPackages { loading_started_at, .. } => {
                let loading_duration = 12; // ticks
                let elapsed_ticks = self.tick_count - loading_started_at;

                if elapsed_ticks >= loading_duration {
                    // Transition to delivering
                    self.state = DriverState::DeliveringPackages { /* ... */ };
                    result.state_changes.insert("loading_complete".to_string(), json!(true));
                }
            }

            // ... more states
        }

        Ok(result)
    }
}
```

**Tick method patterns:**

- Clone state at start: `match &self.state.clone()`
- Update internal counters: `self.tick_count += 1`
- Check transition conditions
- Mutate self for state changes
- Return state changes in result
- Return journeys for movement requests
- Return demands for spawning new work

### Position Tracking

The `position()` method has important semantics:

**Return Some() when:**

- Agent independently tracks its location
- Agent is spatially autonomous
- Agent has its own movement behavior

**Return None when:**

- Agent's position is tracked by another entity
- Agent is contained (package in vehicle)
- Agent is at a fixed location (tracked by facility)

```rust
impl Agent for Package {
    fn position(&self) -> Option<Point<f64>> {
        match &self.state {
            PackageState::Created { origin, .. } => Some(*origin),
            PackageState::Sorted { .. } => None, // At warehouse
            PackageState::Loaded { .. } => None, // On vehicle
            PackageState::InTransit { .. } => None, // On vehicle
            PackageState::Delivered { destination, .. } => Some(*destination),
        }
    }
}

impl Agent for Driver {
    fn position(&self) -> Option<Point<f64>> {
        Some(self.location) // Always has independent position
    }
}
```

### Serialization

Agents must serialize to Arrow format for persistence:

**state_data() - Custom Fields:**

```rust
fn state_data(&self) -> Value {
    let mut map = serde_json::Map::new();

    map.insert("tracking_number".to_string(), json!(self.tracking_number));
    map.insert("weight_kg".to_string(), json!(self.weight_kg));
    map.insert("priority".to_string(), json!(match self.priority {
        PackagePriority::Standard => "standard",
        PackagePriority::Express => "express",
        PackagePriority::Overnight => "overnight",
    }));

    // Add state-specific fields
    match &self.state {
        PackageState::Sorted { warehouse_id, destination } => {
            map.insert("warehouse_id".to_string(), json!(warehouse_id));
            map.insert("dest_x".to_string(), json!(destination.x()));
            map.insert("dest_y".to_string(), json!(destination.y()));
        }
        // ... other states
        _ => {}
    }

    Value::Object(map)
}
```

**from_arrow_row() - Deserialization:**

```rust
fn from_arrow_row(row: &ArrowAgentRow) -> Result<Self> {
    let id = AgentId::from_uuid(
        Uuid::from_slice(&row.id)
            .map_err(|e| Error::Agent(format!("Invalid UUID: {}", e)))?
    );

    let metadata: Value = if let Some(ref m) = row.metadata {
        serde_json::from_str(m).unwrap_or_default()
    } else {
        Value::Null
    };

    // Extract fields
    let tracking_number = metadata.get("tracking_number")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let weight_kg = metadata.get("weight_kg")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);

    // Reconstruct state from status string and metadata
    let state = match row.status.as_str() {
        "created" => {
            let origin_x = metadata.get("origin_x").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let origin_y = metadata.get("origin_y").and_then(|v| v.as_f64()).unwrap_or(0.0);
            // ... extract destination
            PackageState::Created { origin, destination }
        }
        "sorted" => {
            let warehouse_id = metadata.get("warehouse_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            // ... extract destination
            PackageState::Sorted { warehouse_id, destination }
        }
        _ => PackageState::Created { /* defaults */ }
    };

    Ok(Package {
        id,
        state,
        tracking_number,
        created_at: Utc::now(),
        delivered_at: None,
        weight_kg,
        priority: PackagePriority::Standard,
    })
}
```

## Testing Strategies

### Unit Test Structure

From `/crates/sim-logistics/src/agents/package.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_creation() {
        let origin = Point::new(-122.4, 37.8);
        let destination = Point::new(-122.5, 37.9);
        let pkg = Package::new(origin, destination, 2.5, PackagePriority::Standard);

        assert_eq!(pkg.agent_type(), "package");
        assert_eq!(pkg.state.as_str(), "created");
        assert_eq!(pkg.weight_kg, 2.5);
        assert!(!pkg.is_delivered());
    }

    #[test]
    fn test_package_state_transitions() {
        let mut pkg = Package::new(
            Point::new(0.0, 0.0),
            Point::new(1.0, 1.0),
            1.0,
            PackagePriority::Standard,
        );

        // Test each transition
        pkg.mark_sorted("warehouse-1".to_string());
        assert_eq!(pkg.state.as_str(), "sorted");

        pkg.mark_loaded("driver-1".to_string());
        assert_eq!(pkg.state.as_str(), "loaded");

        pkg.mark_in_transit(Utc::now());
        assert_eq!(pkg.state.as_str(), "in_transit");

        pkg.mark_delivered();
        assert_eq!(pkg.state.as_str(), "delivered");
        assert!(pkg.is_delivered());
    }

    #[test]
    fn test_position_semantics() {
        let mut pkg = Package::new(
            Point::new(0.0, 0.0),
            Point::new(1.0, 1.0),
            1.0,
            PackagePriority::Standard,
        );

        // Has position when created
        assert_eq!(pkg.position(), Some(Point::new(0.0, 0.0)));

        // No position when sorted (at warehouse)
        pkg.mark_sorted("w1".to_string());
        assert_eq!(pkg.position(), None);

        // No position when loaded (on vehicle)
        pkg.mark_loaded("d1".to_string());
        assert_eq!(pkg.position(), None);

        // Has position when delivered
        pkg.mark_in_transit(Utc::now());
        pkg.mark_delivered();
        assert_eq!(pkg.position(), Some(Point::new(1.0, 1.0)));
    }

    #[test]
    fn test_arrow_round_trip() {
        let pkg = Package::new(
            Point::new(1.0, 2.0),
            Point::new(3.0, 4.0),
            2.5,
            PackagePriority::Express,
        );

        let row = pkg.to_arrow_row();
        let restored = Package::from_arrow_row(&row).unwrap();

        assert_eq!(pkg.id, restored.id);
        assert_eq!(pkg.state.as_str(), restored.state.as_str());
        assert_eq!(pkg.weight_kg, restored.weight_kg);
    }
}
```

### Test Categories

**1. Creation Tests**

- Test all constructor variants
- Verify initial state
- Check default values

**2. State Transition Tests**

- Test each valid transition
- Verify state changes
- Check state-specific data

**3. Behavior Tests**

- Test tick() method
- Verify state changes during tick
- Check returned values (journeys, demands)

**4. Serialization Tests**

- Test state_data() output
- Test Arrow round-trip
- Verify no data loss

**5. Edge Case Tests**

- Test invalid transitions
- Test boundary conditions
- Test error handling

### Active Agent Testing Pattern

From `/crates/sim-logistics/src/agents/driver.rs`:

```rust
#[test]
fn test_transition_to_loading() {
    let home = Point::new(0.0, 0.0);
    let mut driver = Driver::new(home);

    // Very close warehouse for immediate arrival
    let warehouse = Point::new(0.001, 0.001);
    let destinations = vec![Point::new(2.0, 2.0)];
    let package_ids = vec!["pkg-1".to_string()];

    driver.assign_delivery(
        warehouse,
        "warehouse-1".to_string(),
        destinations,
        package_ids,
    );

    let ctx = SimulationContext::from_now(SimulationConfig::default());
    let result = driver.tick(&ctx).unwrap();

    // Should transition to loading
    assert_eq!(driver.state.as_str(), "loading_packages");
    assert!(result.state_changes.contains_key("arrived_at_warehouse"));
}

#[test]
fn test_loading_duration() {
    let mut driver = Driver::new(Point::new(0.0, 0.0));
    driver.state = DriverState::LoadingPackages {
        warehouse_id: "w1".to_string(),
        package_count: 5,
        loading_started_at: 0,
    };
    driver.metadata_destinations = vec![Point::new(1.0, 1.0)];
    driver.metadata_package_ids = vec!["pkg-1".to_string()];

    let ctx = SimulationContext::from_now(SimulationConfig::default());

    // Loading takes 12 ticks
    for _ in 0..11 {
        driver.tick(&ctx).unwrap();
        assert_eq!(driver.state.as_str(), "loading_packages");
    }

    // 12th tick should complete loading
    let result = driver.tick(&ctx).unwrap();
    assert_eq!(driver.state.as_str(), "delivering_packages");
    assert!(result.state_changes.contains_key("loading_complete"));
}
```

## Common Patterns

### Movement Pattern

```rust
fn move_towards(&mut self, target: Point<f64>, step_duration_secs: f64) -> bool {
    let dx = target.x() - self.location.x();
    let dy = target.y() - self.location.y();
    let distance = (dx * dx + dy * dy).sqrt();

    let max_travel = self.movement_speed * step_duration_secs;

    if distance <= max_travel {
        self.location = target;
        true // Arrived
    } else {
        let ratio = max_travel / distance;
        let new_x = self.location.x() + dx * ratio;
        let new_y = self.location.y() + dy * ratio;
        self.location = Point::new(new_x, new_y);
        false // Still moving
    }
}
```

### Timing Pattern

```rust
// Track when something started
pub fn start_activity(&mut self) {
    self.activity_started_at = self.tick_count;
}

// Check if activity is complete
pub fn is_activity_complete(&self, duration_ticks: u64) -> bool {
    let elapsed = self.tick_count - self.activity_started_at;
    elapsed >= duration_ticks
}
```

### Metadata Pattern

```rust
// Temporary storage for complex state transitions
pub struct Agent {
    // ... fields

    // Temporary metadata for state transitions
    metadata_destinations: Vec<Point<f64>>,
    metadata_package_ids: Vec<String>,
}

// Use during transitions
fn assign_delivery(&mut self, destinations: Vec<Point<f64>>, packages: Vec<String>) {
    self.metadata_destinations = destinations;
    self.metadata_package_ids = packages;
    self.state = State::Assigned;
}

// Clear after use
fn complete_delivery(&mut self) {
    self.metadata_destinations.clear();
    self.metadata_package_ids.clear();
    self.state = State::Idle;
}
```

## Examples from sim-logistics

### Package (Passive Agent)

**Location:** `/crates/sim-logistics/src/agents/package.rs`

**Characteristics:**

- 5-state lifecycle
- Passive behavior (no autonomous actions)
- Position tracked by container (warehouse/vehicle)
- Rich metadata (tracking number, weight, priority)

**Key Methods:**

- `mark_sorted()`, `mark_loaded()`, `mark_in_transit()`, `mark_delivered()` - State transitions
- `delivery_time_seconds()` - SLA metric
- `is_delivered()` - Completion check

### Driver (Active Agent)

**Location:** `/crates/sim-logistics/src/agents/driver.rs`

**Characteristics:**

- 5-state autonomous behavior
- Active movement and decision-making
- Capacity constraints
- Timing-based state transitions

**Key Methods:**

- `assign_delivery()` - External trigger
- `move_towards()` - Autonomous movement
- `tick()` - Complex state machine
- `current_package_count()` - State query

## Best Practices

1. **State Design**: Use enums for distinct states, structs for gradual changes
2. **Position Semantics**: Return None when position is tracked elsewhere
3. **Passive vs Active**: Choose based on autonomy requirements
4. **Serialization**: Test round-trip early and often
5. **Testing**: Aim for >90% coverage with unit tests
6. **Documentation**: Include state machine diagrams in doc comments
7. **Timing**: Use tick counts, not wall clock time
8. **Metadata**: Clear temporary data after use

## Next Steps

- Read [Custom Resources Guide](./custom-resources.md) for resource patterns
- Read [Custom Workflows Guide](./custom-workflows.md) for workflow integration
- Read [Testing Strategies](./testing-strategies.md) for comprehensive testing
