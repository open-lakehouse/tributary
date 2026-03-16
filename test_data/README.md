# Test Data Documentation

This directory contains test data files for the simulation framework, specifically for Phase 7 (road network testing) and Phase 8 (telemetry testing).

## Overview

- **Graph Data**: Located in `crates/sim-geo/test_data/`
  - Small city graph for unit testing
  - Medium city graph for performance benchmarking

- **Event Data**: Located in `crates/sim-telemetry/test_data/`
  - Sample CloudEvents stream for telemetry testing

## Graph JSON Schema

### File Structure

```json
{
  "_comment": "Descriptive comment about the graph",
  "_schema": {
    "nodes": "Array of graph nodes with id, lat, lon",
    "edges": "Array of directed edges with id, from_node, to_node, distance_m, road_type, geometry (GeoJSON LineString)"
  },
  "nodes": [ /* node array */ ],
  "edges": [ /* edge array */ ]
}
```

### Node Schema

Each node represents an intersection or location in the road network:

```json
{
  "id": 0,              // Unique integer identifier
  "lat": 37.7749,       // Latitude (decimal degrees)
  "lon": -122.4194      // Longitude (decimal degrees)
}
```

### Edge Schema

Each edge represents a directed road segment:

```json
{
  "id": 0,                    // Unique integer identifier
  "from_node": 0,             // Source node ID
  "to_node": 1,               // Destination node ID
  "distance_m": 111.19,       // Distance in meters (haversine)
  "road_type": "Residential", // Road classification
  "geometry": {               // GeoJSON LineString
    "type": "LineString",
    "coordinates": [
      [-122.4194, 37.7749],   // [lon, lat] of from_node
      [-122.4184, 37.7749]    // [lon, lat] of to_node
    ]
  }
}
```

### Road Types

The graph includes four road types with different characteristics:

- **Highway**: Major routes, typically on grid edges, higher speed limits
- **Arterial**: Major streets, every 5th row/column in grid
- **Residential**: Most common, lower speed limits
- **ServiceRoad**: Minor roads and alleys, occasional diagonal connections

## Event JSON Schema

### File Structure

```json
{
  "_comment": "Descriptive comment about the event stream",
  "_schema": { /* schema documentation */ },
  "events": [ /* CloudEvents array */ ]
}
```

### CloudEvent Schema (v1.0)

