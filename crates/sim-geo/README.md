# sim-geo

Geospatial and movement abstractions for agent-based simulations.

## Overview

`sim-geo` provides geospatial capabilities for `sim-core` including location management, spatial indexing, and journey tracking. It uses Uber's H3 hexagonal grid system for efficient proximity queries.

## Features

- **Location Management** - Generic location handling with H3 indexing
- **Spatial Queries** - Fast nearest-neighbor and radius searches
- **Journey Tracking** - Movement progression with pluggable velocity models
- **H3 Integration** - Hierarchical hexagonal grid for spatial indexing

## Location Management

Define locations by implementing the `Location` trait:

```rust
use sim_geo::{Location, LocationId};
use geo_types::Point;

struct Warehouse {
    id: LocationId,
    position: Point<f64>,
    capacity: usize,
}

impl Location for Warehouse {
    fn id(&self) -> LocationId { self.id }
    fn location_type(&self) -> &str { "warehouse" }
    fn position(&self) -> Point<f64> { self.position }
    fn capacity(&self) -> Option<usize> { Some(self.capacity) }
}
```

Use `LocationHandler` for spatial operations:

```rust
use sim_geo::LocationHandler;
use h3o::Resolution;

let mut handler = LocationHandler::new(Resolution::Nine);
handler.add_location(warehouse)?;

// Find nearest location
let nearest_id = handler.nearest(my_position)?;

// Find all within 5km
let nearby = handler.within_radius(my_position, 5000.0)?;
```

## Journey Tracking

Track agent movement with journey states:

```rust
use sim_geo::{JourneyState, MovementHandler, ConstantVelocity};
use chrono::Duration;

let mut movement = MovementHandler::new()
    .with_velocity_model(Box::new(ConstantVelocity::new(5.0))); // 5 m/s

// Add journey
movement.add_journey(journey_state);

// Advance by time step
movement.advance_journeys(Duration::seconds(60))?;

// Get completed journeys
let completed = movement.get_completed_journeys();
```

## H3 Spatial Index

Direct use of the H3 index for custom scenarios:

```rust
use sim_geo::H3Index;

let mut index = H3Index::<LocationId>::new();
index.insert(h3_cell, location_id);

let nearby_ids = index.nearby(search_point, resolution)?;
```

## H3 Resolutions

Choose resolution based on your use case:

| Resolution | Cell Area | Use Case |
|-----------|-----------|----------|
| 5 | ~252 km² | City-level |
| 7 | ~5.16 km² | Neighborhood |
| 9 | ~0.105 km² | Block-level (default) |
| 11 | ~4,250 m² | Building-level |

## Testing

```bash
cargo test -p sim-geo
```

All 13 tests pass with coverage of locations, spatial queries, and journeys.

## License

See the workspace LICENSE file.
