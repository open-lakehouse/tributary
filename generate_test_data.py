#!/usr/bin/env python3
"""
Test Data Generator for sim-geo and sim-telemetry
Generates graph data and event streams for testing.
"""

import json
import math
import random
from datetime import datetime, timedelta
from typing import List, Dict, Any
import uuid

# San Francisco coordinates as base
BASE_LAT = 37.7749
BASE_LON = -122.4194
LAT_STEP = 0.001  # ~111 meters
LON_STEP = 0.001  # ~111 meters at SF latitude

ROAD_TYPES = ["Highway", "Arterial", "Residential", "ServiceRoad"]


def generate_grid_graph(grid_size: int, filename: str):
    """Generate a grid-based city graph."""
    nodes = []
    edges = []
    edge_id = 0

    # Create nodes in a grid
    for i in range(grid_size):
        for j in range(grid_size):
            node_id = i * grid_size + j
            lat = BASE_LAT + (i * LAT_STEP)
            lon = BASE_LON + (j * LON_STEP)
            nodes.append({
                "id": node_id,
                "lat": round(lat, 6),
                "lon": round(lon, 6)
            })

    # Create edges connecting neighbors
    for i in range(grid_size):
        for j in range(grid_size):
            node_id = i * grid_size + j
            node_lat = BASE_LAT + (i * LAT_STEP)
            node_lon = BASE_LON + (j * LON_STEP)

            # Connect to right neighbor
            if j < grid_size - 1:
                to_node = node_id + 1
                to_lat = node_lat
                to_lon = node_lon + LON_STEP
                distance = haversine_distance(node_lat, node_lon, to_lat, to_lon)
                road_type = select_road_type(i, j, i, j+1, grid_size)

                edges.append({
                    "id": edge_id,
                    "from_node": node_id,
                    "to_node": to_node,
                    "distance_m": round(distance, 2),
                    "road_type": road_type,
                    "geometry": {
                        "type": "LineString",
                        "coordinates": [
                            [round(node_lon, 6), round(node_lat, 6)],
                            [round(to_lon, 6), round(to_lat, 6)]
                        ]
                    }
                })
                edge_id += 1

            # Connect to bottom neighbor
            if i < grid_size - 1:
                to_node = node_id + grid_size
                to_lat = node_lat + LAT_STEP
                to_lon = node_lon
                distance = haversine_distance(node_lat, node_lon, to_lat, to_lon)
                road_type = select_road_type(i, j, i+1, j, grid_size)

                edges.append({
                    "id": edge_id,
                    "from_node": node_id,
                    "to_node": to_node,
                    "distance_m": round(distance, 2),
                    "road_type": road_type,
                    "geometry": {
                        "type": "LineString",
                        "coordinates": [
                            [round(node_lon, 6), round(node_lat, 6)],
                            [round(to_lon, 6), round(to_lat, 6)]
                        ]
                    }
                })
                edge_id += 1

            # Add some diagonal connections for variety (about 10% of nodes)
            if random.random() < 0.1 and i < grid_size - 1 and j < grid_size - 1:
                to_node = node_id + grid_size + 1
                to_lat = node_lat + LAT_STEP
                to_lon = node_lon + LON_STEP
                distance = haversine_distance(node_lat, node_lon, to_lat, to_lon)

                edges.append({
                    "id": edge_id,
                    "from_node": node_id,
                    "to_node": to_node,
                    "distance_m": round(distance, 2),
                    "road_type": "ServiceRoad",
                    "geometry": {
                        "type": "LineString",
                        "coordinates": [
                            [round(node_lon, 6), round(node_lat, 6)],
                            [round(to_lon, 6), round(to_lat, 6)]
                        ]
                    }
                })
                edge_id += 1

    graph = {
        "_comment": f"Generated {grid_size}x{grid_size} grid graph with {len(nodes)} nodes and {len(edges)} edges",
        "_schema": {
            "nodes": "Array of graph nodes with id, lat, lon",
            "edges": "Array of directed edges with id, from_node, to_node, distance_m, road_type, geometry (GeoJSON LineString)"
        },
        "nodes": nodes,
        "edges": edges
    }

    with open(filename, 'w') as f:
        json.dump(graph, f, indent=2)

    return len(nodes), len(edges)


def haversine_distance(lat1: float, lon1: float, lat2: float, lon2: float) -> float:
    """Calculate distance in meters between two lat/lon points."""
    R = 6371000  # Earth radius in meters

    phi1 = math.radians(lat1)
    phi2 = math.radians(lat2)
    delta_phi = math.radians(lat2 - lat1)
    delta_lambda = math.radians(lon2 - lon1)

    a = math.sin(delta_phi/2)**2 + math.cos(phi1) * math.cos(phi2) * math.sin(delta_lambda/2)**2
    c = 2 * math.atan2(math.sqrt(a), math.sqrt(1-a))

    return R * c


