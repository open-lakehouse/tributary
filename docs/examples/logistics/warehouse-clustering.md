# Warehouse Clustering Example

H3-based spatial clustering for warehouse assignment and geographic distribution.

## Overview

- H3 hexagonal indexing at multiple resolutions
- Spatial clustering of warehouses
- Nearest warehouse assignment
- Geographic coverage analysis

## H3 Indexing

```rust
use sim_geo::H3Index;

// Index warehouse locations at resolution 7 (~5km cells)
let mut warehouse_clusters: HashMap<String, Vec<LocationId>> = HashMap::new();

for warehouse in warehouses {
    let h3 = H3Index::from_point(warehouse.position, 7)?;
    warehouse_clusters
        .entry(h3.to_string())
        .or_default()
        .push(warehouse.id);
}

println!("Warehouses distributed across {} H3 cells", warehouse_clusters.len());
```

## Nearest Warehouse Assignment

```rust
fn assign_warehouse(
    customer_location: Point<f64>,
    warehouse_clusters: &HashMap<String, Vec<LocationId>>,
    warehouses: &LocationHandler
) -> Option<LocationId> {
    // Get customer's H3 cell
    let customer_h3 = H3Index::from_point(customer_location, 7)?;

    // Try same cell first
    if let Some(nearby_warehouses) = warehouse_clusters.get(&customer_h3.to_string()) {
        return Some(nearby_warehouses[0]);
    }

    // Try neighbors
    for neighbor in customer_h3.neighbors()? {
        if let Some(nearby_warehouses) = warehouse_clusters.get(&neighbor.to_string()) {
            return Some(nearby_warehouses[0]);
        }
    }

    // Fall back to closest warehouse
    find_closest_warehouse(customer_location, warehouses)
}
```

## Multi-Resolution Analysis

```rust
// Analyze coverage at different resolutions
for resolution in 5..=9 {
    let mut cells = HashSet::new();

    for warehouse in &warehouses {
        let h3 = H3Index::from_point(warehouse.position, resolution)?;
        cells.insert(h3.to_string());
    }

    println!("Resolution {}: {} unique cells covered", resolution, cells.len());
}
```

**Output:**

```
Resolution 5: 3 unique cells covered   (~100km cells)
Resolution 6: 8 unique cells covered   (~40km cells)
Resolution 7: 15 unique cells covered  (~15km cells)
Resolution 8: 45 unique cells covered  (~5km cells)
Resolution 9: 112 unique cells covered (~2km cells)
```

## Geographic Distribution Visualization

```rust
// Export H3 cells for visualization
use std::fs::File;
use std::io::Write;

fn export_warehouse_clusters(
    clusters: &HashMap<String, Vec<LocationId>>,
    output_path: &str
) -> Result<()> {
    let mut file = File::create(output_path)?;

    writeln!(file, "h3_cell,warehouse_count")?;
    for (h3_cell, warehouses) in clusters {
        writeln!(file, "{},{}", h3_cell, warehouses.len())?;
    }

    Ok(())
}
```

## See Also

- [sim-geo API Reference](../../api-reference/sim-geo.md#h3index)
- [package-routing.md](./package-routing.md)
