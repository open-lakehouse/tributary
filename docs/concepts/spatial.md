# Spatial Operations

**Last Updated:** March 16, 2026

## Overview

Tributary provides comprehensive geospatial capabilities through the sim-geo crate. It includes location management with H3 hexagonal indexing, journey tracking with pluggable velocity models, and road network routing with fast_paths for realistic navigation.

## Objective

Learn how to:
- Use H3 spatial indexing for location management
- Track agent movement with journey states
- Implement road network routing
- Configure velocity models (Constant, TimeOfDay)
- Perform spatial queries (nearest-neighbor, radius)

## Prerequisites

- [Agents](agents.md) understanding
- [Getting Started](../GETTING_STARTED.md)
- Basic geospatial concepts (lat/lon, distance)

## H3 Spatial Indexing

### H3 Resolution Levels

Choose resolution based on your use case:

| Resolution | Cell Area | Use Case |
|-----------|-----------|----------|
| 5 | ~252 km² | City-level operations |
| 7 | ~5.16 km² | Neighborhood zones |
| 9 | ~0.105 km² | Block-level (default) |
| 11 | ~4,250 m² | Building-level precision |
| 13 | ~43 m² | Precise location tracking |

### LocationHandler

Manage locations with H3 spatial indexing:

```rust
use sim_geo::{LocationHandler, Location, LocationId};
use geo::Point;
use h3o::Resolution;

// Create handler with resolution 9 (block-level)
let mut handler = LocationHandler::new(Resolution::Nine);

// Define location
struct Restaurant {
    id: LocationId,
    position: Point<f64>,
    capacity: usize,
}

impl Location for Restaurant {
    fn id(&self) -> LocationId { self.id }
    fn location_type(&self) -> &str { "restaurant" }
    fn position(&self) -> Point<f64> { self.position }
    fn capacity(&self) -> Option<usize> { Some(self.capacity) }
}

// Add locations
let restaurant = Restaurant {
    id: LocationId::new(),
    position: Point::new(-122.4194, 37.7749), // San Francisco
    capacity: 50,
};

handler.add_location(Box::new(restaurant))?;

// Spatial queries
let my_position = Point::new(-122.4183, 37.7750);

// Find nearest location
let nearest_id = handler.nearest(my_position)?;

// Find all within 1km radius
let nearby = handler.within_radius(my_position, 1000.0)?;
println!("Found {} locations within 1km", nearby.len());

// Get location by ID
if let Some(location) = handler.get_location(nearest_id) {
    println!("Nearest: {} at {:?}", location.location_type(), location.position());
}
```

### H3 Cell Operations

Direct H3 operations:

```rust
use h3o::{CellIndex, LatLng, Resolution};

// Convert point to H3 cell
let point = LatLng::new(37.7749, -122.4194)?;
let cell = point.to_cell(Resolution::Nine);

// Get cell neighbors
let neighbors = cell.grid_disk::<Vec<_>>(1);  // 1-ring neighbors

// Get cell area
let area_km2 = cell.area_km2();

// Cell to lat/lng
let center = cell.to_lat_lng();
```

## Movement and Journey Tracking

### JourneyState

Track agent movement:

```rust
use sim_geo::{JourneyState, MovementHandler};
use chrono::{Utc, Duration};

let journey = JourneyState {
    agent_id: AgentId::new(),
    origin: Point::new(-122.4194, 37.7749),
    destination: Point::new(-122.4083, 37.7849),
    started_at: Utc::now(),
    progress: 0.0,  // 0.0 = origin, 1.0 = destination
};
```

### MovementHandler

Manage all agent journeys:

```rust
use sim_geo::{MovementHandler, ConstantVelocity};

// Create handler with constant velocity (5 m/s = 18 km/h)
let mut movement = MovementHandler::new()
    .with_velocity_model(Box::new(ConstantVelocity::new(5.0)));

// Add journey
movement.add_journey(journey);

// Advance all journeys by time step
movement.advance_journeys(Duration::seconds(30))?;

// Get completed journeys
let completed = movement.get_completed_journeys();
for journey in completed {
    println!("Agent {} arrived at destination", journey.agent_id);
}

// Check journey progress
if let Some(journey) = movement.get_journey(agent_id) {
    println!("Progress: {:.1}%", journey.progress * 100.0);
    println!("Current position: {:?}", journey.current_position());
}
```

## Velocity Models

### ConstantVelocity

Fixed speed:

```rust
use sim_geo::ConstantVelocity;

// 5 m/s (18 km/h, walking speed)
let model = ConstantVelocity::new(5.0);

// 13.9 m/s (50 km/h, city driving)
let model = ConstantVelocity::new(13.9);
```

### TimeOfDayVelocity

Variable speed based on time:

