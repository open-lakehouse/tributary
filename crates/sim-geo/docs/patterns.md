# sim-geo Usage Patterns

## Pattern 1: Basic Location Management

```rust
use sim_geo::{Location, LocationHandler, LocationId};
use geo_types::Point;
use h3o::Resolution;

#[derive(Debug)]
struct Store {
    id: LocationId,
    position: Point,
    capacity: usize,
}

impl Location for Store {
    fn id(&self) -> LocationId { self.id }
    fn location_type(&self) -> &str { "store" }
    fn position(&self) -> Point { self.position }
    fn capacity(&self) -> Option<usize> { Some(self.capacity) }
}

fn main() -> sim_geo::Result<()> {
    let mut handler = LocationHandler::new(Resolution::Nine);

    // Add locations
    handler.add_location(Store {
        id: LocationId::new(),
        position: Point::new(-122.4194, 37.7749),
        capacity: 100,
    })?;

    // Find nearest
    let query_point = Point::new(-122.42, 37.77);
    let nearest_id = handler.nearest(query_point)?;

    // Find within radius (5km)
    let nearby = handler.within_radius(query_point, 5000.0)?;
    println!("Found {} locations within 5km", nearby.len());

    Ok(())
}
```

## Pattern 2: Road Network Routing

```rust
use sim_geo::{RoadNetwork, NodeId};
use geo_types::Coord;

fn main() -> sim_geo::Result<()> {
    // Load from OSM PBF
    let network = RoadNetwork::from_osm_pbf("city.osm.pbf")?;

    // Prepare for routing (Contraction Hierarchies)
    let prepared = network.prepare_routing()?;

    // Find route
    let origin = prepared.nearest_node(Coord::from((-122.4194, 37.7749)))?;
    let dest = prepared.nearest_node(Coord::from((-122.4089, 37.7858)))?;

    let route = prepared.route(origin, dest)?;

    println!("Route distance: {:.2} meters", route.total_distance);
    println!("Waypoints: {}", route.path.len());

    Ok(())
}
```

## Pattern 3: Journey Tracking with Time-of-Day Speed

```rust
use sim_geo::{MovementHandler, Journey, TimeOfDayVelocity};
use geo_types::Point;
use chrono::{Duration, NaiveTime};
use std::collections::HashMap;

fn main() -> sim_geo::Result<()> {
    // Configure time-of-day speed patterns
    let mut peak_hours = HashMap::new();
    peak_hours.insert(NaiveTime::from_hms_opt(8, 0, 0).unwrap(), 0.6);   // Morning rush: 40% slower
    peak_hours.insert(NaiveTime::from_hms_opt(17, 0, 0).unwrap(), 0.5);  // Evening rush: 50% slower

    let velocity_model = TimeOfDayVelocity::new(
        13.89,  // 50 km/h base speed
        peak_hours,
    );

    let mut movement = MovementHandler::new()
        .with_velocity_model(Box::new(velocity_model));

    // Add journey
    let journey = Journey {
        agent_id: AgentId::new(),
        origin: Point::new(-122.4194, 37.7749),
        destination: Point::new(-122.4089, 37.7858),
        waypoints: vec![],  // Can add route waypoints
        ..Default::default()
    };

    movement.start_journey(journey)?;

    // Advance by 1 minute
    movement.advance_journeys(now, Duration::minutes(1))?;

    // Check progress
    for journey in movement.active_journeys() {
        println!("Progress: {:.1}%",
            100.0 * journey.distance_covered / journey.total_distance
        );
    }

    Ok(())
}
```

## Best Practices

### Choose Appropriate H3 Resolution
- Urban simulations: Resolution 9 (105m² cells)
- Regional simulations: Resolution 7 (5km² cells)
- High-precision: Resolution 11 (4250m² cells)

### Prepare Routing Once
```rust
// Do this once at startup
let prepared = network.prepare_routing()?;

// Reuse for all queries
for (origin, dest) in route_requests {
    let route = prepared.route(origin, dest)?;
}
```

### Batch Journey Updates
```rust
// Do this: Single batch update
movement.advance_journeys(now, time_step)?;

// Not this: Multiple calls per step
for journey in journeys {
    journey.advance(time_step)?;  // Inefficient
}
```