def select_road_type(i1: int, j1: int, i2: int, j2: int, grid_size: int) -> str:
    """Select road type based on position in grid."""
    # Edges of grid are highways
    if i1 == 0 or i1 == grid_size - 1 or j1 == 0 or j1 == grid_size - 1:
        return "Highway"
    # Every 5th row/column is arterial
    elif i1 % 5 == 0 or j1 % 5 == 0:
        return "Arterial"
    # Most are residential
    elif random.random() < 0.8:
        return "Residential"
    else:
        return "ServiceRoad"


def generate_sample_events(num_events: int, filename: str):
    """Generate sample CloudEvents for telemetry testing."""
    events = []
    base_time = datetime.utcnow() - timedelta(hours=1)

    event_types = [
        "agent_created",
        "agent_ticked",
        "resource_allocated",
        "workflow_step_completed"
    ]

    agent_ids = [f"agent-{i:04d}" for i in range(10)]

    for i in range(num_events):
        event_type = random.choice(event_types)
        agent_id = random.choice(agent_ids)
        event_time = base_time + timedelta(seconds=i * 36)  # Events every 36 seconds

        event_data = generate_event_data(event_type, agent_id, i)

        event = {
            "specversion": "1.0",
            "id": str(uuid.uuid4()),
            "source": f"urn:sim:agent:{agent_id}",
            "type": f"com.simulation.{event_type}",
            "time": event_time.isoformat() + "Z",
            "datacontenttype": "application/json",
            "data": event_data
        }

        events.append(event)

    output = {
        "_comment": f"Sample CloudEvents v1.0 stream with {num_events} events",
        "_schema": {
            "specversion": "CloudEvents version (1.0)",
            "id": "Unique event identifier (UUID)",
            "source": "Event source URI",
            "type": "Event type identifier",
            "time": "Event timestamp (RFC3339)",
            "datacontenttype": "Content type of data field",
            "data": "Event payload (varies by type)"
        },
        "events": events
    }

    with open(filename, 'w') as f:
        json.dump(output, f, indent=2)

    return len(events)


def generate_event_data(event_type: str, agent_id: str, sequence: int) -> Dict[str, Any]:
    """Generate event-specific data payload."""
    if event_type == "agent_created":
        return {
            "agent_id": agent_id,
            "agent_type": random.choice(["worker", "coordinator", "optimizer"]),
            "initial_state": {
                "position": {"x": random.uniform(0, 1000), "y": random.uniform(0, 1000)},
                "energy": 100.0
            }
        }

    elif event_type == "agent_ticked":
        return {
            "agent_id": agent_id,
            "tick": sequence,
            "state": {
                "position": {"x": random.uniform(0, 1000), "y": random.uniform(0, 1000)},
                "energy": random.uniform(50, 100),
                "velocity": random.uniform(0, 10)
            },
            "actions_taken": random.randint(0, 5)
        }

    elif event_type == "resource_allocated":
        return {
            "agent_id": agent_id,
            "resource_type": random.choice(["cpu", "memory", "network", "storage"]),
            "amount": random.uniform(10, 100),
            "allocation_id": str(uuid.uuid4())
        }

    elif event_type == "workflow_step_completed":
        return {
            "agent_id": agent_id,
            "workflow_id": f"workflow-{sequence % 5}",
            "step_name": random.choice(["initialize", "process", "validate", "finalize"]),
            "duration_ms": random.randint(50, 500),
            "success": random.random() < 0.95
        }

    return {}


def main():
    print("Generating test data files...")

    # Generate small city graph
    print("\n1. Generating small city graph (10x10 grid)...")
    small_file = "/Users/robert.pack/code/management/crates/sim-geo/test_data/small_city_graph.json"
    small_nodes, small_edges = generate_grid_graph(10, small_file)
    print(f"   Created: {small_file}")
    print(f"   Nodes: {small_nodes}, Edges: {small_edges}")

    # Generate medium city graph
    print("\n2. Generating medium city graph (32x32 grid for ~1000 nodes)...")
    medium_file = "/Users/robert.pack/code/management/crates/sim-geo/test_data/medium_city_graph.json"
    medium_nodes, medium_edges = generate_grid_graph(32, medium_file)
    print(f"   Created: {medium_file}")
    print(f"   Nodes: {medium_nodes}, Edges: {medium_edges}")

    # Generate sample events
    print("\n3. Generating sample event stream...")
    events_file = "/Users/robert.pack/code/management/crates/sim-telemetry/test_data/sample_events.json"
    num_events = generate_sample_events(100, events_file)
    print(f"   Created: {events_file}")
    print(f"   Events: {num_events}")

    print("\n✓ All test data files generated successfully!")


if __name__ == "__main__":
    main()
