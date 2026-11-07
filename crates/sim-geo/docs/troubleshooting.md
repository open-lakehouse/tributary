# sim-geo Troubleshooting

## Common Issues

### Issue 1: Routing Queries Return None

**Symptom**: `route()` returns `None` instead of a path.

**Causes**:
- Nodes are in disconnected components
- Road network has no path between nodes
- One-way restrictions prevent path

**Solution**:
```rust
match prepared.route(origin, dest) {
    Some(route) => println!("Found route: {:.1}m", route.total_distance),
    None => {
        // Check if nodes are in same connected component
        let origin_component = prepared.component_id(origin);
        let dest_component = prepared.component_id(dest);

        if origin_component != dest_component {
            println!("Nodes are in different connected components!");
            // Find nearest node in same component as destination
            let alternate = prepared.nearest_in_component(origin_point, dest_component)?;
            let route = prepared.route(alternate, dest)?;
        }
    }
}
```

### Issue 2: Slow Routing Performance

**Symptom**: Routing queries take >100ms.

**Solution**: Ensure Contraction Hierarchies is prepared:
```rust
// Wrong: Using unprepared network
let route = network.route_dijkstra(origin, dest)?;  // Slow!

// Right: Use prepared CH network
let prepared = network.prepare_routing()?;
let route = prepared.route(origin, dest)?;  // Fast (<1ms)
```

### Issue 3: H3 Proximity Queries Return Empty

**Symptom**: `within_radius()` returns no results when locations exist.

**Cause**: Resolution mismatch or cell calculation error.

**Solution**:
```rust
let handler = LocationHandler::new(Resolution::Nine);

// Add locations
handler.add_location(my_location)?;

// Query with same resolution
let nearby = handler.within_radius(point, 5000.0)?;

// Debug: Check H3 cells
let cell = h3o::LatLng::from_degrees(point.y(), point.x())
    .unwrap()
    .to_cell(Resolution::Nine);
println!("Query cell: {:?}", cell);

for loc in handler.all_locations() {
    let loc_cell = h3o::LatLng::from_degrees(loc.position().y(), loc.position().x())
        .unwrap()
        .to_cell(Resolution::Nine);
    println!("Location {} cell: {:?}", loc.id(), loc_cell);
}
```

### Issue 4: Journeys Never Complete

**Symptom**: Journeys stuck in `InProgress` state.

**Causes**:
- Velocity model returns 0 or NaN speed
- Distance calculation error
- `advance_journeys()` not called

**Solution**:
```rust
// Verify velocity model
let velocity = movement.velocity_model();
let speed = velocity.speed(Utc::now(), RoadType::Residential);
assert!(speed > 0.0 && speed.is_finite(), "Invalid speed: {}", speed);

// Ensure advance is called every tick
impl SimulationHooks for MyHooks {
    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        self.movement.advance_journeys(ctx.current_time(), ctx.time_step())?;

        // Check for stuck journeys
        for journey in self.movement.active_journeys() {
            if ctx.step() - journey.start_step > 10000 {
                eprintln!("Journey {} stuck for {} steps",
                    journey.agent_id, ctx.step() - journey.start_step);
            }
        }

        Ok(())
    }
}
```

## FAQ

### Q: How do I load a real road network?

**A**: Download OSM data and parse:
```bash
# Download OSM extract (e.g., from Geofabrik)
wget https://download.geofabrik.de/...city.osm.pbf

# In code:
let network = RoadNetwork::from_osm_pbf("city.osm.pbf")?;
network.save_to_parquet("network.parquet")?;

// Later: load from Parquet (faster)
let network = RoadNetwork::load_from_parquet("network.parquet")?;
```

### Q: Can I mix different H3 resolutions?

**A**: No, LocationHandler uses single resolution. For multi-resolution:
```rust
let coarse = LocationHandler::new(Resolution::Seven);
let fine = LocationHandler::new(Resolution::Eleven);

// Use appropriate handler for each query type
```

### Q: How do I model different vehicle types?

**A**: Use custom VelocityModel:
```rust
struct VehicleVelocity {
    vehicle_type: VehicleType,
}

impl VelocityModel for VehicleVelocity {
    fn speed(&self, time: DateTime<Utc>, road_type: RoadType) -> f64 {
        let base = match road_type {
            RoadType::Motorway => 30.0,
            RoadType::Residential => 8.0,
            _ => 15.0,
        };

        match self.vehicle_type {
            VehicleType::Car => base,
            VehicleType::Bicycle => base * 0.3,
            VehicleType::Truck => base * 0.7,
        }
    }
}
```

## Known Limitations

1. **No turn restrictions**: Routing doesn't model turn prohibitions
2. **No multi-modal**: Can't combine road + rail + walking
3. **Static networks**: Edge weights don't update during simulation
4. **2D only**: No elevation or 3D routing

## Performance Tips

- Use CH for networks >10k nodes
- Choose H3 resolution wisely (higher = more memory)
- Batch journey updates instead of individual calls
- Cache prepared routing networks
