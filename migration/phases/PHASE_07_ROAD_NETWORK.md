# Phase 7: Road Network Integration & Realistic Movement

**Status**: NOT STARTED
**Priority**: 🟡 MEDIUM
**Estimated Effort**: 1 week
**Dependencies**: None (can run parallel to Phase 5, 6)
**Blocks**: Realistic delivery time estimates

---

## Objectives

1. Port road network graph building from universe crate
2. Load pre-built road graphs (skip OSM parsing for now)
3. Implement journey planning with Dijkstra routing
4. Add multi-segment journey support
5. Implement TimeOfDayVelocity model for traffic patterns

---

## Prerequisites Checklist

- [x] Universe crate has working graph implementation (`universe/src/state/movement_next.rs`)
- [x] MovementHandler exists (`crates/sim-geo/src/movement/handler.rs`)
- [x] VelocityModel trait defined (`crates/sim-geo/src/movement/velocity.rs`)
- [x] Journey struct defined (`crates/sim-geo/src/movement/journey.rs`)

---

## Current State

### What Exists
```rust
// crates/sim-geo/src/movement/handler.rs
pub struct MovementHandler {
    journeys: HashMap<JourneyId, Journey>,
}

pub struct Journey {
    pub id: JourneyId,
    pub agent_id: AgentId,
    pub route: LineString<f64>,  // Straight line currently
    pub distance_m: f64,
    pub progress: f64,
}
```

**Problem**: Routes are straight lines, no road network, unrealistic travel times.

### What's Needed
- Road network graph with nodes/edges
- Dijkstra shortest path routing
- Multi-segment journeys
- Realistic edge weights (distance, speed limits)

---

## Strategy

**Don't reinvent the wheel**: Use existing universe implementation as reference.

**Key Files to Port**:
- `universe/src/state/movement_next.rs:1-400` - Graph, routing, journey logic
- `universe/src/builders/graph.rs` - Graph serialization/loading

**Simplification**: Skip OSM parsing, use pre-built graph files.

---

## Tasks

### Task 7.1: Create Graph Module

**File**: `crates/sim-geo/src/graph/mod.rs` (new)

**Port from**: `universe/src/state/movement_next.rs:50-150`

```rust
use geo::Point;
use std::collections::HashMap;

pub type NodeId = u64;
pub type EdgeId = u64;

#[derive(Clone, Debug)]
pub struct RoadNetwork {
    pub nodes: HashMap<NodeId, RoadNode>,
    pub edges: HashMap<EdgeId, RoadEdge>,
    pub adjacency: HashMap<NodeId, Vec<EdgeId>>,  // node → outgoing edges
}

#[derive(Clone, Debug)]
pub struct RoadNode {
    pub id: NodeId,
    pub position: Point<f64>,
}

#[derive(Clone, Debug)]
pub struct RoadEdge {
    pub id: EdgeId,
    pub from_node: NodeId,
    pub to_node: NodeId,
    pub distance_m: f64,
    pub geometry: LineString<f64>,
    pub road_type: RoadType,
    pub speed_limit_mps: f64,  // meters per second
}

#[derive(Clone, Debug, PartialEq)]
pub enum RoadType {
    Highway,
    Arterial,
    Residential,
    ServiceRoad,
}

impl RoadNetwork {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            adjacency: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, node: RoadNode) {
        self.adjacency.entry(node.id).or_insert_with(Vec::new);
        self.nodes.insert(node.id, node);
    }

    pub fn add_edge(&mut self, edge: RoadEdge) {
        let from = edge.from_node;
        self.adjacency.entry(from).or_default().push(edge.id);
        self.edges.insert(edge.id, edge);
    }

    pub fn get_node(&self, id: NodeId) -> Option<&RoadNode> {
        self.nodes.get(&id)
    }

    pub fn get_edge(&self, id: EdgeId) -> Option<&RoadEdge> {
        self.edges.get(&id)
    }

    pub fn outgoing_edges(&self, node_id: NodeId) -> Vec<&RoadEdge> {
        self.adjacency
            .get(&node_id)
            .map(|edge_ids| {
                edge_ids.iter().filter_map(|id| self.edges.get(id)).collect()
            })
            .unwrap_or_default()
    }
}
```

