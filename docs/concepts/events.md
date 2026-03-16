# Events

**Last Updated:** March 16, 2026

## Overview

The event infrastructure in Tributary provides CloudEvents 1.0 compliant event emission and storage. All simulation state changes can be captured as standardized events and exported to Parquet or JSON Lines formats for analysis.

## Objective

Learn how to:
- Define custom event payloads
- Emit events during simulation
- Export to Parquet and JSON Lines
- Query events with DataFusion
- Integrate with workflows and agents

## Prerequisites

- [Getting Started](../GETTING_STARTED.md)
- Basic understanding of CloudEvents specification
- Knowledge of [Agents](agents.md)

## CloudEvents 1.0 Specification

All events follow the CloudEvents standard:

```json
{
  "specversion": "1.0",
  "id": "019cf220-a8b2-7000-8000-000000000000",
  "source": "/agents/019cf220-...",
  "type": "io.simulation.agent.spawned",
  "time": "2026-03-16T10:00:00Z",
  "subject": "customer-001",
  "datacontenttype": "application/json",
  "data": {
    "agent_id": "019cf220-...",
    "agent_type": "customer",
    "location": "downtown"
  }
}
```

## EventPayload Trait

Define custom event types by implementing `EventPayload`:

```rust
use sim_events::EventPayload;
use serde_json::json;

#[derive(Debug)]
pub struct OrderCreatedEvent {
    pub order_id: TaskId,
    pub customer_id: AgentId,
    pub items: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for OrderCreatedEvent {
    fn event_type(&self) -> &str {
        "io.kitchen.order.created"
    }

    fn source(&self) -> String {
        format!("/customers/{}", self.customer_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.order_id.to_string())
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "order_id": self.order_id.to_string(),
            "customer_id": self.customer_id.to_string(),
            "items": self.items,
            "timestamp": self.timestamp.to_rfc3339(),
        })
    }
}
```

## EventDispatcher

The EventDispatcher collects and emits events:

```rust
use sim_events::{Event, EventDispatcher};

let mut dispatcher = EventDispatcher::new();

// Create and emit event
let event = Event::new(Box::new(OrderCreatedEvent {
    order_id: TaskId::new(),
    customer_id: AgentId::new(),
    items: vec!["burger".to_string(), "fries".to_string()],
    timestamp: Utc::now(),
}));

dispatcher.emit(event);

// Check buffer
println!("Buffered events: {}", dispatcher.buffered_count());
println!("Order events: {}", dispatcher.count_by_type("io.kitchen.order.created"));
```

## Storage Formats

### Parquet Export

Efficient columnar storage with compression:

```rust
// Export to Parquet
let count = dispatcher.flush_to_parquet("events.parquet")?;
println!("Wrote {} events to Parquet", count);

// Benefits:
// - 9x compression with Snappy
// - Columnar format for analytics
// - Arrow-compatible
// - Query with DataFusion, DuckDB, Polars
```

### JSON Lines Export

Human-readable line-delimited JSON:

```rust
// Export to JSON Lines
let count = dispatcher.flush_to_jsonl("events.jsonl")?;
println!("Wrote {} events to JSON Lines", count);

// Benefits:
// - Human-readable
// - Line-based processing
// - Compatible with jq, grep
// - Standard CloudEvents format
```

## Event Types

Use reverse-DNS notation for event types:

### Agent Events
- `io.simulation.agent.spawned`
- `io.simulation.agent.status_changed`
- `io.simulation.agent.departed`

### Resource Events
- `io.simulation.resource.allocated`
- `io.simulation.resource.released`
- `io.simulation.task.created`
- `io.simulation.task.completed`

### Workflow Events
- `io.simulation.workflow.started`
- `io.simulation.workflow.step_advanced`
- `io.simulation.workflow.completed`

### Domain-Specific Events
- `io.kitchen.order.created`
- `io.kitchen.order.assigned`
- `io.kitchen.order.completed`
- `io.logistics.package.sorted`
- `io.logistics.package.loaded`
- `io.logistics.package.delivered`

