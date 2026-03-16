# sim-events Troubleshooting

## Common Issues

### Issue 1: Events Not Written to File

**Cause**: Forgot to call `flush_to_parquet()` or `flush_to_jsonl()`.

**Solution**:
```rust
impl SimulationHooks for MyHooks {
    fn on_end(&mut self, _ctx: &SimulationContext) -> Result<()> {
        self.dispatcher.flush_to_parquet("events.parquet")?;
        Ok(())
    }
}
```

### Issue 2: Memory Usage Growing

**Cause**: Events buffered indefinitely without flushing.

**Solution**: Flush periodically:
```rust
if ctx.step() % 1000 == 0 {
    dispatcher.flush_to_parquet(format!("events_{}.parquet", ctx.step()))?;
}
```

### Issue 3: CloudEvents Validation Errors

**Cause**: Missing required fields or invalid format.

**Solution**: Ensure EventPayload returns valid values:
```rust
impl EventPayload for MyEvent {
    fn event_type(&self) -> &str {
        "com.example.my.event"  // Must be non-empty
    }

    fn source(&self) -> String {
        format!("/entities/{}", self.id)  // Must be non-empty
    }
}
```

## FAQ

### Q: How do I query Parquet events?

**A**: Use DataFusion:
```rust
use datafusion::prelude::*;

let ctx = SessionContext::new();
let df = ctx.read_parquet("events.parquet", ParquetReadOptions::default()).await?;

// Filter by event type
let orders = df.filter(col("type").eq(lit("io.kitchen.order.created")))?;
let results = orders.collect().await?;
```

### Q: Can I add custom fields to events?

**A**: Yes, via JSON data payload:
```rust
fn to_json(&self) -> serde_json::Value {
    json!({
        "standard_field": self.value,
        "custom_field": self.custom,  // Add any fields
    })
}
```

### Q: Should I use Parquet or JSON Lines?

**A**:
- Parquet: Production, analytics, large volumes
- JSON Lines: Debugging, human-readable, small volumes

## Known Limitations

1. No event streaming (batch only)
2. No event replay functionality
3. No schema evolution support
4. Single-file output per flush
