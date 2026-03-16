# Test Data Usage Examples

This document provides practical examples of how to use the test data files in Rust code.

## Loading Graph Data

### Basic Loading

```rust
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Deserialize)]
struct GraphNode {
    id: u32,
    lat: f64,
    lon: f64,
}

#[derive(Debug, Deserialize)]
struct GraphEdge {
    id: u32,
    from_node: u32,
    to_node: u32,
    distance_m: f64,
    road_type: String,
    geometry: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct CityGraph {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

fn load_graph(path: &str) -> Result<CityGraph, Box<dyn std::error::Error>> {
    let data = fs::read_to_string(path)?;
    let graph: CityGraph = serde_json::from_str(&data)?;
    Ok(graph)
}

// Usage
let graph = load_graph("crates/sim-geo/test_data/small_city_graph.json")?;
println!("Loaded graph with {} nodes and {} edges",
    graph.nodes.len(),
    graph.edges.len()
);
```

### Building an Adjacency List

```rust
use std::collections::HashMap;

fn build_adjacency_list(graph: &CityGraph) -> HashMap<u32, Vec<(u32, f64)>> {
    let mut adj_list: HashMap<u32, Vec<(u32, f64)>> = HashMap::new();

    // Initialize all nodes
    for node in &graph.nodes {
        adj_list.insert(node.id, Vec::new());
    }

    // Add edges
    for edge in &graph.edges {
        adj_list.get_mut(&edge.from_node)
            .unwrap()
            .push((edge.to_node, edge.distance_m));
    }

    adj_list
}

// Usage
let graph = load_graph("crates/sim-geo/test_data/small_city_graph.json")?;
let adj_list = build_adjacency_list(&graph);

// Find neighbors of node 0
if let Some(neighbors) = adj_list.get(&0) {
    println!("Node 0 has {} neighbors:", neighbors.len());
    for (neighbor_id, distance) in neighbors {
        println!("  -> Node {}: {:.2}m", neighbor_id, distance);
    }
}
```

### Filtering by Road Type

```rust
fn filter_edges_by_road_type(graph: &CityGraph, road_type: &str) -> Vec<&GraphEdge> {
    graph.edges
        .iter()
        .filter(|e| e.road_type == road_type)
        .collect()
}

// Usage
let graph = load_graph("crates/sim-geo/test_data/small_city_graph.json")?;
let highways = filter_edges_by_road_type(&graph, "Highway");
println!("Found {} highway edges", highways.len());
```

## Loading Event Data

### Basic Event Loading

```rust
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Deserialize)]
struct CloudEvent {
    specversion: String,
    id: String,
    source: String,
    #[serde(rename = "type")]
    event_type: String,
    time: String,
    datacontenttype: String,
    data: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct EventStream {
    events: Vec<CloudEvent>,
}

fn load_events(path: &str) -> Result<EventStream, Box<dyn std::error::Error>> {
    let data = fs::read_to_string(path)?;
    let stream: EventStream = serde_json::from_str(&data)?;
    Ok(stream)
}

// Usage
let stream = load_events("crates/sim-telemetry/test_data/sample_events.json")?;
println!("Loaded {} events", stream.events.len());
```

### Filtering Events by Type

```rust
fn filter_events_by_type<'a>(
    stream: &'a EventStream,
    event_type: &str
) -> Vec<&'a CloudEvent> {
    stream.events
        .iter()
        .filter(|e| e.event_type.ends_with(event_type))
        .collect()
}

// Usage
let stream = load_events("crates/sim-telemetry/test_data/sample_events.json")?;
let ticks = filter_events_by_type(&stream, "agent_ticked");
println!("Found {} tick events", ticks.len());
```

### Grouping Events by Agent

```rust
use std::collections::HashMap;

fn group_events_by_agent(stream: &EventStream) -> HashMap<String, Vec<&CloudEvent>> {
    let mut groups: HashMap<String, Vec<&CloudEvent>> = HashMap::new();

    for event in &stream.events {
        if let Some(agent_id) = event.data.get("agent_id").and_then(|v| v.as_str()) {
            groups.entry(agent_id.to_string())
                .or_insert_with(Vec::new)
                .push(event);
        }
    }

    groups
}

// Usage
let stream = load_events("crates/sim-telemetry/test_data/sample_events.json")?;
let by_agent = group_events_by_agent(&stream);

for (agent_id, events) in &by_agent {
    println!("Agent {}: {} events", agent_id, events.len());
}
```

### Parsing Event Data by Type

