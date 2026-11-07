# sim-events API Reference

`sim-events` provides CloudEvents-compliant event emission and persistence for agent-based simulations. Events are the primary mechanism for recording state transitions and enabling observability.

## Table of Contents

- [CloudEvents Overview](#cloudevents-overview)
- [Event Core Types](#event-core-types)
- [Event Dispatcher](#event-dispatcher)
- [Storage](#storage)
- [Best Practices](#best-practices)

---

## CloudEvents Overview

All events follow the [CloudEvents 1.0 specification](https://cloudevents.io/):

**Required Fields:**
- `id` - Unique event identifier (UUID v7 for time-ordering)
- `source` - URI identifying the event source
- `specversion` - CloudEvents version ("1.0")
- `type` - Event type (reverse domain notation)

**Optional Fields:**
- `time` - When the event occurred (ISO 8601)
- `datacontenttype` - MIME type of data payload
- `data` - Event payload (domain-specific)
- `subject` - Subject of the event in context

---

## Event Core Types

### Event

Wrapper containing CloudEvents metadata and payload.

```rust
pub struct Event { /* private fields */ }

impl Event {
    /// `time` is the simulation time of the occurrence (`ctx.current_time()`)
    pub fn new(payload: Box<dyn EventPayload>, time: DateTime<Utc>) -> Self;

    pub fn id(&self) -> EventId;               // UUID v7
    pub fn event_type(&self) -> &str;          // from the payload
    pub fn source(&self) -> &str;              // from the payload
    pub fn subject(&self) -> Option<&str>;     // from the payload
    pub fn time(&self) -> DateTime<Utc>;
    pub fn payload(&self) -> &dyn EventPayload;
    pub fn to_cloudevents_json(&self) -> serde_json::Value;
    pub fn to_record(&self) -> EventRecord;
}
```

**Example:**

```rust
use sim_events::Event;

// Create event with custom payload
let event = Event::new(Box::new(OrderCreatedEvent {
    order_id: "order-123".to_string(),
    customer_id: "customer-456".to_string(),
    items: vec!["burger".to_string(), "fries".to_string()],
    total_amount: 15.99,
}), ctx.current_time());
```

### EventPayload

Trait for domain-specific event data.

```rust
pub trait EventPayload: Send + Sync + std::fmt::Debug {
    /// CloudEvents type (e.g., "com.example.order.created")
    fn event_type(&self) -> &str;

    /// CloudEvents source (e.g., "/orders/order-123")
    fn source(&self) -> String;

    /// Serialize payload to JSON
    fn to_json(&self) -> serde_json::Value;

    /// Optional: CloudEvents subject
    fn subject(&self) -> Option<String> {
        None
    }

    /// Optional: MIME type of data
    fn datacontenttype(&self) -> String {
        "application/json".to_string()
    }
}
```

**Example Implementation:**

```rust
use sim_events::EventPayload;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCreatedEvent {
    pub order_id: String,
    pub customer_id: String,
    pub items: Vec<String>,
    pub total_amount: f64,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for OrderCreatedEvent {
    fn event_type(&self) -> &str {
        "io.kitchen.order.created"
    }

    fn source(&self) -> String {
        format!("/orders/{}", self.order_id)
    }

    fn subject(&self) -> Option<String> {
        Some(format!("customer/{}", self.customer_id))
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}
```

### EventRecord

Flattened representation for storage.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: String,
    pub source: String,
    pub specversion: String,
    pub type_: String,
    pub time: String,
    pub datacontenttype: String,
    pub data_json: String,
    pub subject: Option<String>,
}

impl EventRecord {
    pub fn from_event(event: &Event) -> Self;
    pub fn to_event(&self) -> Result<Event>;
}
```

---

## Event Dispatcher

Central event bus for emitting and collecting events.

```rust
pub struct EventDispatcher {
    pub fn new() -> Self;
    pub fn emit(&mut self, event: Event) -> Result<()>;
    pub fn drain(&mut self) -> Vec<Event>;
    pub fn count(&self) -> usize;
    pub fn clear(&mut self);
}
```

**Example:**

```rust
use sim_events::EventDispatcher;

let mut dispatcher = EventDispatcher::new();

// Emit events
dispatcher.emit(Event::new(Box::new(OrderCreatedEvent { /* ... */ }), ctx.current_time()));
dispatcher.emit(Event::new(Box::new(OrderAssignedEvent { /* ... */ }), ctx.current_time()));

// Drain for persistence
let events = dispatcher.drain();
println!("Collected {} events", events.len());

// Clear buffer
dispatcher.clear();
```

### Integration with SimulationHooks

```rust
use sim_core::SimulationHooks;
use sim_events::EventDispatcher;

struct MySimulationHooks {
    event_dispatcher: EventDispatcher,
}

impl SimulationHooks for MySimulationHooks {
    fn on_resources_allocated(
        &mut self,
        ctx: &SimulationContext,
        assignments: &[Assignment]
    ) -> Result<()> {
        // Emit assignment events
        for assignment in assignments {
            let event = Event::new(Box::new(TaskAssignedEvent {
                task_id: assignment.task_id.to_string(),
                resource_id: assignment.resource_id.to_string(),
                timestamp: ctx.current_time(),
            }), ctx.current_time());
            self.event_dispatcher.emit(event);
        }
        Ok(())
    }

    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        // Periodically flush events to storage
        if ctx.step() % 100 == 0 {
            let events = self.event_dispatcher.drain();
            // Write to storage...
        }
        Ok(())
    }
}
```

---

## Storage

### ParquetEventWriter

Persistent event storage using Apache Parquet.

```rust
pub struct ParquetEventWriter {
    pub fn new(output_path: impl Into<PathBuf>) -> Result<Self>;
    pub fn write_events(&mut self, events: &[Event]) -> Result<()>;
    pub fn flush(&mut self) -> Result<()>;
}
```

**Example:**

```rust
use sim_events::{EventDispatcher, ParquetEventWriter};

let mut dispatcher = EventDispatcher::new();
let mut writer = ParquetEventWriter::new("./events/simulation.parquet")?;

// Emit events during simulation
dispatcher.emit(event1);
dispatcher.emit(event2);

// Periodically flush to disk
let events = dispatcher.drain();
writer.write_events(&events)?;
writer.flush()?;
```

**Schema:**

The Parquet files use the following schema:

```
id: String
source: String
specversion: String (always "1.0")
type: String
time: String (ISO 8601)
datacontenttype: String
data_json: String (JSON serialized data field)
subject: Optional String
```

### Reading Events

```rust
use parquet::file::reader::{FileReader, SerializedFileReader};
use std::fs::File;

let file = File::open("events/simulation.parquet")?;
let reader = SerializedFileReader::new(file)?;

let mut records = Vec::new();
for row_group in reader.row_groups() {
    // Read and process events
}
```

---

## Common Event Patterns

sim-events ships no event payloads. The payloads below illustrate patterns
you would define in a domain crate (see `crates/sim-kitchen/src/events.rs`
and `crates/sim-logistics/src/events.rs` for real ones).

### Agent Lifecycle Events

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCreatedEvent {
    pub agent_id: String,
    pub agent_type: String,
    pub position: Option<(f64, f64)>,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for AgentCreatedEvent {
    fn event_type(&self) -> &str {
        "io.simulation.agent.created"
    }

    fn source(&self) -> String {
        format!("/agents/{}", self.agent_id)
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "agent_id": self.agent_id,
            "agent_type": self.agent_type,
            "position": self.position,
            "timestamp": self.timestamp.to_rfc3339(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStateChangedEvent {
    pub agent_id: String,
    pub previous_status: String,
    pub new_status: String,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for AgentStateChangedEvent {
    fn event_type(&self) -> &str {
        "io.simulation.agent.state_changed"
    }

    fn source(&self) -> String {
        format!("/agents/{}", self.agent_id)
    }

    fn subject(&self) -> Option<String> {
        Some(format!("status/{}", self.new_status))
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "agent_id": self.agent_id,
            "previous_status": self.previous_status,
            "new_status": self.new_status,
            "timestamp": self.timestamp.to_rfc3339(),
        })
    }
}
```

### Resource Events

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignedEvent {
    pub task_id: String,
    pub resource_id: String,
    pub resource_type: String,
    pub estimated_completion: DateTime<Utc>,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for TaskAssignedEvent {
    fn event_type(&self) -> &str {
        "io.simulation.task.assigned"
    }

    fn source(&self) -> String {
        format!("/tasks/{}", self.task_id)
    }

    fn subject(&self) -> Option<String> {
        Some(format!("resource/{}", self.resource_id))
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "task_id": self.task_id,
            "resource_id": self.resource_id,
            "resource_type": self.resource_type,
            "estimated_completion": self.estimated_completion.to_rfc3339(),
            "timestamp": self.timestamp.to_rfc3339(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCompletedEvent {
    pub task_id: String,
    pub resource_id: String,
    pub duration_seconds: f64,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for TaskCompletedEvent {
    fn event_type(&self) -> &str {
        "io.simulation.task.completed"
    }

    fn source(&self) -> String {
        format!("/tasks/{}", self.task_id)
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "task_id": self.task_id,
            "resource_id": self.resource_id,
            "duration_seconds": self.duration_seconds,
            "timestamp": self.timestamp.to_rfc3339(),
        })
    }
}
```

### Workflow Events

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStepStartedEvent {
    pub workflow_id: String,
    pub instance_id: String,
    pub step_index: usize,
    pub step_type: String,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for WorkflowStepStartedEvent {
    fn event_type(&self) -> &str {
        "io.simulation.workflow.step_started"
    }

    fn source(&self) -> String {
        format!("/workflows/{}/instances/{}", self.workflow_id, self.instance_id)
    }

    fn subject(&self) -> Option<String> {
        Some(format!("step/{}", self.step_index))
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "workflow_id": self.workflow_id,
            "instance_id": self.instance_id,
            "step_index": self.step_index,
            "step_type": self.step_type,
            "timestamp": self.timestamp.to_rfc3339(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowCompletedEvent {
    pub workflow_id: String,
    pub instance_id: String,
    pub total_duration_seconds: f64,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for WorkflowCompletedEvent {
    fn event_type(&self) -> &str {
        "io.simulation.workflow.completed"
    }

    fn source(&self) -> String {
        format!("/workflows/{}/instances/{}", self.workflow_id, self.instance_id)
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "workflow_id": self.workflow_id,
            "instance_id": self.instance_id,
            "total_duration_seconds": self.total_duration_seconds,
            "timestamp": self.timestamp.to_rfc3339(),
        })
    }
}
```

---

## Best Practices

### Event Naming

Use reverse domain notation for event types:

```
{domain}.{entity}.{action}

Examples:
- io.kitchen.order.created
- io.kitchen.order.assigned
- io.kitchen.order.completed
- io.logistics.package.sorted
- io.logistics.package.delivered
```

### Source URIs

Use hierarchical paths for event sources:

```
/{entity_type}/{entity_id}
/{service}/{entity_type}/{entity_id}

Examples:
- /orders/order-123
- /agents/customer-456
- /workflows/cooking/instances/instance-789
```

### Subject Field

Use subjects to categorize events within a source:

```
{category}/{value}

Examples:
- status/completed
- customer/customer-456
- resource/kitchen-1
- step/2
```

### Batch Emission

Emit events in batches for performance:

```rust
impl SimulationHooks for MyHooks {
    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        // Flush every 100 steps or at simulation end
        if ctx.step() % 100 == 0 || ctx.is_final_step() {
            let events = self.dispatcher.drain();
            if !events.is_empty() {
                self.writer.write_events(&events)?;
                self.writer.flush()?;
            }
        }
        Ok(())
    }
}
```

### Error Handling

```rust
// Graceful degradation: Don't fail simulation on event errors
match dispatcher.emit(event) {
    Ok(_) => {}
    Err(e) => {
        eprintln!("Warning: Failed to emit event: {}", e);
        // Continue simulation
    }
}
```

### Event Volume Management

For high-frequency events, use sampling:

```rust
struct SamplingEventEmitter {
    dispatcher: EventDispatcher,
    sample_rate: f64,  // 0.0 to 1.0
}

impl SamplingEventEmitter {
    fn emit_sampled(&mut self, event: Event) -> Result<()> {
        if rand::random::<f64>() < self.sample_rate {
            self.dispatcher.emit(event);
        }
        Ok(())
    }
}
```

### Querying Events

Using DuckDB for analysis:

```sql
-- Load events from Parquet
CREATE TABLE events AS
SELECT * FROM read_parquet('events/simulation.parquet');

-- Count events by type
SELECT type, COUNT(*) as count
FROM events
GROUP BY type
ORDER BY count DESC;

-- Time series analysis
SELECT
    DATE_TRUNC('minute', CAST(time AS TIMESTAMP)) as minute,
    COUNT(*) as event_count
FROM events
GROUP BY minute
ORDER BY minute;

-- Find events for specific entity
SELECT *
FROM events
WHERE source LIKE '%order-123%'
ORDER BY time;
```

---

## Complete Example

```rust
use sim_core::{SimulationRunner, SimulationHooks, SimulationContext};
use sim_events::{Event, EventPayload, EventDispatcher, ParquetEventWriter};

// Define event types
#[derive(Debug, Clone, Serialize)]
struct OrderCreatedEvent { /* ... */ }

impl EventPayload for OrderCreatedEvent { /* ... */ }

// Create hooks with event handling
struct EventEmittingHooks {
    dispatcher: EventDispatcher,
    writer: ParquetEventWriter,
}

impl SimulationHooks for EventEmittingHooks {
    fn on_resources_allocated(
        &mut self,
        ctx: &SimulationContext,
        assignments: &[Assignment]
    ) -> Result<()> {
        for assignment in assignments {
            let event = Event::new(Box::new(TaskAssignedEvent {
                task_id: assignment.task_id.to_string(),
                resource_id: assignment.resource_id.to_string(),
                timestamp: ctx.current_time(),
            }), ctx.current_time());
            self.dispatcher.emit(event);
        }
        Ok(())
    }

    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        // Flush every 100 steps
        if ctx.step() % 100 == 0 {
            let events = self.dispatcher.drain();
            self.writer.write_events(&events)?;
            self.writer.flush()?;
        }
        Ok(())
    }

    fn on_end(&mut self, _ctx: &SimulationContext) -> Result<()> {
        // Final flush
        let events = self.dispatcher.drain();
        self.writer.write_events(&events)?;
        self.writer.flush()?;
        Ok(())
    }
}

// Setup simulation
let mut runner = SimulationRunner::new(config);
runner.set_hooks(Box::new(EventEmittingHooks {
    dispatcher: EventDispatcher::new(),
    writer: ParquetEventWriter::new("./events/simulation.parquet")?,
}));

// Run
runner.run_for_steps(1000)?;
```

---

## See Also

- [sim-core API Reference](./sim-core.md) - Core simulation abstractions
- [sim-telemetry API Reference](./sim-telemetry.md) - Telemetry generation from events
- [Kitchen Example](../examples/kitchen/basic.md) - Event emission in practice
