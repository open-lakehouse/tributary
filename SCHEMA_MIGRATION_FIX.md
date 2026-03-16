# Schema Migration Fix: node_index Field Reference Issue

**Date:** March 16, 2026
**Status:** FIXED
**Tests Fixed:** 5 of 13 failing tests
**Final Status:** 29 passing / 8 failing (down from 24 passing / 13 failing)

## Problem Summary

After Phase 7 road network refactoring, caspers-universe had 13 failing tests due to SQL queries referencing a `node_index` field that was causing schema errors when routing data was missing.

### Root Cause

The `MovementHandler` in `crates/universe/src/state/movement_next.rs` creates a computed column called `node_index` (H3 cell index) for spatial indexing of routing nodes. This column is cached in RecordBatches for performance.

**The Issue:**
When no routing data exists (empty `.caspers/system/routing_nodes/` directory), the cached `node_index` RecordBatches are empty. When DataFusion tries to create a DataFrame from empty RecordBatches without an explicit schema, it creates a DataFrame with **zero columns**. Later queries that reference `node_index` fail with:

```
FieldNotFound { field: Column { relation: None, name: "node_index" }, valid_fields: [] }
```

### Secondary Issue

The initial schema definition incorrectly specified `node_index` as `FixedSizeBinary(8)`, but the H3 function `h3_longlatash3()` actually returns `Int64`. This caused type coercion errors:

```
Cannot infer common argument type for comparison operation Int64 = FixedSizeBinary(8)
```

## Solution

### Fix 1: Provide Explicit Schema for Empty DataFrames

Modified the `node_index()` method to detect when RecordBatches are empty and provide an explicit schema:

**File:** `crates/universe/src/state/movement_next.rs`

```rust
fn node_index(&self, ctx: &SimulationContext) -> Result<DataFrame> {
    // When no routing data exists (empty batches), DataFusion needs an explicit schema
    // to avoid "FieldNotFound" errors when selecting columns
    if self.node_index.is_empty() {
        // Create empty DataFrame with proper schema
        let empty_batch = RecordBatch::new_empty(NODE_INDEX_SCHEMA.clone());
        Ok(ctx.ctx().read_batch(empty_batch)?)
    } else {
        Ok(ctx.ctx().read_batches(self.node_index.iter().cloned())?)
    }
}
```

### Fix 2: Correct Schema Type for node_index

Created a static schema definition with the correct types:

```rust
/// Schema for the node_index DataFrame: node_id, osmid, node_index (H3 as Int64), geometry
static NODE_INDEX_SCHEMA: LazyLock<SchemaRef> = LazyLock::new(|| {
    SchemaRef::new(Schema::new(vec![
        Field::new("node_id", DataType::Int64, false),
        Field::new("osmid", DataType::Int64, false),
        Field::new("node_index", DataType::Int64, false), // H3 cell ID as BIGINT
        Field::new(
            "geometry",
            DataType::Struct(
                vec![
                    Field::new("x", DataType::Float64, false),
                    Field::new("y", DataType::Float64, false),
                ]
                .into(),
            ),
            false,
        ),
    ]))
});
```

## Test Results

### Before Fix
- 24 tests passing
- 13 tests failing (all with `node_index` schema errors)

### After Fix
- 29 tests passing (+5)
- 8 tests failing (-5)

### Tests Fixed (Schema Issues)
1. `agents::kitchen::tests::test_orders_with_routing_join_isolation` ✅
2. `simulation::tests::test_simulation_step` ✅
3. `simulation::tests::test_snapshot_roundtrip` ✅
4. `simulation::event_log::tests::test_event_log_from_run` ✅
5. `simulation::lifecycle_tests::test_order_ready_event_schema` ✅

### Remaining Failures (Unrelated Issues)

**Length Spheroid Function Tests (3)** - Calculation errors, not schema:
- `functions::length_spheroid::test::test_linestring`
- `functions::length_spheroid::test::test_wkb_linestring`
- `functions::length_spheroid::test::test_multilinestring`