Each event follows the [CloudEvents v1.0 specification](https://github.com/cloudevents/spec/blob/v1.0/spec.md):

```json
{
  "specversion": "1.0",                              // CloudEvents version
  "id": "uuid-string",                               // Unique event ID (UUID)
  "source": "urn:sim:agent:agent-0001",             // Event source URI
  "type": "com.simulation.agent_ticked",            // Event type identifier
  "time": "2026-03-16T10:30:00.000Z",              // ISO 8601 timestamp
  "datacontenttype": "application/json",            // Content type
  "data": { /* event-specific payload */ }          // Event data
}
```

### Event Types

The sample data includes four event types:

#### 1. agent_created

Emitted when a new agent is created in the simulation.

```json
{
  "type": "com.simulation.agent_created",
  "data": {
    "agent_id": "agent-0001",
    "agent_type": "worker",  // worker | coordinator | optimizer
    "initial_state": {
      "position": {"x": 100.5, "y": 200.3},
      "energy": 100.0
    }
  }
}
```

#### 2. agent_ticked

Emitted on each simulation tick for an agent.

```json
{
  "type": "com.simulation.agent_ticked",
  "data": {
    "agent_id": "agent-0001",
    "tick": 42,
    "state": {
      "position": {"x": 150.2, "y": 220.8},
      "energy": 87.5,
      "velocity": 5.2
    },
    "actions_taken": 3
  }
}
```

#### 3. resource_allocated

Emitted when resources are allocated to an agent.

```json
{
  "type": "com.simulation.resource_allocated",
  "data": {
    "agent_id": "agent-0001",
    "resource_type": "cpu",  // cpu | memory | network | storage
    "amount": 25.5,
    "allocation_id": "uuid-string"
  }
}
```

#### 4. workflow_step_completed

Emitted when an agent completes a workflow step.

```json
{
  "type": "com.simulation.workflow_step_completed",
  "data": {
    "agent_id": "agent-0001",
    "workflow_id": "workflow-1",
    "step_name": "process",  // initialize | process | validate | finalize
    "duration_ms": 250,
    "success": true
  }
}
```

## Test Data Files

### 1. small_city_graph.json

- **Location**: `crates/sim-geo/test_data/small_city_graph.json`
- **Size**: ~74 KB
- **Purpose**: Unit testing and development
- **Structure**: 10x10 grid
- **Nodes**: 100
- **Edges**: ~188 (includes some diagonal connections)
- **Coverage**: San Francisco area (~1.1 km²)

**Usage Example**:

```rust
use std::fs;
use serde_json::Value;

let data = fs::read_to_string("crates/sim-geo/test_data/small_city_graph.json")?;
let graph: Value = serde_json::from_str(&data)?;

let nodes = &graph["nodes"];
let edges = &graph["edges"];

println!("Loaded {} nodes and {} edges",
    nodes.as_array().unwrap().len(),
    edges.as_array().unwrap().len()
);
```

### 2. medium_city_graph.json

- **Location**: `crates/sim-geo/test_data/medium_city_graph.json`
- **Size**: ~815 KB
- **Purpose**: Performance benchmarking and stress testing
- **Structure**: 32x32 grid
- **Nodes**: 1024
- **Edges**: ~2077 (includes some diagonal connections)
- **Coverage**: San Francisco area (~11.3 km²)

**Usage Example**:

```rust
// Benchmark pathfinding on larger graph
use std::time::Instant;

let data = fs::read_to_string("crates/sim-geo/test_data/medium_city_graph.json")?;
let graph: Value = serde_json::from_str(&data)?;

let start = Instant::now();
// Run pathfinding algorithm
let duration = start.elapsed();

println!("Pathfinding on {} nodes took {:?}",
    graph["nodes"].as_array().unwrap().len(),
    duration
);
```

### 3. sample_events.json

- **Location**: `crates/sim-telemetry/test_data/sample_events.json`
- **Size**: ~51 KB
- **Purpose**: Telemetry system testing
- **Events**: 100
- **Time Range**: 1 hour (events every 36 seconds)
- **Event Types**: agent_created, agent_ticked, resource_allocated, workflow_step_completed
- **Agents**: 10 unique agents (agent-0000 through agent-0009)

**Usage Example**:

```rust
use std::fs;
use serde_json::Value;

let data = fs::read_to_string("crates/sim-telemetry/test_data/sample_events.json")?;
let events_file: Value = serde_json::from_str(&data)?;
let events = events_file["events"].as_array().unwrap();

// Filter events by type
let ticks: Vec<_> = events.iter()
    .filter(|e| e["type"] == "com.simulation.agent_ticked")
    .collect();

println!("Found {} tick events out of {} total events",
    ticks.len(),
    events.len()
);

// Process events chronologically
for event in events {
    let event_type = event["type"].as_str().unwrap();
    let timestamp = event["time"].as_str().unwrap();
    let agent_id = event["data"]["agent_id"].as_str().unwrap();

    println!("{} - {} - {}", timestamp, agent_id, event_type);
}
```

## Generation

The test data was generated using `generate_test_data.py` in the repository root. To regenerate:

```bash
python3 generate_test_data.py
```

### Generation Algorithm

**Graph Generation**:
- Nodes placed in a regular grid with ~111m spacing
- Edges connect horizontal and vertical neighbors
- 10% of nodes have diagonal connections for variety
- Road types assigned based on position:
  - Grid edges: Highway
  - Every 5th row/column: Arterial
  - 80% of others: Residential
  - 20% of others: ServiceRoad
- Distances calculated using Haversine formula
- Geometry encoded as GeoJSON LineString

**Event Generation**:
- Events distributed across 10 agents
- 100 events over 1 hour period (36s intervals)
- Random mix of all four event types
- Realistic data values (energy 50-100, velocity 0-10, etc.)
- UUIDs for event IDs and allocation IDs
- ISO 8601 timestamps with 'Z' suffix (UTC)

## Validation

All files are valid JSON and conform to their respective schemas. Validate with:

```bash
# Validate JSON syntax
python3 -m json.tool crates/sim-geo/test_data/small_city_graph.json > /dev/null
python3 -m json.tool crates/sim-geo/test_data/medium_city_graph.json > /dev/null
python3 -m json.tool crates/sim-telemetry/test_data/sample_events.json > /dev/null
```

## Design Decisions

1. **Grid-based graphs**: Simplified, predictable topology for testing. Real-world graphs can be added later.

2. **GeoJSON geometry**: Standard format for geographic data, compatible with mapping libraries.

3. **CloudEvents v1.0**: Industry standard for event data, ensures interoperability.

4. **Embedded schema documentation**: `_comment` and `_schema` fields in JSON for self-documentation.

5. **Realistic scales**:
   - Small graph fits in memory easily, fast to process
   - Medium graph ~10x larger, suitable for performance testing
   - Event stream covers realistic simulation timeline

6. **Mixed road types**: Enables testing of routing algorithms that consider road classification.

7. **Diagonal connections**: Adds graph complexity beyond simple grid, tests edge cases.

8. **Multiple event types**: Covers common simulation event patterns (creation, ticking, resource management, workflows).

## Future Enhancements

Potential additions for future phases:

- Real-world road network data (OSM import)
- Traffic signal and intersection data
- Elevation profiles for edges
- One-way streets and turn restrictions
- Compressed event streams (gzip, parquet)
- Time-series event aggregations
- Multi-day event sequences
- Error and exception events
- Performance profiling events