**Acceptance Criteria**:
- [ ] RoadNetwork struct created
- [ ] RoadNode and RoadEdge defined
- [ ] Basic graph operations (add_node, add_edge, outgoing_edges)
- [ ] Unit tests for graph building

---

### Task 7.2: Implement Graph Serialization/Loading

**File**: `crates/sim-geo/src/graph/io.rs` (new)

**Port from**: `universe/src/builders/graph.rs`

```rust
use serde::{Serialize, Deserialize};
use std::path::Path;

#[derive(Serialize, Deserialize)]
struct SerializedGraph {
    nodes: Vec<SerializedNode>,
    edges: Vec<SerializedEdge>,
}

#[derive(Serialize, Deserialize)]
struct SerializedNode {
    id: NodeId,
    x: f64,
    y: f64,
}

#[derive(Serialize, Deserialize)]
struct SerializedEdge {
    id: EdgeId,
    from: NodeId,
    to: NodeId,
    distance: f64,
    geometry: Vec<(f64, f64)>,  // Points in LineString
    road_type: String,
    speed_limit_mps: f64,
}

impl RoadNetwork {
    /// Load pre-built graph from JSON file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let serialized: SerializedGraph = serde_json::from_str(&data)?;

        let mut network = Self::new();

        for node in serialized.nodes {
            network.add_node(RoadNode {
                id: node.id,
                position: Point::new(node.x, node.y),
            });
        }

        for edge in serialized.edges {
            let geometry = LineString::new(
                edge.geometry.into_iter().map(|(x, y)| Coordinate { x, y }).collect()
            );

            network.add_edge(RoadEdge {
                id: edge.id,
                from_node: edge.from,
                to_node: edge.to,
                distance_m: edge.distance,
                geometry,
                road_type: parse_road_type(&edge.road_type),
                speed_limit_mps: edge.speed_limit_mps,
            });
        }

        Ok(network)
    }

    /// Save graph to JSON file
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let serialized = SerializedGraph {
            nodes: self.nodes.values().map(|n| SerializedNode {
                id: n.id,
                x: n.position.x(),
                y: n.position.y(),
            }).collect(),
            edges: self.edges.values().map(|e| SerializedEdge {
                id: e.id,
                from: e.from_node,
                to: e.to_node,
                distance: e.distance_m,
                geometry: e.geometry.points().map(|p| (p.x(), p.y())).collect(),
                road_type: format!("{:?}", e.road_type),
                speed_limit_mps: e.speed_limit_mps,
            }).collect(),
        };

        let json = serde_json::to_string_pretty(&serialized)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}
```

**Acceptance Criteria**:
- [ ] Can load graph from JSON file
- [ ] Can save graph to JSON file
- [ ] Round-trip test: save → load → verify equality
- [ ] Compatible with universe graph format

---

### Task 7.3: Implement Dijkstra Routing

**File**: `crates/sim-geo/src/graph/routing.rs` (new)

**Port from**: `universe/src/state/movement_next.rs:200-300`

```rust
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;

#[derive(Clone)]
struct RouteState {
    node: NodeId,
    cost: f64,
    path: Vec<EdgeId>,
}

impl Ord for RouteState {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for RouteState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for RouteState {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}

impl Eq for RouteState {}

impl RoadNetwork {
    /// Find shortest path using Dijkstra's algorithm
    pub fn find_shortest_path(
        &self,
        start: NodeId,
        goal: NodeId
    ) -> Option<Route> {
        let mut heap = BinaryHeap::new();
        let mut distances: HashMap<NodeId, f64> = HashMap::new();

        distances.insert(start, 0.0);
        heap.push(RouteState {
            node: start,
            cost: 0.0,
            path: Vec::new(),
        });

        while let Some(RouteState { node, cost, path }) = heap.pop() {
            if node == goal {
                return Some(Route {
                    edges: path,
                    total_distance_m: cost,
                });
            }

            if cost > *distances.get(&node).unwrap_or(&f64::MAX) {
                continue;
            }

            for edge in self.outgoing_edges(node) {
                let next_node = edge.to_node;
                let next_cost = cost + edge.distance_m;

                if next_cost < *distances.get(&next_node).unwrap_or(&f64::MAX) {
                    distances.insert(next_node, next_cost);
                    let mut next_path = path.clone();
                    next_path.push(edge.id);

                    heap.push(RouteState {
                        node: next_node,
                        cost: next_cost,
                        path: next_path,
                    });
                }
            }
        }

        None  // No path found
    }
}

#[derive(Clone, Debug)]
pub struct Route {
    pub edges: Vec<EdgeId>,
    pub total_distance_m: f64,
}

impl Route {
    /// Get full geometry by concatenating edge geometries
    pub fn to_linestring(&self, network: &RoadNetwork) -> LineString<f64> {
        let mut points = Vec::new();

        for edge_id in &self.edges {
            if let Some(edge) = network.get_edge(*edge_id) {
                points.extend(edge.geometry.points());
            }
        }

        LineString::new(points.into_iter().map(|p| Coordinate { x: p.x(), y: p.y() }).collect())
    }
}
```

