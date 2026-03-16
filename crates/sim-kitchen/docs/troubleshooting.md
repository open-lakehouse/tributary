# sim-kitchen Troubleshooting

## Common Issues

### Issue 1: Orders Not Getting Assigned

**Cause**: All kitchens at capacity.

**Solution**:
```rust
// Increase kitchen capacity
let mut sim = KitchenSimulation::new(3, 10, true);  // 10 capacity instead of 5

// Or add more kitchens
let mut sim = KitchenSimulation::new(5, 5, true);  // 5 kitchens instead of 3
```

### Issue 2: Customers Stuck in WaitingForFood

**Cause**: Orders not completing (no task completion logic).

**Solution**: Ensure workflow engine is advancing:
```rust
impl SimulationHooks for KitchenHooks {
    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        // Complete orders that have finished
        let now = ctx.current_time();
        for assignment in self.allocator.assigned_tasks() {
            if assignment.finish_time <= now {
                self.allocator.complete_task(assignment.task_id)?;
                // Notify customer order is ready
            }
        }
        Ok(())
    }
}
```

### Issue 3: No Events Generated

**Cause**: EventDispatcher not flushed.

**Solution**:
```rust
runner.run_for_steps(100)?;

// Access hooks to flush events
if let Some(hooks) = runner.hooks_mut().downcast_mut::<KitchenSimulationHooks>() {
    hooks.dispatcher.flush_to_parquet("events.parquet")?;
}
```

## FAQ

### Q: How do I adjust order probability?

**A**: Configure when creating customers:
```rust
let customer = Customer::new_with_probability(0.5);  // 50% chance per tick
```

### Q: Can I model delivery times?

**A**: Yes, use courier delivery workflow:
```rust
let sim = KitchenSimulation::with_workflows(
    num_kitchens,
    kitchen_capacity,
    false,  // No supply chain
    true,   // Enable delivery
);
```

### Q: How do I track inventory?

**A**: Use the InventoryManager:
```rust
let mut inventory = InventoryManager::new();
inventory.add_inventory(kitchen_id, initial_stock)?;

// Deplete during order preparation
inventory.consume(kitchen_id, ingredient, amount)?;
```

## Known Limitations

1. No dynamic pricing
2. No customer preferences/loyalty
3. Fixed kitchen locations
4. No peak-hour staffing adjustments
