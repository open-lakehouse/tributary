# sim-logistics Troubleshooting

## Common Issues

### Issue 1: Packages Stuck in Sorted State

**Cause**: No loading docks or drivers available.

**Solution**:
```rust
// Add more loading capacity
sim.add_loading_dock(LoadingDock {
    id: ResourceId::new(),
    capacity: 5,
})?;

// Or add more drivers
sim.add_driver(Driver::new(vehicle_capacity))?;
```

### Issue 2: Low Driver Utilization

**Cause**: Packages not reaching InTransit state fast enough.

**Solution**: Increase sorting and loading throughput:
```rust
let sorting_station = SortingStation {
    id: ResourceId::new(),
    capacity: 20,  // Increase from 10
};
```

### Issue 3: Delivery Times Too Long

**Cause**: Inefficient routing or congestion.

**Solution**: Use real road network with routing:
```rust
let network = RoadNetwork::from_osm_pbf("city.osm.pbf")?;
let prepared = network.prepare_routing()?;

// Use CH routing for fast queries
let route = prepared.route(origin, dest)?;
```

## FAQ

### Q: How do I model same-day delivery?

**A**: Use priority and constrain workflow timing:
```rust
let package = Package::new_with_deadline(
    origin,
    destination,
    PackagePriority::Overnight,
    Duration::hours(12),  // Must deliver within 12 hours
);
```

### Q: Can I model multiple delivery vehicles?

**A**: Yes, create multiple drivers:
```rust
for i in 0..10 {
    sim.add_driver(Driver::new_with_capacity(20))?;  // 20 packages per vehicle
}
```

### Q: How do I track package events?

**A**: Use the event dispatcher:
```rust
for event in sim.get_events() {
    if event.r#type == "io.logistics.package.delivered" {
        println!("Package delivered: {:?}", event.data);
    }
}
```

## Known Limitations

1. No multi-modal transport (truck + air + rail)
2. No real-time routing updates
3. No dynamic pricing or cost optimization
4. No package damage or loss simulation
