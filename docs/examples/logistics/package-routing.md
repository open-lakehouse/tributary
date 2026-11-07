# Package Routing Example

Multi-stop deliveries with road network routing and route optimization.

## Overview

- RoadNetwork for realistic routing
- Multi-stop delivery planning
- Route optimization
- Journey tracking

## Road Network Setup

```rust
use sim_geo::{RoadNetwork, RoadNode, RoadEdge, RoadType};

let mut network = RoadNetwork::new();

// Add intersection nodes
let warehouse_node = network.add_node(RoadNode {
    id: NodeId::new(),
    position: Point::new(-122.4194, 37.7749),
    node_type: "warehouse".to_string(),
});

let delivery_node_1 = network.add_node(RoadNode {
    id: NodeId::new(),
    position: Point::new(-122.4100, 37.7800),
    node_type: "delivery".to_string(),
});

let delivery_node_2 = network.add_node(RoadNode {
    id: NodeId::new(),
    position: Point::new(-122.4050, 37.7850),
    node_type: "delivery".to_string(),
});

// Add road segments
network.add_edge(RoadEdge {
    id: EdgeId::new(),
    from_node: warehouse_node,
    to_node: delivery_node_1,
    length_meters: 1500.0,
    road_type: RoadType::Arterial,
    speed_limit: Some(22.0),  // ~50 mph
});

network.add_edge(RoadEdge {
    id: EdgeId::new(),
    from_node: delivery_node_1,
    to_node: delivery_node_2,
    length_meters: 800.0,
    road_type: RoadType::Residential,
    speed_limit: Some(13.4),  // ~30 mph
});
```

## Multi-Stop Route Planning

```rust
// Find route through multiple stops
let stops = vec![warehouse_node, delivery_node_1, delivery_node_2];

let mut total_distance = 0.0;
let mut route_geometry = Vec::new();

for window in stops.windows(2) {
    if let Some(segment_route) = network.find_route(window[0], window[1]) {
        let distance = network.get_route_distance(&segment_route);
        let geometry = network.get_route_geometry(&segment_route);

        total_distance += distance;
        route_geometry.extend(geometry);
    }
}

println!("Total route distance: {:.1}m", total_distance);
```

## Integration with Driver Agent

```rust
struct Driver {
    id: AgentId,
    state: DriverState,
    current_position: Point<f64>,
    planned_route: Vec<Point<f64>>,
    delivery_stops: Vec<DeliveryStop>,
}

impl Agent for Driver {
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        let mut result = AgentTickResult::default();

        match &self.state {
            DriverState::PlanningRoute => {
                // Generate route through all stops
                let route = self.plan_multi_stop_route(&network)?;
                self.planned_route = route;

                // Start journey
                result.new_journeys.push(JourneyRequest {
                    id: None,
                    agent_id: self.id(),
                    origin: self.current_position,
                    destination: self.delivery_stops[0].location,
                    metadata: HashMap::from([
                        ("route".to_string(), serde_json::to_string(&self.planned_route)?)
                    ]),
                });

                self.state = DriverState::EnRoute;
            }
            _ => {}
        }

        Ok(result)
    }
}
```

## See Also

- `crates/sim-logistics/src/agents/mod.rs`
