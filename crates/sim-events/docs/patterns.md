# sim-events Usage Patterns

## Pattern 1: Define Custom Event Payloads

```rust
use sim_events::EventPayload;
use serde_json::json;

#[derive(Debug)]
struct OrderCreatedEvent {
    order_id: String,
    customer_id: String,
    items: Vec<String>,
    total: f64,
}

impl EventPayload for OrderCreatedEvent {
    fn event_type(&self) -> &str {
        "io.kitchen.order.created"
    }

    fn source(&self) -> String {
        format!("/orders/{}", self.order_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.customer_id.clone())
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "order_id": self.order_id,
            "customer_id": self.customer_id,
            "items": self.items,
            "total": self.total,
        })
    }
}
```

## Pattern 2: Emit Events from Simulation Hooks

```rust
use sim_core::{SimulationHooks, SimulationContext, AgentTickResult};
use sim_events::{Event, EventDispatcher};

struct EventEmittingHooks {
    dispatcher: EventDispatcher,
}

impl SimulationHooks for EventEmittingHooks {
    fn on_agents_ticked(
        &mut self,
        ctx: &SimulationContext,
        results: &[AgentTickResult],
    ) -> sim_core::Result<()> {
        for result in results {
            for demand in &result.demands {
                let event = Event::new(Box::new(OrderCreatedEvent {
                    order_id: demand.id.to_string(),
                    customer_id: result.agent_id.to_string(),
                    items: vec!["burger".to_string()],
                    total: 15.99,
                }));
                self.dispatcher.emit(event);
            }
        }
        Ok(())
    }

    fn on_end(&mut self, _ctx: &SimulationContext) -> sim_core::Result<()> {
        let count = self.dispatcher.flush_to_parquet("events.parquet")?;
        println!("Wrote {} events", count);
        Ok(())
    }
}
```

## Pattern 3: Periodic Event Flushing

```rust
impl SimulationHooks for EventEmittingHooks {
    fn on_after_tick(&mut self, ctx: &SimulationContext) -> sim_core::Result<()> {
        // Flush every 1000 steps
        if ctx.step() % 1000 == 0 {
            let count = self.dispatcher.flush_to_parquet(
                format!("events_step_{}.parquet", ctx.step())
            )?;
            println!("[{}] Flushed {} events", ctx.step(), count);
        }
        Ok(())
    }
}
```

## Best Practices

1. **Use reverse-DNS event types**: `io.kitchen.order.created`
2. **Flush events periodically** to avoid unbounded memory growth
3. **Include relevant IDs in source/subject** for filtering
4. **Keep payloads focused** on essential data
5. **Use Parquet for analysis**, JSON Lines for debugging