## Integration Patterns

### Pattern 1: Agent State Changes

Emit events from agent ticks:

```rust
impl SimulationHooks for MyHooks {
    fn on_agents_ticked(
        &mut self,
        ctx: &SimulationContext,
        results: &[AgentTickResult],
    ) -> Result<()> {
        for result in results {
            // Emit state change events
            for change in &result.state_changes {
                let event = Event::new(Box::new(AgentStatusChangedEvent {
                    agent_id: change.agent_id,
                    old_status: change.old_status.clone(),
                    new_status: change.new_status.clone(),
                    timestamp: ctx.current_time(),
                }));
                self.dispatcher.emit(event);
            }
        }
        Ok(())
    }
}
```

### Pattern 2: Workflow Progression

Track workflow state:

```rust
impl SimulationHooks for MyHooks {
    fn on_workflow_advanced(
        &mut self,
        instance_id: WorkflowInstanceId,
        step_index: usize,
    ) -> Result<()> {
        let event = Event::new(Box::new(WorkflowStepAdvancedEvent {
            instance_id,
            step_index,
            timestamp: Utc::now(),
        }));
        self.dispatcher.emit(event);
        Ok(())
    }

    fn on_workflow_completed(
        &mut self,
        instance_id: WorkflowInstanceId,
    ) -> Result<()> {
        let event = Event::new(Box::new(WorkflowCompletedEvent {
            instance_id,
            timestamp: Utc::now(),
        }));
        self.dispatcher.emit(event);
        Ok(())
    }
}
```

### Pattern 3: Resource Allocation

Track task assignments:

```rust
impl SimulationHooks for MyHooks {
    fn on_tasks_assigned(
        &mut self,
        assignments: &[TaskAssignment],
    ) -> Result<()> {
        for assignment in assignments {
            let event = Event::new(Box::new(TaskAssignedEvent {
                task_id: assignment.task_id,
                resource_id: assignment.resource_id,
                timestamp: Utc::now(),
            }));
            self.dispatcher.emit(event);
        }
        Ok(())
    }
}
```

## Querying Events

### DataFusion Queries

Query Parquet files with SQL:

```rust
use datafusion::prelude::*;

let ctx = SessionContext::new();
let df = ctx.read_parquet("events.parquet", ParquetReadOptions::default()).await?;

// Filter by event type
let orders = df
    .filter(col("event_type").eq(lit("io.kitchen.order.created")))?
    .collect()
    .await?;

// Count by hour
let hourly_counts = df
    .aggregate(
        vec![date_trunc(lit("hour"), col("time"))],
        vec![count(col("id")).alias("count")],
    )?
    .collect()
    .await?;

// Join with agents
let df_agents = ctx.read_parquet("agents.parquet", ParquetReadOptions::default()).await?;
let joined = df
    .join(df_agents, JoinType::Inner, &["agent_id"], &["id"], None)?
    .collect()
    .await?;
```

### JSON Lines Processing

Process with jq or other tools:

```bash
# Count events by type
cat events.jsonl | jq -r '.type' | sort | uniq -c

# Extract all order events
cat events.jsonl | jq 'select(.type == "io.kitchen.order.created")'

# Time series analysis
cat events.jsonl | jq -r '[.time, .type] | @csv'
```

## Examples from Reference Implementations

### sim-kitchen Events

5 event types for order lifecycle:

```rust
// Order created
io.kitchen.order.created
{
  "order_id": "...",
  "customer_id": "...",
  "items": ["burger", "fries"]
}

// Order assigned to kitchen
io.kitchen.order.assigned
{
  "order_id": "...",
  "kitchen_id": "...",
  "assigned_at": "2026-03-16T10:00:00Z"
}

// Order completed
io.kitchen.order.completed
{
  "order_id": "...",
  "completed_at": "2026-03-16T10:05:00Z"
}

// Customer eating
io.kitchen.customer.eating
{
  "customer_id": "...",
  "order_id": "..."
}

// Customer done
io.kitchen.customer.done
{
  "customer_id": "...",
  "total_time_minutes": 15
}
```

