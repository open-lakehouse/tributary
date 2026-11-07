# sim-geo API Reference

`sim-geo` provides geospatial capabilities for agent-based simulations, including journey planning, road network routing, location management, and H3 spatial indexing.

## Table of Contents

- [Location Management](#location-management)
- [Movement and Journeys](#movement-and-journeys)
- [Road Network Routing](#road-network-routing)
- [Spatial Indexing](#spatial-indexing)
- [Integration with sim-core](#integration-with-sim-core)

---

## Location Management

### Location

Represents a point of interest with optional metadata.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub id: LocationId,
    pub position: Point<f64>,
    pub location_type: String,
    pub metadata: HashMap<String, String>,
}

impl Location {
    pub fn new(position: Point<f64>, location_type: String) -> Self;
    pub fn with_id(id: LocationId, position: Point<f64>, location_type: String) -> Self;
    pub fn distance_to(&self, other: &Location) -> f64;
}
```

**Example:**

```rust
use sim_geo::Location;
use geo_types::Point;

// Create warehouse location
let warehouse = Location::new(
    Point::new(-122.4194, 37.7749),
    "warehouse".to_string()
);

// With metadata
let mut metadata = HashMap::new();
metadata.insert("capacity".to_string(), "1000".to_string());
metadata.insert("name".to_string(), "SF Main Warehouse".to_string());

let warehouse = Location {
    id: LocationId::new(),
    position: Point::new(-122.4194, 37.7749),
    location_type: "warehouse".to_string(),
    metadata,
};
```

### LocationHandler

Manages location lifecycle with Arrow storage.

```rust
pub struct LocationHandler {
    pub fn new() -> Self;
    pub fn add_location(&mut self, location: Location) -> Result<LocationId>;
    pub fn get_location(&self, id: LocationId) -> Option<&Location>;
    pub fn get_location_mut(&mut self, id: LocationId) -> Option<&mut Location>;
    pub fn remove_location(&mut self, id: LocationId) -> Option<Location>;
    pub fn find_by_type(&self, location_type: &str) -> Vec<&Location>;
    pub fn find_within_radius(&self, center: Point<f64>, radius_meters: f64) -> Vec<&Location>;
    pub fn count(&self) -> usize;
    pub fn export_batch(&self) -> RecordBatch;
    pub fn import_batch(&mut self, batch: RecordBatch) -> Result<()>;
}
```

**Example:**

```rust
use sim_geo::LocationHandler;

let mut locations = LocationHandler::new();

// Add locations
let warehouse_id = locations.add_location(warehouse)?;
let customer_id = locations.add_location(customer_address)?;

// Query by type
let warehouses = locations.find_by_type("warehouse");

// Proximity search
let nearby = locations.find_within_radius(
    Point::new(-122.4194, 37.7749),
    5000.0  // 5km radius
);

// Export/import for snapshots
let batch = locations.export_batch();
locations.import_batch(batch)?;
```

---

## Movement and Journeys

### Journey

Tracks agent movement from origin to destination.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Journey {
    pub id: JourneyId,
    pub agent_id: AgentId,
    pub origin: Point<f64>,
    pub destination: Point<f64>,
    pub state: JourneyState,
    pub start_time: DateTime<Utc>,
    pub expected_end_time: Option<DateTime<Utc>>,
    pub actual_end_time: Option<DateTime<Utc>>,
    pub distance_meters: f64,
    pub route: Option<Vec<Point<f64>>>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JourneyState {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

impl Journey {
    pub fn new(
        agent_id: AgentId,
        origin: Point<f64>,
        destination: Point<f64>,
        start_time: DateTime<Utc>
    ) -> Self;

    pub fn with_route(mut self, route: Vec<Point<f64>>) -> Self;
    pub fn is_complete(&self, current_time: DateTime<Utc>) -> bool;
    pub fn progress(&self, current_time: DateTime<Utc>) -> f64;
    pub fn current_position(&self, current_time: DateTime<Utc>) -> Point<f64>;
}
```

**Example:**

```rust
use sim_geo::Journey;

// Create journey
let journey = Journey::new(
    agent_id,
    Point::new(-122.4194, 37.7749),  // Origin
    Point::new(-122.3959, 37.7937),  // Destination
    start_time
);

// Check progress
let progress = journey.progress(current_time);  // 0.0 to 1.0

// Get current position along route
let current_pos = journey.current_position(current_time);
```

### MovementHandler

Plans journeys (along a road network when one is loaded, otherwise in a
straight line) and advances them with a velocity model.

```rust
pub struct MovementHandler { /* ... */ }

impl MovementHandler {
    pub fn new() -> Self;                                   // ConstantVelocity::default(), no network
    pub fn with_velocity_model(self, model: Box<dyn VelocityModel>) -> Self;
    pub fn with_network(self, network: RoadNetwork) -> Self;
    pub fn load_network(&mut self, network: RoadNetwork);
    pub fn plan_journey(&self, agent_id: AgentId, origin: Point<f64>, destination: Point<f64>,
                        start_time: DateTime<Utc>) -> Result<JourneyState>;
    pub fn add_journey(&mut self, journey: JourneyState);
    pub fn advance_journeys(&mut self, now: DateTime<Utc>, time_step: Duration) -> Result<()>;
    pub fn get_completed_journeys(&self) -> Vec<JourneyState>;
    pub fn remove_completed(&mut self) -> Vec<JourneyState>;
    pub fn active_count(&self) -> usize;
}
```

**Example:**

```rust
use sim_geo::{MovementHandler, movement::ConstantVelocity};

let mut movement = MovementHandler::new()
    .with_velocity_model(Box::new(ConstantVelocity::new(8.0))); // 8 m/s

let journey = movement.plan_journey(courier_id, restaurant, customer, ctx.current_time())?;
movement.add_journey(journey);

// Each tick:
movement.advance_journeys(ctx.current_time(), ctx.time_step())?;
for arrived in movement.remove_completed() {
    println!("{} arrived after {:.0} m", arrived.agent_id, arrived.distance_m);
}
```

### VelocityModel

Speed of a journey, used by `MovementHandler::advance_journeys`.

```rust
pub trait VelocityModel: Send + Sync {
    /// Speed in m/s at the given progress (0.0..=1.0)
    fn velocity_m_s(&self, journey: &JourneyState, progress: f64) -> f64;

    /// Speed at a given simulation time; defaults to `velocity_m_s`.
    /// Time-aware models (TimeOfDayVelocity) override this.
    fn velocity_m_s_at_time(&self, journey: &JourneyState, progress: f64,
                            now: DateTime<Utc>) -> f64;
}
```

**Built-in Velocity Models:**

#### ConstantVelocity

Fixed speed regardless of time or conditions.

```rust
pub struct ConstantVelocity {
    pub fn new(meters_per_second: f64) -> Self;
}
```

#### TimeOfDayVelocity

Varies speed by time of day (e.g., traffic patterns).

```rust
pub struct TimeOfDayVelocity {
    pub fn new(schedule: Vec<(u32, f64)>) -> Self;
}
```

**Example:**

```rust
// Slower during rush hour
let velocity_model = TimeOfDayVelocity::new(vec![
    (7, 15.0),   // 7:00 AM - 15 m/s (traffic)
    (9, 25.0),   // 9:00 AM - 25 m/s (normal)
    (17, 12.0),  // 5:00 PM - 12 m/s (heavy traffic)
    (19, 25.0),  // 7:00 PM - 25 m/s (normal)
]);
```

---

## Road Network Routing

### RoadNetwork

Graph-based road network with routing capabilities.

```rust
pub struct RoadNetwork {
    pub fn new() -> Self;
    pub fn add_node(&mut self, node: RoadNode) -> NodeId;
    pub fn add_edge(&mut self, edge: RoadEdge) -> EdgeId;
    pub fn find_route(&self, from: NodeId, to: NodeId) -> Option<Vec<NodeId>>;
    pub fn find_nearest_node(&self, position: Point<f64>) -> Option<NodeId>;
    pub fn get_route_distance(&self, route: &[NodeId]) -> f64;
    pub fn get_route_geometry(&self, route: &[NodeId]) -> Vec<Point<f64>>;
}

pub struct RoadNode {
    pub id: NodeId,
    pub position: Point<f64>,
    pub node_type: String,
}

pub struct RoadEdge {
    pub id: EdgeId,
    pub from_node: NodeId,
    pub to_node: NodeId,
    pub length_meters: f64,
    pub road_type: RoadType,
    pub speed_limit: Option<f64>,
}

pub enum RoadType {
    Highway,
    Arterial,
    Residential,
    ServiceRoad,
}
```

**Example:**

```rust
use sim_geo::{RoadNetwork, RoadNode, RoadEdge, RoadType};

let mut network = RoadNetwork::new();

// Add nodes
let node1 = network.add_node(RoadNode {
    id: NodeId::new(),
    position: Point::new(-122.4194, 37.7749),
    node_type: "intersection".to_string(),
});

let node2 = network.add_node(RoadNode {
    id: NodeId::new(),
    position: Point::new(-122.3959, 37.7937),
    node_type: "intersection".to_string(),
});

// Add edge
network.add_edge(RoadEdge {
    id: EdgeId::new(),
    from_node: node1,
    to_node: node2,
    length_meters: 3500.0,
    road_type: RoadType::Arterial,
    speed_limit: Some(22.0),  // ~50 mph
});

// Find route
if let Some(route) = network.find_route(node1, node2) {
    let distance = network.get_route_distance(&route);
    let geometry = network.get_route_geometry(&route);
    println!("Route distance: {:.1}m", distance);
}

// Snap location to network
let nearest_node = network.find_nearest_node(
    Point::new(-122.4100, 37.7800)
);
```

### FastGraph (Optional: For large networks)

Optimized routing for networks with 100k+ nodes.

```rust
pub struct FastGraph {
    pub fn from_network(network: &RoadNetwork) -> Self;
    pub fn shortest_path(&self, from: NodeId, to: NodeId) -> Option<Vec<NodeId>>;
    pub fn distance(&self, from: NodeId, to: NodeId) -> Option<f64>;
}
```

---

## Spatial Indexing

### H3Index

Hexagonal hierarchical spatial indexing system.

```rust
pub struct H3Index {
    pub fn from_point(point: Point<f64>, resolution: u8) -> Result<Self>;
    pub fn to_cell_id(&self) -> u64;
    pub fn to_string(&self) -> String;
    pub fn get_resolution(&self) -> u8;
    pub fn to_boundary(&self) -> Vec<Point<f64>>;
    pub fn neighbors(&self) -> Result<Vec<H3Index>>;
    pub fn distance_to(&self, other: &H3Index) -> Result<i32>;
}
```

**Resolutions:**

- `0-2`: Continental/Regional (100-1000km)
- `3-5`: City/County (10-100km)
- `6-8`: Neighborhood (1-10km)
- `9-11`: Block/Building (100-1000m)
- `12-15`: Meter-level precision

**Example:**

```rust
use sim_geo::H3Index;

// Create H3 index at resolution 9 (neighborhood level)
let h3 = H3Index::from_point(
    Point::new(-122.4194, 37.7749),
    9
)?;

println!("H3 Cell: {}", h3.to_string());

// Get neighbors for proximity analysis
let neighbors = h3.neighbors()?;

// Distance between cells
let h3_other = H3Index::from_point(other_point, 9)?;
let distance = h3.distance_to(&h3_other)?;  // Grid cell hops
```

**Use Cases:**

```rust
// Warehouse clustering
use std::collections::HashMap;

let mut warehouse_by_h3: HashMap<String, Vec<LocationId>> = HashMap::new();

for warehouse in warehouses {
    let h3 = H3Index::from_point(warehouse.position, 7)?;
    warehouse_by_h3
        .entry(h3.to_string())
        .or_default()
        .push(warehouse.id);
}

// Find nearest warehouse by H3 cell
let customer_h3 = H3Index::from_point(customer_location, 7)?;
if let Some(nearby_warehouses) = warehouse_by_h3.get(&customer_h3.to_string()) {
    // Assign from same cell first
}
```

---

## Integration with sim-core

### Using Journeys with Agents

The runner does not move agents. `AgentTickResult::new_journeys` exists but
is not consumed, so domain code owns a `MovementHandler`: plan journeys where
the domain decides an agent should travel (e.g. in a hook or the domain
wrapper's `step`), advance them each tick with the simulation time, and
apply arrivals to the agents.

```rust
// In the domain wrapper's step(), after runner.tick():
let now = runner.context().current_time();
movement.advance_journeys(now, runner.context().time_step())?;
for arrived in movement.remove_completed() {
    // look up the agent by arrived.agent_id and update its state
}
```

## Complete Example: Delivery Simulation

```rust
use sim_core::{SimulationRunner, SimulationConfig};
use sim_geo::{LocationHandler, MovementHandler, RoadNetwork, TimeOfDayVelocity};

// Setup locations
let mut locations = LocationHandler::new();
let warehouse_id = locations.add_location(warehouse)?;
let customer_id = locations.add_location(customer_address)?;

// Setup movement
let velocity_model = Box::new(TimeOfDayVelocity::new(vec![
    (0, 30.0),
    (8, 15.0),
    (18, 25.0),
]));
let movement = MovementHandler::new(velocity_model);

// Setup road network (optional)
let mut network = RoadNetwork::new();
// Add nodes and edges...

// Create simulation
let config = SimulationConfig::default();
let mut runner = SimulationRunner::new(config);

// Register components
runner.set_population_handler(couriers);
runner.set_hooks(Box::new(GeoSimulationHooks {
    movement_handler: movement,
    locations,
    network,
}));

// Run
runner.run_for_duration(Duration::hours(24))?;
```

---

## Performance Considerations

### Large Location Sets

For simulations with 10k+ locations:

```rust
// Use spatial indexing
use std::collections::HashMap;

struct SpatialLocationIndex {
    by_h3: HashMap<String, Vec<LocationId>>,
    by_type: HashMap<String, Vec<LocationId>>,
}

impl SpatialLocationIndex {
    fn find_nearest(&self, point: Point<f64>, location_type: &str, resolution: u8) -> Vec<LocationId> {
        let h3 = H3Index::from_point(point, resolution)?;
        let cell_key = h3.to_string();

        // Check current cell
        if let Some(locs) = self.by_h3.get(&cell_key) {
            return locs.clone();
        }

        // Check neighbors
        for neighbor in h3.neighbors()? {
            if let Some(locs) = self.by_h3.get(&neighbor.to_string()) {
                return locs.clone();
            }
        }

        vec![]
    }
}
```

### Large Road Networks

For networks with 100k+ edges:

```rust
// Use FastGraph for routing
let fast_graph = FastGraph::from_network(&network);

// Cache frequently used routes
use std::collections::HashMap;

struct RouteCache {
    cache: HashMap<(NodeId, NodeId), Vec<NodeId>>,
}

impl RouteCache {
    fn get_or_compute(&mut self, from: NodeId, to: NodeId, graph: &FastGraph) -> Vec<NodeId> {
        if let Some(route) = self.cache.get(&(from, to)) {
            return route.clone();
        }

        if let Some(route) = graph.shortest_path(from, to) {
            self.cache.insert((from, to), route.clone());
            route
        } else {
            vec![]
        }
    }
}
```

---

## See Also

- [sim-core API Reference](./sim-core.md) - Core simulation abstractions
- [Integration Patterns](./integration.md) - Cross-crate integration
- [Logistics Example](../examples/logistics/package-routing.md) - Road network routing
