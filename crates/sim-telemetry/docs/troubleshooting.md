# sim-telemetry Troubleshooting

## Common Issues

### Issue 1: High Memory Usage

**Cause**: Not flushing collector regularly.

**Solution**:
```rust
if collector.record_count() > 10000 {
    let records = collector.flush();
    export_batch(&records)?;
}
```

### Issue 2: Missing Telemetry Records

**Cause**: Generators not added before processing events.

**Solution**:
```rust
collector.add_generator(Box::new(EventToLogGenerator::new()));
// Must come AFTER adding generators
collector.process_event(&event)?;
```

### Issue 3: Performance Impact >5%

**Cause**: Too many generators or inefficient custom generator.

**Solution**: Profile and optimize:
```bash
cargo flamegraph --example telemetry_demo
```

## FAQ

### Q: How do I customize log levels?

**A**: Configure probabilities:
```rust
let generator = EventToLogGenerator::with_probabilities(
    0.05,  // 5% error
    0.10,  // 10% warn
);
```

### Q: Can I create custom generators?

**A**: Yes, implement `TelemetryGenerator`:
```rust
struct CustomGenerator;

impl TelemetryGenerator for CustomGenerator {
    fn generate(&mut self, event: &Value) -> Result<Vec<TelemetryRecord>, TelemetryError> {
        // Custom logic
        Ok(vec![])
    }
}
```

## Known Limitations

1. No automatic sampling (all events processed)
2. No streaming export (batch only)
3. No aggregation functions
4. Fixed OTLP endpoint configuration