**File:** `crates/sim-kitchen/src/events.rs`

### sim-logistics Events

Package delivery lifecycle:

```rust
// Package sorted
io.logistics.package.sorted
{
  "package_id": "...",
  "zone": "...",
  "lane": 3
}

// Package loaded
io.logistics.package.loaded
{
  "package_id": "...",
  "driver_id": "...",
  "truck_id": "..."
}

// Package delivered
io.logistics.package.delivered
{
  "package_id": "...",
  "delivered_at": "2026-03-16T10:30:00Z",
  "delivery_time_minutes": 45
}
```

**File:** `crates/sim-logistics/src/events.rs`

## Performance Considerations

### Buffering

Events are buffered in memory before flushing:

```rust
// Check buffer size
if dispatcher.buffered_count() > 10000 {
    let count = dispatcher.flush_to_parquet("events.parquet")?;
    println!("Flushed {} events", count);
}
```

### Batch Emission

Emit multiple events at once:

```rust
let events = vec![event1, event2, event3];
dispatcher.emit_batch(events);
```

### Parquet Compression

9x compression ratio with Snappy codec:

- Raw JSON: ~10MB for 10k events
- Parquet: ~1.1MB for 10k events

## Common Pitfalls

### Pitfall 1: Missing event_type

```rust
// ❌ Wrong - empty event type
impl EventPayload for MyEvent {
    fn event_type(&self) -> &str {
        ""  // Invalid!
    }
}

// ✅ Correct - descriptive event type
impl EventPayload for MyEvent {
    fn event_type(&self) -> &str {
        "io.myapp.entity.action"
    }
}
```

### Pitfall 2: Not Flushing Events

```rust
// ❌ Wrong - events never written
impl SimulationHooks for MyHooks {
    fn on_end(&mut self, ctx: &SimulationContext) -> Result<()> {
        // Forgot to flush!
        Ok(())
    }
}

// ✅ Correct - flush on end
impl SimulationHooks for MyHooks {
    fn on_end(&mut self, ctx: &SimulationContext) -> Result<()> {
        self.dispatcher.flush_to_parquet("events.parquet")?;
        Ok(())
    }
}
```

### Pitfall 3: UUID String Conversion

```rust
// ❌ Wrong - UUID bytes in JSON
json!({
    "agent_id": self.agent_id,  // Raw UUID, not serializable
})

// ✅ Correct - convert to string
json!({
    "agent_id": self.agent_id.to_string(),
})
```

## Testing Events

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = Event::new(Box::new(OrderCreatedEvent {
            order_id: TaskId::new(),
            customer_id: AgentId::new(),
            items: vec!["burger".to_string()],
            timestamp: Utc::now(),
        }));

        assert_eq!(event.event_type(), "io.kitchen.order.created");
        assert!(event.id().to_string().starts_with("019cf220"));
    }

    #[test]
    fn test_dispatcher_buffering() {
        let mut dispatcher = EventDispatcher::new();

        assert_eq!(dispatcher.buffered_count(), 0);

        dispatcher.emit(Event::new(Box::new(MyEvent { /* ... */ })));
        assert_eq!(dispatcher.buffered_count(), 1);

        dispatcher.emit(Event::new(Box::new(MyEvent { /* ... */ })));
        assert_eq!(dispatcher.buffered_count(), 2);

        dispatcher.clear();
        assert_eq!(dispatcher.buffered_count(), 0);
    }
}
```

## Related Documentation

- [Agents](agents.md) - Event generation from agent state
- [Workflows](workflows.md) - Workflow lifecycle events
- [Telemetry](telemetry.md) - Events to observability signals

## Summary

CloudEvents infrastructure provides:

- CloudEvents 1.0 specification compliance
- Parquet and JSON Lines export (9x compression)
- EventDispatcher with buffering
- DataFusion query support
- Type-safe event payloads
- Examples in sim-kitchen and sim-logistics

---

**Last Updated:** March 16, 2026