```rust
#[derive(Debug, Deserialize)]
struct AgentTickedData {
    agent_id: String,
    tick: u64,
    state: AgentState,
    actions_taken: u32,
}

#[derive(Debug, Deserialize)]
struct AgentState {
    position: Position,
    energy: f64,
    velocity: f64,
}

#[derive(Debug, Deserialize)]
struct Position {
    x: f64,
    y: f64,
}

fn parse_tick_event(event: &CloudEvent) -> Result<AgentTickedData, serde_json::Error> {
    serde_json::from_value(event.data.clone())
}

// Usage
let stream = load_events("crates/sim-telemetry/test_data/sample_events.json")?;
for event in stream.events.iter().filter(|e| e.event_type.ends_with("agent_ticked")) {
    match parse_tick_event(event) {
        Ok(tick_data) => {
            println!("Agent {} at tick {}: pos=({:.1}, {:.1}), energy={:.1}",
                tick_data.agent_id,
                tick_data.tick,
                tick_data.state.position.x,
                tick_data.state.position.y,
                tick_data.state.energy
            );
        }
        Err(e) => eprintln!("Failed to parse tick event: {}", e),
    }
}
```

## Complete Integration Example

Here's a complete example that uses both graph and event data together:

```rust
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

// Assume all the struct definitions from above

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load graph data
    let graph = load_graph("crates/sim-geo/test_data/small_city_graph.json")?;
    println!("Loaded city graph: {} nodes, {} edges",
        graph.nodes.len(),
        graph.edges.len()
    );

    // Build routing structure
    let adj_list = build_adjacency_list(&graph);

    // Load event stream
    let stream = load_events("crates/sim-telemetry/test_data/sample_events.json")?;
    println!("Loaded event stream: {} events", stream.events.len());

    // Process events
    let mut agent_positions: HashMap<String, (f64, f64)> = HashMap::new();

    for event in &stream.events {
        if event.event_type.ends_with("agent_ticked") {
            if let Ok(tick_data) = parse_tick_event(event) {
                agent_positions.insert(
                    tick_data.agent_id.clone(),
                    (tick_data.state.position.x, tick_data.state.position.y)
                );
            }
        }
    }

    // Report
    println!("\nAgent final positions:");
    for (agent_id, (x, y)) in &agent_positions {
        println!("  {}: ({:.1}, {:.1})", agent_id, x, y);
    }

    // Analyze road network
    let mut road_type_stats: HashMap<String, (usize, f64)> = HashMap::new();
    for edge in &graph.edges {
        let entry = road_type_stats.entry(edge.road_type.clone())
            .or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += edge.distance_m;
    }

    println!("\nRoad network statistics:");
    for (road_type, (count, total_distance)) in &road_type_stats {
        println!("  {}: {} segments, {:.1}km total",
            road_type,
            count,
            total_distance / 1000.0
        );
    }

    Ok(())
}
```

## Testing Examples

### Unit Test with Small Graph

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_graph_structure() {
        let graph = load_graph("crates/sim-geo/test_data/small_city_graph.json")
            .expect("Failed to load graph");

        // Verify expected structure
        assert_eq!(graph.nodes.len(), 100);
        assert!(graph.edges.len() >= 180 && graph.edges.len() <= 220);

        // Verify all node IDs are unique
        let mut ids: Vec<_> = graph.nodes.iter().map(|n| n.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), graph.nodes.len());

        // Verify all edges reference valid nodes
        let node_ids: std::collections::HashSet<_> =
            graph.nodes.iter().map(|n| n.id).collect();
        for edge in &graph.edges {
            assert!(node_ids.contains(&edge.from_node));
            assert!(node_ids.contains(&edge.to_node));
            assert!(edge.distance_m > 0.0);
        }
    }

    #[test]
    fn test_event_stream_structure() {
        let stream = load_events("crates/sim-telemetry/test_data/sample_events.json")
            .expect("Failed to load events");

        assert_eq!(stream.events.len(), 100);

        // Verify all events are CloudEvents v1.0
        for event in &stream.events {
            assert_eq!(event.specversion, "1.0");
            assert!(!event.id.is_empty());
            assert!(event.source.starts_with("urn:sim:agent:"));
            assert!(event.event_type.starts_with("com.simulation."));
        }
    }
}
```

### Benchmark with Medium Graph

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_graph_loading(c: &mut Criterion) {
    c.bench_function("load_medium_graph", |b| {
        b.iter(|| {
            load_graph(black_box("crates/sim-geo/test_data/medium_city_graph.json"))
                .expect("Failed to load graph")
        });
    });
}

fn benchmark_adjacency_list(c: &mut Criterion) {
    let graph = load_graph("crates/sim-geo/test_data/medium_city_graph.json")
        .expect("Failed to load graph");

    c.bench_function("build_adjacency_list_1024_nodes", |b| {
        b.iter(|| build_adjacency_list(black_box(&graph)));
    });
}

criterion_group!(benches, benchmark_graph_loading, benchmark_adjacency_list);
criterion_main!(benches);
```

## Performance Notes

- **Small graph**: Fast loading (<1ms), suitable for unit tests
- **Medium graph**: ~10-20ms loading, use for integration tests and benchmarks
- **Event stream**: ~5-10ms loading, suitable for telemetry pipeline tests

## See Also

- Main documentation: `/test_data/README.md`
- Graph schema details: Phase 7 documentation
- Event schema details: Phase 8 documentation
- CloudEvents spec: https://github.com/cloudevents/spec/blob/v1.0/spec.md