**Acceptance Criteria**:
- [ ] Dijkstra algorithm implemented
- [ ] Returns shortest path as sequence of edges
- [ ] Can convert route to LineString geometry
- [ ] Unit tests with known graph verify correct paths
- [ ] Performance acceptable (<100ms for 10k node graph)

---

### Task 7.4: Integrate Graph with MovementHandler

**File**: `crates/sim-geo/src/movement/handler.rs`

**Add to MovementHandler**:
```rust
use crate::graph::{RoadNetwork, Route};

pub struct MovementHandler {
    journeys: HashMap<JourneyId, Journey>,
    network: Option<RoadNetwork>,  // ← Add this
}

impl MovementHandler {
    pub fn new() -> Self {
        Self {
            journeys: HashMap::new(),
            network: None,
        }
    }

    /// Load road network
    pub fn load_network(&mut self, network: RoadNetwork) {
        self.network = Some(network);
    }

    /// Plan journey using road network (if available)
    pub fn plan_journey(
        &mut self,
        agent_id: AgentId,
        origin: Point<f64>,
        destination: Point<f64>,
        velocity_model: &dyn VelocityModel
    ) -> Result<JourneyId> {
        let route_geometry = if let Some(network) = &self.network {
            // Find nearest nodes to origin and destination
            let start_node = self.find_nearest_node(network, origin)?;
            let goal_node = self.find_nearest_node(network, destination)?;

            // Route on network
            let route = network.find_shortest_path(start_node, goal_node)
                .ok_or_else(|| anyhow!("No path found"))?;

            route.to_linestring(network)
        } else {
            // Fallback: straight line
            LineString::new(vec![
                Coordinate { x: origin.x(), y: origin.y() },
                Coordinate { x: destination.x(), y: destination.y() },
            ])
        };

        let distance_m = route_geometry.haversine_length();

        let journey_id = JourneyId::new();
        let journey = Journey {
            id: journey_id,
            agent_id,
            route: route_geometry,
            distance_m,
            progress: 0.0,
            metadata: HashMap::new(),
        };

        self.journeys.insert(journey_id, journey);
        Ok(journey_id)
    }

    fn find_nearest_node(&self, network: &RoadNetwork, point: Point<f64>) -> Result<NodeId> {
        network.nodes.values()
            .min_by_key(|node| {
                let dist = node.position.haversine_distance(&point);
                (dist * 1000.0) as u64  // Convert to millimeters for integer comparison
            })
            .map(|node| node.id)
            .ok_or_else(|| anyhow!("No nodes in network"))
    }
}
```

**Acceptance Criteria**:
- [ ] MovementHandler can load road network
- [ ] plan_journey() uses routing when network available
- [ ] Falls back to straight line if no network
- [ ] Finds nearest nodes to origin/destination
- [ ] Integration test: plan journey on real network

---

### Task 7.5: Implement TimeOfDayVelocity Model

**File**: `crates/sim-geo/src/movement/velocity.rs`