```rust
use sim_geo::{TimeOfDayVelocity, VelocityPeriod};

let model = TimeOfDayVelocity::new(vec![
    VelocityPeriod {
        start_hour: 0,
        end_hour: 7,
        velocity_mps: 16.7,  // 60 km/h (night, light traffic)
    },
    VelocityPeriod {
        start_hour: 7,
        end_hour: 9,
        velocity_mps: 8.3,   // 30 km/h (morning rush)
    },
    VelocityPeriod {
        start_hour: 9,
        end_hour: 17,
        velocity_mps: 13.9,  // 50 km/h (daytime)
    },
    VelocityPeriod {
        start_hour: 17,
        end_hour: 19,
        velocity_mps: 8.3,   // 30 km/h (evening rush)
    },
    VelocityPeriod {
        start_hour: 19,
        end_hour: 24,
        velocity_mps: 16.7,  // 60 km/h (night)
    },
]);

let mut movement = MovementHandler::new()
    .with_velocity_model(Box::new(model));
```

### Custom Velocity Models

```rust
use sim_geo::{VelocityModel, JourneyState};
use chrono::Duration;

pub struct WeatherAwareVelocity {
    base_velocity: f64,
    rain_factor: f64,
}

impl VelocityModel for WeatherAwareVelocity {
    fn calculate_progress(
        &self,
        journey: &JourneyState,
        time_delta: Duration,
    ) -> f64 {
        let distance = journey.distance_meters();
        let velocity = if self.is_raining() {
            self.base_velocity * self.rain_factor  // Slower in rain
        } else {
            self.base_velocity
        };

        let distance_traveled = velocity * time_delta.num_seconds() as f64;
        (journey.progress + (distance_traveled / distance)).min(1.0)
    }
}
```

## Road Network Routing

### Graph Structure

Build road networks:

```rust
use sim_geo::routing::{RoadNetwork, Node, Edge};

let mut network = RoadNetwork::new();

// Add nodes (intersections)
let node1 = network.add_node(Node {
    id: 1,
    position: Point::new(-122.4194, 37.7749),
});

let node2 = network.add_node(Node {
    id: 2,
    position: Point::new(-122.4083, 37.7849),
});

// Add edges (roads)
network.add_edge(Edge {
    source: node1,
    target: node2,
    weight: 1200.0,  // Distance in meters
});

// Prepare for routing (builds Contraction Hierarchies)
network.prepare()?;
```

### Routing Queries

Find shortest paths:

```rust
// Find path between nodes
let path = network.shortest_path(node1, node2)?;

println!("Path length: {} meters", path.total_distance);
println!("Nodes: {:?}", path.nodes);

// Performance: 7µs per query (143x better than 1ms target)
```

### FastGraph Caching

Optimized routing with caching:

```rust
use sim_geo::routing::FastGraph;

// Create cached graph
let fast_graph = FastGraph::from_road_network(network);

// Lightning-fast queries: 330ns (5645x speedup over uncached)
let path = fast_graph.route(origin, destination)?;
```

### JSON Import/Export

```rust
// Export network
network.save_to_json("road_network.json")?;

// Import network
let network = RoadNetwork::load_from_json("road_network.json")?;
```

## Integration with Agents

### Agent Movement Pattern

```rust
impl Agent for DeliveryDriver {
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        let mut result = AgentTickResult::default();

        match self.status {
            DriverStatus::Idle => {
                if let Some(destination) = self.get_next_delivery() {
                    // Start journey
                    result.journeys.push(JourneyState {
                        agent_id: self.id,
                        origin: self.current_location,
                        destination,
                        started_at: ctx.current_time(),
                        progress: 0.0,
                    });

                    self.status = DriverStatus::EnRoute;
                }
            }

            DriverStatus::EnRoute => {
                // Movement handled by MovementHandler
                // Transition occurs when journey completes
            }

            _ => {}
        }

        Ok(result)
    }
}
```

### Simulation Hooks

Process journeys in hooks:

```rust
impl SimulationHooks for GeoHooks {
    fn on_agents_ticked(
        &mut self,
        ctx: &SimulationContext,
        results: &[AgentTickResult],
    ) -> Result<()> {
        // Collect journey requests
        for result in results {
            for journey in &result.journeys {
                self.movement.add_journey(journey.clone());
            }
        }

        // Advance all journeys
        self.movement.advance_journeys(ctx.time_step())?;

        // Handle arrivals
        let completed = self.movement.get_completed_journeys();
        for journey in completed {
            self.on_agent_arrived(journey.agent_id, journey.destination)?;
        }

        Ok(())
    }
}
```

## Performance

Phase 7 road network performance:

