# Decision: SQL-Based Task Allocation (Not Implemented)

**Date**: 2026-03-16
**Status**: Evaluated and Rejected
**Decision**: Keep current HashMap-based allocation approach

## Context

During Phase 7+8 development, we evaluated implementing a SQL-based task allocation method using DataFusion as an alternative to the current HashMap-based approach in `ResourceAllocator`.

## Analysis

### Current Approach (HashMap-based)
- **Complexity**: O(1) lookups via HashMap indices + O(n) iteration
- **Performance**: < 1ms for thousands of resources
- **Benefits**:
  - Simple, maintainable code
  - Efficient columnar access via RecordBatch
  - Well-tested and proven in production

### SQL Approach (DataFusion)
- **Complexity**: Query parsing + execution + result conversion
- **Overhead**: Query optimizer, execution engine, RecordBatch → Assignment conversion
- **Benefits**:
  - Better for complex multi-constraint queries
  - Natural for analytical workloads
  - Good for external data integration

## Decision Rationale

**We chose to keep the HashMap approach** for the following reasons:

1. **Current strategies are simple filters**: LeastLoaded and FirstAvailable don't benefit from SQL query optimization
   - Just type matching + availability check
   - Direct iteration is faster for these simple cases

2. **No performance benefit**: DataFusion overhead (query parsing, planning, execution) exceeds any potential gains for simple allocation

3. **Code complexity**: SQL approach would add:
   - Strategy-to-SQL translation layer
   - RecordBatch to Assignment conversion
   - More error handling
   - Harder to debug allocation decisions

4. **Already evaluated**: See `allocator.rs` lines 707-714 for detailed analysis

## When to Revisit

Consider SQL-based allocation if we need:

1. **Multi-constraint allocation**:
   ```sql
   WHERE resource_type = 'truck'
     AND location IN (nearby_locations)
     AND cost < budget
     AND skill_level >= required_skill
   ```

2. **Complex scoring functions**:
   ```sql
   ORDER BY (
     location_distance * 0.3 +
     cost_score * 0.2 +
     availability_score * 0.5
   )
   ```

3. **External data integration**: Join with historical data, weather data, traffic data

4. **Analytical queries**: "What's the average allocation time by resource type?"

## Implementation Notes

If we do implement SQL allocation in the future:

### Strategy-Specific SQL

```rust
fn generate_allocation_sql(strategy: &str) -> &str {
    match strategy {
        "LeastLoaded" => "
            SELECT t.id as task_id, r.id as resource_id
            FROM tasks t
            JOIN resources r ON t.required_resource_type = r.resource_type
            WHERE t.status = 'pending' AND r.available > 0
            ORDER BY r.current_load ASC
            LIMIT 100
        ",
        "FirstAvailable" => "
            SELECT t.id as task_id, r.id as resource_id
            FROM tasks t
            JOIN resources r ON t.required_resource_type = r.resource_type
            WHERE t.status = 'pending' AND r.available > 0
            LIMIT 100
        ",
        _ => panic!("Unknown strategy"),
    }
}
```

### RecordBatch Conversion

```rust
fn batch_to_assignments(batch: &RecordBatch) -> Result<Vec<Assignment>> {
    let task_id_array = batch.column(0).as_any().downcast_ref::<BinaryArray>()?;
    let resource_id_array = batch.column(1).as_any().downcast_ref::<BinaryArray>()?;

    (0..batch.num_rows())
        .map(|i| {
            Ok(Assignment {
                task_id: TaskId::from_bytes(task_id_array.value(i))?,
                resource_id: ResourceId::from_bytes(resource_id_array.value(i))?,
            })
        })
        .collect()
}
```

### Benchmarking Template

If implementing, benchmark against current approach:

```rust
fn bench_allocation_hashmap(c: &mut Criterion) {
    // Measure current assign_tasks() performance
}

fn bench_allocation_sql(c: &mut Criterion) {
    // Measure assign_tasks_sql() performance
}

// Expected result: HashMap faster for simple strategies
// Only implement SQL if it shows 2x+ improvement for complex strategies
```

## References

- `crates/sim-core/src/resources/allocator.rs` (lines 696-731): Current implementation
- `crates/sim-core/src/resources/allocator.rs` (lines 707-714): Evaluation notes
- Phase 7+8 Stage 4 requirements: SQL allocation (optional)

## Conclusion

**The HashMap approach is the right solution for our current allocation strategies.** SQL would add complexity without providing measurable benefits. We should only revisit this if we introduce complex multi-constraint allocation strategies that truly require SQL's expressiveness and optimization capabilities.