**Add Implementation**:
```rust
#[derive(Clone, Debug)]
pub struct TimeOfDayVelocity {
    /// Speed multipliers by hour (0-23)
    pub hourly_multipliers: [f64; 24],
    pub base_speed_mps: f64,
}

impl TimeOfDayVelocity {
    /// Create with rush hour slowdowns
    pub fn with_rush_hours(base_speed_mps: f64) -> Self {
        let mut multipliers = [1.0; 24];

        // Morning rush hour (7-9 AM): 60% speed
        for hour in 7..=9 {
            multipliers[hour] = 0.6;
        }

        // Evening rush hour (5-7 PM): 50% speed
        for hour in 17..=19 {
            multipliers[hour] = 0.5;
        }

        // Late night (11 PM - 5 AM): 120% speed (less traffic)
        for hour in [23, 0, 1, 2, 3, 4, 5] {
            multipliers[hour] = 1.2;
        }

        Self {
            hourly_multipliers: multipliers,
            base_speed_mps,
        }
    }
}

impl VelocityModel for TimeOfDayVelocity {
    fn velocity_m_s(&self, journey: &Journey, _progress: f64, current_time: DateTime<Utc>) -> f64 {
        let hour = current_time.hour() as usize;
        let multiplier = self.hourly_multipliers[hour];
        self.base_speed_mps * multiplier
    }
}
```

**Acceptance Criteria**:
- [ ] TimeOfDayVelocity implemented
- [ ] Rush hour slowdowns (60% and 50% speed)
- [ ] Late night speedups (120% speed)
- [ ] Unit test verifies correct multipliers by hour
- [ ] Integration test shows delivery times vary by time of day

---

## Deliverables

### Code Changes
- [ ] RoadNetwork graph structure (`sim-geo/src/graph/mod.rs`)
- [ ] Graph serialization/loading (`sim-geo/src/graph/io.rs`)
- [ ] Dijkstra routing (`sim-geo/src/graph/routing.rs`)
- [ ] MovementHandler integration
- [ ] TimeOfDayVelocity model
- [ ] Example graph file for testing

### Tests
- [ ] Graph building tests (3 tests)
- [ ] Routing algorithm tests (5 tests)
- [ ] Serialization round-trip test (1 test)
- [ ] Movement handler integration test (2 tests)
- [ ] TimeOfDay velocity test (2 tests)
- [ ] Total: ~15 new tests

### Documentation
- [ ] Graph file format documented
- [ ] Routing algorithm documented
- [ ] Example: how to create and use road network
- [ ] Migration guide from universe routing

---

## Testing Strategy

### Unit Tests
```bash
cd crates/sim-geo
cargo test graph::tests
cargo test routing::tests
cargo test velocity::tests
```

### Integration Test
```bash
cargo test --test road_network_integration
```

**Test Scenario**:
1. Load graph from file
2. Create journey from point A to point B
3. Verify route follows roads (not straight line)
4. Verify total distance realistic
5. Advance journey with TimeOfDay velocity
6. Verify completion time varies by start time

### Example Graph
Create `test_data/small_city_graph.json` with:
- 100 nodes (grid layout)
- 200 edges (connecting grid)
- Mixed road types (highway, arterial, residential)
- Speed limits: 30 mps (highway), 15 mps (arterial), 8 mps (residential)

---

## Acceptance Criteria (Phase Complete)

- [ ] **Road Network Loaded**: Can load pre-built graph from JSON
- [ ] **Routing Works**: Dijkstra finds shortest paths
- [ ] **Realistic Journeys**: Routes follow roads, not straight lines
- [ ] **Traffic Patterns**: TimeOfDay velocity model working
- [ ] **Tests Pass**: All 15+ tests passing
- [ ] **Performance**: Routing <100ms for typical graph size
- [ ] **Documentation**: Clear guide for using road networks

---

## Example Usage

```rust
// Load network
let mut movement = MovementHandler::new();
let network = RoadNetwork::load_from_file("graphs/city.json")?;
movement.load_network(network);

// Create velocity model with rush hours
let velocity_model = TimeOfDayVelocity::with_rush_hours(15.0);  // 15 m/s base

// Plan journey
let origin = Point::new(-122.42, 37.77);  // San Francisco
let destination = Point::new(-122.40, 37.79);

let journey_id = movement.plan_journey(
    agent_id,
    origin,
    destination,
    &velocity_model
)?;

// Journey will follow roads, slower during rush hours
```

---

**Estimated Effort**: 1 week
**Can Start**: Immediately (parallel to Phase 5, 6)
**Blocks**: Realistic delivery estimates