- **Graph construction**: 3.5ms for 1,000 nodes (65% better than 10ms target)
- **Graph loading**: 2.88ms for 100 nodes (94% better than 50ms target)
- **Routing queries**: 7µs (143x better than 1ms target)
- **FastGraph cached**: 330ns (5645x speedup)

### Optimization Tips

```rust
// ✅ Good - batch journey processing
let journeys = vec![journey1, journey2, journey3];
for journey in journeys {
    movement.add_journey(journey);
}
movement.advance_journeys(time_delta)?;

// ❌ Bad - individual processing
movement.add_journey(journey1);
movement.advance_journeys(time_delta)?;
movement.add_journey(journey2);
movement.advance_journeys(time_delta)?;  // Inefficient!
```

## Examples from Reference Implementations

### sim-kitchen: Courier Movement

```rust
// Courier picks up order and delivers
let journey = JourneyState {
    agent_id: self.id,
    origin: kitchen_location,
    destination: customer_location,
    started_at: ctx.current_time(),
    progress: 0.0,
};

result.journeys.push(journey);
```

**File:** `crates/sim-kitchen/src/agents/courier.rs`

### sim-logistics: Driver Routing

```rust
// Driver with multiple stops
impl Driver {
    fn plan_route(&self, warehouse: &Warehouse, destinations: &[Point<f64>]) -> Vec<JourneyState> {
        let mut journeys = vec![];
        let mut current = warehouse.position();

        for destination in destinations {
            journeys.push(JourneyState {
                agent_id: self.id,
                origin: current,
                destination: *destination,
                started_at: Utc::now(),
                progress: 0.0,
            });
            current = *destination;
        }

        // Return to warehouse
        journeys.push(JourneyState {
            agent_id: self.id,
            origin: current,
            destination: warehouse.position(),
            started_at: Utc::now(),
            progress: 0.0,
        });

        journeys
    }
}
```

**File:** `crates/sim-logistics/src/agents/driver.rs`

## Common Pitfalls

### Pitfall 1: Wrong Resolution

```rust
// ❌ Wrong - resolution too coarse
let handler = LocationHandler::new(Resolution::Five);  // ~252 km² cells!

// ✅ Correct - appropriate resolution
let handler = LocationHandler::new(Resolution::Nine);  // ~0.1 km² cells
```

### Pitfall 2: Not Advancing Journeys

```rust
// ❌ Wrong - journeys never progress
movement.add_journey(journey);
// Forgot to call advance_journeys!

// ✅ Correct - advance in tick loop
impl SimulationHooks for MyHooks {
    fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) -> Result<()> {
        // Add journeys
        for result in results {
            for journey in &result.journeys {
                self.movement.add_journey(journey.clone());
            }
        }

        // Advance!
        self.movement.advance_journeys(ctx.time_step())?;
        Ok(())
    }
}
```

### Pitfall 3: Distance Units

```rust
// ❌ Wrong - mixing units
handler.within_radius(position, 1.0)?;  // 1 meter? 1 km?

// ✅ Correct - explicit units
handler.within_radius(position, 1000.0)?;  // 1000 meters = 1 km
```

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_h3_indexing() {
        let mut handler = LocationHandler::new(Resolution::Nine);

        let location = Restaurant {
            id: LocationId::new(),
            position: Point::new(-122.4194, 37.7749),
            capacity: 50,
        };

        handler.add_location(Box::new(location)).unwrap();

        // Find nearby
        let nearby = handler.within_radius(
            Point::new(-122.4183, 37.7750),
            1000.0
        ).unwrap();

        assert_eq!(nearby.len(), 1);
    }

    #[test]
    fn test_journey_completion() {
        let mut movement = MovementHandler::new()
            .with_velocity_model(Box::new(ConstantVelocity::new(10.0)));

        let journey = JourneyState {
            agent_id: AgentId::new(),
            origin: Point::new(0.0, 0.0),
            destination: Point::new(0.01, 0.0),  // ~1.1 km
            started_at: Utc::now(),
            progress: 0.0,
        };

        movement.add_journey(journey);

        // Advance for enough time to complete
        movement.advance_journeys(Duration::seconds(200)).unwrap();

        let completed = movement.get_completed_journeys();
        assert_eq!(completed.len(), 1);
    }
}
```

## Related Documentation

- [Agents](agents.md) - Agent movement and journeys
- [Resources](resources.md) - Location-based resource allocation
- [Workflows](workflows.md) - Multi-leg journeys in workflows

## Summary

Geospatial capabilities provide:

- H3 hexagonal indexing (5 resolution levels)
- LocationHandler with spatial queries
- Journey tracking with velocity models
- Road network routing (7µs queries)
- FastGraph caching (330ns queries)
- 157 tests validating all features

---

**Last Updated:** March 16, 2026