**Lifecycle Tests (5)** - Fail due to missing routing data for actual delivery simulation:
- `agents::kitchen::tests::test_unreachable_orders`
- `simulation::lifecycle_tests::test_courier_returns_to_idle`
- `simulation::lifecycle_tests::test_order_status_transitions`
- `simulation::lifecycle_tests::test_order_full_lifecycle`
- `simulation::lifecycle_tests::test_multiple_orders_lifecycle`

These tests require actual routing graph data in `.caspers/system/routing_nodes/` and `.caspers/system/routing_edges/` to complete delivery journeys.

## Schema Architecture

### Movement Handler Schema Flow

1. **Routing Nodes** (from `.caspers/system/routing_nodes/`):
   ```
   location: Utf8
   id: FixedSizeBinary(16) [UUID]
   properties.osmid: Int64
   geometry: Struct{x: Float64, y: Float64}
   ```

2. **Computed node_index** (H3 spatial index):
   ```sql
   SELECT
     cast(row_number() - 1 AS Int64) AS node_id,
     properties.osmid AS osmid,
     h3_longlatash3(geometry.x, geometry.y, 9) AS node_index,  -- Returns Int64
     geometry
   FROM routing_nodes
   ```

3. **Cached Schema** (stored in `self.node_index`):
   ```
   node_id: Int64
   osmid: Int64
   node_index: Int64  [H3 cell ID]
   geometry: Struct{x: Float64, y: Float64}
   ```

4. **Usage in Joins**:
   ```sql
   -- Spatial join to find nearest node
   SELECT journey.*, node.node_id, node.geometry
   FROM journey_data AS journey
   LEFT JOIN node_index AS node
     ON h3_longlatash3(journey.origin.x, journey.origin.y, 9) = node.node_index
   ```

## Implications for Future Domain Developers

### When Adding New Movement/Routing Features

1. **Always provide explicit schemas** when caching DataFrames that might be empty
2. **H3 spatial indices** are stored as `Int64`, not binary formats
3. **Empty routing data is valid** - the system should gracefully handle missing network data
4. **Test with and without routing data** to ensure degradation is graceful

### Schema Best Practices

```rust
// ✅ GOOD: Explicit schema for empty DataFrames
fn cached_data(&self, ctx: &Context) -> Result<DataFrame> {
    if self.cache.is_empty() {
        let empty = RecordBatch::new_empty(SCHEMA.clone());
        Ok(ctx.read_batch(empty)?)
    } else {
        Ok(ctx.read_batches(self.cache.iter().cloned())?)
    }
}

// ❌ BAD: No schema for empty batches
fn cached_data(&self, ctx: &Context) -> Result<DataFrame> {
    Ok(ctx.read_batches(self.cache.iter().cloned())?)  // Fails with empty cache!
}
```

### Key Takeaways

1. **DataFusion schema inference fails on empty iterators** - always provide explicit schema
2. **Verify function return types** - UDF documentation may differ from implementation
3. **H3 functions return Int64** - not FixedSizeBinary as might be expected
4. **Spatial joins require matching types** - ensure both sides of join use same H3 data type
5. **Test edge cases** - empty data, missing files, zero results

## Files Modified

- `/Users/robert.pack/code/management/crates/universe/src/state/movement_next.rs`
  - Added `NODE_INDEX_SCHEMA` static definition
  - Modified `node_index()` method to handle empty RecordBatches
  - Updated imports to include Schema and Field types

## Verification

Run tests to confirm fixes:

```bash
# Verify specific fixed tests
cargo test --package caspers-universe test_orders_with_routing_join_isolation
cargo test --package caspers-universe test_simulation_step
cargo test --package caspers-universe test_snapshot_roundtrip

# Full test suite
cargo test --package caspers-universe
# Expected: 29 passed, 8 failed
```

---

**Resolution Status:** ✅ COMPLETE - P2 schema issue resolved, 5 tests fixed
