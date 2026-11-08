//! Road network graph data structures and operations
//!
//! This module provides the core graph data structures for representing road networks
//! in agent-based simulations. The graph is represented as a directed graph with:
//!
//! - **Nodes**: Intersections or junction points with geographic coordinates
//! - **Edges**: Road segments connecting nodes with geometric paths and properties
//! - **Adjacency**: Efficient lookups of outgoing edges from each node
//! - **I/O**: JSON serialization/deserialization for loading and saving networks
//!
//! # Example
//!
//! ```
//! use sim_geo::graph::{RoadNetwork, RoadNode, RoadEdge, RoadType};
//! use geo::Point;
//! use geo::LineString;
//!
//! // Create an empty road network
//! let mut network = RoadNetwork::new();
//!
//! // Add intersection nodes
//! let node1 = RoadNode {
//!     id: 1,
//!     position: Point::new(-122.4194, 37.7749),
//! };
//! let node2 = RoadNode {
//!     id: 2,
//!     position: Point::new(-122.4184, 37.7759),
//! };
//!
//! network.add_node(node1.clone());
//! network.add_node(node2.clone());
//!
//! // Add road segment connecting them
//! let edge = RoadEdge {
//!     id: 1,
//!     from_node: 1,
//!     to_node: 2,
//!     distance_m: 150.0,
//!     geometry: LineString::from(vec![
//!         (-122.4194, 37.7749),
//!         (-122.4184, 37.7759),
//!     ]),
//!     road_type: RoadType::Residential,
//!     speed_limit_mps: RoadType::Residential.default_speed_mps(),
//! };
//!
//! network.add_edge(edge);
//!
//! // Query the network
//! assert_eq!(network.node_count(), 2);
//! assert_eq!(network.edge_count(), 1);
//! assert_eq!(network.get_neighbors(1).len(), 1);
//!
//! // Save and load from JSON
//! let path = std::env::temp_dir().join(format!("sim-geo-doc-{}.json", std::process::id()));
//! network.save_to_file(&path).unwrap();
//! let loaded = RoadNetwork::load_from_file(&path).unwrap();
//! assert_eq!(loaded.node_count(), 2);
//! # std::fs::remove_file(&path).unwrap();
//! ```

mod io;
pub mod routing;

// Re-export I/O types
pub use io::GraphError;
pub use routing::{Route, RoutingGraph};

use geo::{LineString, Point};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::OnceLock;

/// Unique identifier for a road network node (intersection/junction)
pub type NodeId = u64;

/// Unique identifier for a road network edge (road segment)
pub type EdgeId = u64;

/// A directed graph representing a road network
///
/// The network maintains three primary data structures:
/// - `nodes`: Map of node IDs to node data (positions)
/// - `edges`: Map of edge IDs to edge data (geometry, properties)
/// - `adjacency`: Map of node IDs to outgoing edge IDs for efficient traversal
/// - `fast_graph_cache`: Cached preprocessed graph for fast routing queries
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoadNetwork {
    /// All nodes in the network indexed by ID
    pub nodes: HashMap<NodeId, RoadNode>,
    /// All edges in the network indexed by ID
    pub edges: HashMap<EdgeId, RoadEdge>,
    /// Adjacency list mapping each node to its outgoing edges
    pub adjacency: HashMap<NodeId, Vec<EdgeId>>,
    /// Cached routing graph for efficient routing (not serialized)
    #[serde(skip)]
    fast_graph_cache: OnceLock<RoutingGraph>,
}

/// A node in the road network representing an intersection or junction point
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoadNode {
    /// Unique identifier for this node
    pub id: NodeId,
    /// Geographic position (longitude, latitude)
    pub position: Point<f64>,
}

/// A directed edge in the road network representing a road segment
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoadEdge {
    /// Unique identifier for this edge
    pub id: EdgeId,
    /// Source node ID
    pub from_node: NodeId,
    /// Destination node ID
    pub to_node: NodeId,
    /// Length of the road segment in meters
    pub distance_m: f64,
    /// Detailed geometric path of the road segment
    pub geometry: LineString<f64>,
    /// Classification of the road
    pub road_type: RoadType,
    /// Speed limit in meters per second
    pub speed_limit_mps: f64,
}

/// Classification of road types with associated default speed limits
///
/// Speed limits are defined in meters per second (m/s) and represent
/// typical travel speeds for each road class.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RoadType {
    /// Major high-speed road (e.g., freeway, motorway)
    Highway,
    /// Major urban road (e.g., main street, boulevard)
    Arterial,
    /// Local neighborhood street
    Residential,
    /// Service or access road (e.g., parking lot, alley)
    ServiceRoad,
}

impl RoadNetwork {
    /// Creates a new empty road network
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::graph::RoadNetwork;
    ///
    /// let network = RoadNetwork::new();
    /// assert_eq!(network.node_count(), 0);
    /// assert_eq!(network.edge_count(), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            adjacency: HashMap::new(),
            fast_graph_cache: OnceLock::new(),
        }
    }

    /// Adds a node to the network
    ///
    /// If a node with the same ID already exists, it will be replaced.
    /// The adjacency list entry for the node is created if it doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `node` - The node to add to the network
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::graph::{RoadNetwork, RoadNode};
    /// use geo::Point;
    ///
    /// let mut network = RoadNetwork::new();
    /// let node = RoadNode {
    ///     id: 1,
    ///     position: Point::new(-122.4194, 37.7749),
    /// };
    /// network.add_node(node);
    /// assert_eq!(network.node_count(), 1);
    /// ```
    pub fn add_node(&mut self, node: RoadNode) {
        let node_id = node.id;
        self.nodes.insert(node_id, node);
        self.adjacency.entry(node_id).or_default();
        self.invalidate_route_cache();
    }

    /// Drop the preprocessed routing graph so the next query rebuilds it
    ///
    /// [`add_node`](Self::add_node) and [`add_edge`](Self::add_edge) call this
    /// for you. Call it yourself after editing `nodes`, `edges` or
    /// `adjacency` directly.
    pub fn invalidate_route_cache(&mut self) {
        self.fast_graph_cache = OnceLock::new();
    }

    /// Adds an edge to the network
    ///
    /// The edge is added to the edges map and registered in the adjacency list
    /// for the source node. If the edge ID already exists, it will be replaced.
    ///
    /// Note: This method does not validate that the from_node and to_node exist.
    /// Use `get_node()` to check node existence if needed.
    ///
    /// # Arguments
    ///
    /// * `edge` - The edge to add to the network
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::graph::{RoadNetwork, RoadNode, RoadEdge, RoadType};
    /// use geo::{Point, LineString};
    ///
    /// let mut network = RoadNetwork::new();
    /// network.add_node(RoadNode { id: 1, position: Point::new(0.0, 0.0) });
    /// network.add_node(RoadNode { id: 2, position: Point::new(1.0, 1.0) });
    ///
    /// let edge = RoadEdge {
    ///     id: 1,
    ///     from_node: 1,
    ///     to_node: 2,
    ///     distance_m: 100.0,
    ///     geometry: LineString::from(vec![(0.0, 0.0), (1.0, 1.0)]),
    ///     road_type: RoadType::Residential,
    ///     speed_limit_mps: 8.0,
    /// };
    /// network.add_edge(edge);
    /// assert_eq!(network.edge_count(), 1);
    /// ```
    pub fn add_edge(&mut self, edge: RoadEdge) {
        let edge_id = edge.id;
        let from_node = edge.from_node;

        // Add edge to edges map
        self.edges.insert(edge_id, edge);

        // Add edge to adjacency list for source node
        self.adjacency.entry(from_node).or_default().push(edge_id);
        self.invalidate_route_cache();
    }

    /// Retrieves a node by ID
    ///
    /// # Arguments
    ///
    /// * `node_id` - The ID of the node to retrieve
    ///
    /// # Returns
    ///
    /// An `Option` containing a reference to the node if it exists, `None` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::graph::{RoadNetwork, RoadNode};
    /// use geo::Point;
    ///
    /// let mut network = RoadNetwork::new();
    /// let node = RoadNode { id: 42, position: Point::new(0.0, 0.0) };
    /// network.add_node(node);
    ///
    /// assert!(network.get_node(42).is_some());
    /// assert!(network.get_node(999).is_none());
    /// ```
    pub fn get_node(&self, node_id: NodeId) -> Option<&RoadNode> {
        self.nodes.get(&node_id)
    }

    /// Retrieves an edge by ID
    ///
    /// # Arguments
    ///
    /// * `edge_id` - The ID of the edge to retrieve
    ///
    /// # Returns
    ///
    /// An `Option` containing a reference to the edge if it exists, `None` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::graph::{RoadNetwork, RoadNode, RoadEdge, RoadType};
    /// use geo::{Point, LineString};
    ///
    /// let mut network = RoadNetwork::new();
    /// network.add_node(RoadNode { id: 1, position: Point::new(0.0, 0.0) });
    /// network.add_node(RoadNode { id: 2, position: Point::new(1.0, 1.0) });
    ///
    /// let edge = RoadEdge {
    ///     id: 100,
    ///     from_node: 1,
    ///     to_node: 2,
    ///     distance_m: 100.0,
    ///     geometry: LineString::from(vec![(0.0, 0.0), (1.0, 1.0)]),
    ///     road_type: RoadType::Highway,
    ///     speed_limit_mps: 30.0,
    /// };
    /// network.add_edge(edge);
    ///
    /// assert!(network.get_edge(100).is_some());
    /// assert!(network.get_edge(999).is_none());
    /// ```
    pub fn get_edge(&self, edge_id: EdgeId) -> Option<&RoadEdge> {
        self.edges.get(&edge_id)
    }

    /// Gets all outgoing edges from a node
    ///
    /// Returns a slice of edge IDs for all edges that originate from the given node.
    /// Returns an empty slice if the node doesn't exist or has no outgoing edges.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The ID of the source node
    ///
    /// # Returns
    ///
    /// A slice of edge IDs representing outgoing edges from the node
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::graph::{RoadNetwork, RoadNode, RoadEdge, RoadType};
    /// use geo::{Point, LineString};
    ///
    /// let mut network = RoadNetwork::new();
    /// network.add_node(RoadNode { id: 1, position: Point::new(0.0, 0.0) });
    /// network.add_node(RoadNode { id: 2, position: Point::new(1.0, 0.0) });
    /// network.add_node(RoadNode { id: 3, position: Point::new(0.0, 1.0) });
    ///
    /// // Add two edges from node 1
    /// network.add_edge(RoadEdge {
    ///     id: 1,
    ///     from_node: 1,
    ///     to_node: 2,
    ///     distance_m: 100.0,
    ///     geometry: LineString::from(vec![(0.0, 0.0), (1.0, 0.0)]),
    ///     road_type: RoadType::Residential,
    ///     speed_limit_mps: 8.0,
    /// });
    /// network.add_edge(RoadEdge {
    ///     id: 2,
    ///     from_node: 1,
    ///     to_node: 3,
    ///     distance_m: 100.0,
    ///     geometry: LineString::from(vec![(0.0, 0.0), (0.0, 1.0)]),
    ///     road_type: RoadType::Residential,
    ///     speed_limit_mps: 8.0,
    /// });
    ///
    /// assert_eq!(network.get_neighbors(1).len(), 2);
    /// assert_eq!(network.get_neighbors(2).len(), 0);
    /// ```
    pub fn get_neighbors(&self, node_id: NodeId) -> &[EdgeId] {
        self.adjacency
            .get(&node_id)
            .map(|edges| edges.as_slice())
            .unwrap_or(&[])
    }

    /// Returns the total number of nodes in the network
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::graph::{RoadNetwork, RoadNode};
    /// use geo::Point;
    ///
    /// let mut network = RoadNetwork::new();
    /// assert_eq!(network.node_count(), 0);
    ///
    /// network.add_node(RoadNode { id: 1, position: Point::new(0.0, 0.0) });
    /// assert_eq!(network.node_count(), 1);
    /// ```
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns the total number of edges in the network
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::graph::{RoadNetwork, RoadNode, RoadEdge, RoadType};
    /// use geo::{Point, LineString};
    ///
    /// let mut network = RoadNetwork::new();
    /// assert_eq!(network.edge_count(), 0);
    ///
    /// network.add_node(RoadNode { id: 1, position: Point::new(0.0, 0.0) });
    /// network.add_node(RoadNode { id: 2, position: Point::new(1.0, 1.0) });
    /// network.add_edge(RoadEdge {
    ///     id: 1,
    ///     from_node: 1,
    ///     to_node: 2,
    ///     distance_m: 100.0,
    ///     geometry: LineString::from(vec![(0.0, 0.0), (1.0, 1.0)]),
    ///     road_type: RoadType::Arterial,
    ///     speed_limit_mps: 15.0,
    /// });
    /// assert_eq!(network.edge_count(), 1);
    /// ```
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

impl Default for RoadNetwork {
    fn default() -> Self {
        Self::new()
    }
}

impl RoadType {
    /// Returns the default speed limit for this road type in meters per second
    ///
    /// # Returns
    ///
    /// - `Highway`: 30 m/s (~108 km/h, ~67 mph)
    /// - `Arterial`: 15 m/s (~54 km/h, ~34 mph)
    /// - `Residential`: 8 m/s (~29 km/h, ~18 mph)
    /// - `ServiceRoad`: 5 m/s (~18 km/h, ~11 mph)
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::graph::RoadType;
    ///
    /// assert_eq!(RoadType::Highway.default_speed_mps(), 30.0);
    /// assert_eq!(RoadType::Arterial.default_speed_mps(), 15.0);
    /// assert_eq!(RoadType::Residential.default_speed_mps(), 8.0);
    /// assert_eq!(RoadType::ServiceRoad.default_speed_mps(), 5.0);
    /// ```
    pub fn default_speed_mps(&self) -> f64 {
        match self {
            RoadType::Highway => 30.0,    // ~108 km/h
            RoadType::Arterial => 15.0,   // ~54 km/h
            RoadType::Residential => 8.0, // ~29 km/h
            RoadType::ServiceRoad => 5.0, // ~18 km/h
        }
    }
}

impl fmt::Display for RoadType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RoadType::Highway => write!(f, "Highway"),
            RoadType::Arterial => write!(f, "Arterial"),
            RoadType::Residential => write!(f, "Residential"),
            RoadType::ServiceRoad => write!(f, "Service Road"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_network() {
        let network = RoadNetwork::new();
        assert_eq!(network.node_count(), 0);
        assert_eq!(network.edge_count(), 0);
        assert_eq!(network.get_neighbors(1).len(), 0);
    }

    #[test]
    fn test_add_nodes() {
        let mut network = RoadNetwork::new();

        let node1 = RoadNode {
            id: 1,
            position: Point::new(-122.4194, 37.7749),
        };
        let node2 = RoadNode {
            id: 2,
            position: Point::new(-122.4184, 37.7759),
        };

        network.add_node(node1.clone());
        network.add_node(node2.clone());

        assert_eq!(network.node_count(), 2);
        assert!(network.get_node(1).is_some());
        assert!(network.get_node(2).is_some());
        assert!(network.get_node(3).is_none());

        // Verify node positions
        let retrieved = network.get_node(1).unwrap();
        assert_eq!(retrieved.id, 1);
        assert_eq!(retrieved.position.x(), -122.4194);
        assert_eq!(retrieved.position.y(), 37.7749);
    }

    #[test]
    fn test_add_edges_and_adjacency() {
        let mut network = RoadNetwork::new();

        // Add nodes
        network.add_node(RoadNode {
            id: 1,
            position: Point::new(0.0, 0.0),
        });
        network.add_node(RoadNode {
            id: 2,
            position: Point::new(1.0, 0.0),
        });
        network.add_node(RoadNode {
            id: 3,
            position: Point::new(0.0, 1.0),
        });

        // Add edges from node 1 to nodes 2 and 3
        let edge1 = RoadEdge {
            id: 1,
            from_node: 1,
            to_node: 2,
            distance_m: 100.0,
            geometry: LineString::from(vec![(0.0, 0.0), (1.0, 0.0)]),
            road_type: RoadType::Residential,
            speed_limit_mps: 8.0,
        };
        let edge2 = RoadEdge {
            id: 2,
            from_node: 1,
            to_node: 3,
            distance_m: 100.0,
            geometry: LineString::from(vec![(0.0, 0.0), (0.0, 1.0)]),
            road_type: RoadType::Arterial,
            speed_limit_mps: 15.0,
        };
        let edge3 = RoadEdge {
            id: 3,
            from_node: 2,
            to_node: 3,
            distance_m: 141.4,
            geometry: LineString::from(vec![(1.0, 0.0), (0.0, 1.0)]),
            road_type: RoadType::Highway,
            speed_limit_mps: 30.0,
        };

        network.add_edge(edge1);
        network.add_edge(edge2);
        network.add_edge(edge3);

        assert_eq!(network.edge_count(), 3);

        // Check adjacency
        let neighbors_1 = network.get_neighbors(1);
        assert_eq!(neighbors_1.len(), 2);
        assert!(neighbors_1.contains(&1));
        assert!(neighbors_1.contains(&2));

        let neighbors_2 = network.get_neighbors(2);
        assert_eq!(neighbors_2.len(), 1);
        assert!(neighbors_2.contains(&3));

        let neighbors_3 = network.get_neighbors(3);
        assert_eq!(neighbors_3.len(), 0);
    }

    #[test]
    fn test_duplicate_node_replacement() {
        let mut network = RoadNetwork::new();

        let node1 = RoadNode {
            id: 1,
            position: Point::new(0.0, 0.0),
        };
        let node1_updated = RoadNode {
            id: 1,
            position: Point::new(5.0, 5.0),
        };

        network.add_node(node1);
        assert_eq!(network.node_count(), 1);
        assert_eq!(network.get_node(1).unwrap().position.x(), 0.0);

        // Adding node with same ID should replace it
        network.add_node(node1_updated);
        assert_eq!(network.node_count(), 1);
        assert_eq!(network.get_node(1).unwrap().position.x(), 5.0);
    }

    #[test]
    fn test_edge_with_nonexistent_nodes() {
        let mut network = RoadNetwork::new();

        // Add edge without adding nodes first
        let edge = RoadEdge {
            id: 1,
            from_node: 99,
            to_node: 100,
            distance_m: 50.0,
            geometry: LineString::from(vec![(0.0, 0.0), (1.0, 1.0)]),
            road_type: RoadType::ServiceRoad,
            speed_limit_mps: 5.0,
        };

        network.add_edge(edge);

        // Edge should be added even if nodes don't exist
        assert_eq!(network.edge_count(), 1);
        assert!(network.get_edge(1).is_some());

        // But nodes still don't exist
        assert!(network.get_node(99).is_none());
        assert!(network.get_node(100).is_none());
    }

    #[test]
    fn test_road_type_speeds() {
        assert_eq!(RoadType::Highway.default_speed_mps(), 30.0);
        assert_eq!(RoadType::Arterial.default_speed_mps(), 15.0);
        assert_eq!(RoadType::Residential.default_speed_mps(), 8.0);
        assert_eq!(RoadType::ServiceRoad.default_speed_mps(), 5.0);
    }

    #[test]
    fn test_road_type_display() {
        assert_eq!(format!("{}", RoadType::Highway), "Highway");
        assert_eq!(format!("{}", RoadType::Arterial), "Arterial");
        assert_eq!(format!("{}", RoadType::Residential), "Residential");
        assert_eq!(format!("{}", RoadType::ServiceRoad), "Service Road");
    }

    #[test]
    fn test_serialization() {
        let mut network = RoadNetwork::new();

        network.add_node(RoadNode {
            id: 1,
            position: Point::new(-122.4194, 37.7749),
        });

        network.add_edge(RoadEdge {
            id: 1,
            from_node: 1,
            to_node: 2,
            distance_m: 100.0,
            geometry: LineString::from(vec![(-122.4194, 37.7749), (-122.4184, 37.7759)]),
            road_type: RoadType::Residential,
            speed_limit_mps: 8.0,
        });

        // Test serialization
        let json = serde_json::to_string(&network).unwrap();
        assert!(json.contains("nodes"));
        assert!(json.contains("edges"));
        assert!(json.contains("adjacency"));

        // Test deserialization
        let deserialized: RoadNetwork = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.node_count(), 1);
        assert_eq!(deserialized.edge_count(), 1);
    }

    #[test]
    fn test_empty_neighbors_for_isolated_node() {
        let mut network = RoadNetwork::new();

        // Add a node with no edges
        network.add_node(RoadNode {
            id: 1,
            position: Point::new(0.0, 0.0),
        });

        assert_eq!(network.get_neighbors(1).len(), 0);
        assert_eq!(network.get_neighbors(999).len(), 0); // Non-existent node
    }

    // Benchmark test - measures construction time for 1000-node graph
    #[test]
    fn benchmark_network_construction() {
        use std::time::Instant;

        let start = Instant::now();
        let mut network = RoadNetwork::new();

        // Add 1000 nodes
        for i in 0..1000 {
            network.add_node(RoadNode {
                id: i,
                position: Point::new(i as f64 * 0.001, i as f64 * 0.001),
            });
        }

        // Add ~3000 edges (average of 3 edges per node)
        let mut edge_id = 0;
        for i in 0..1000 {
            // Connect to next 3 nodes (with wraparound)
            for j in 1..=3 {
                let to_node = (i + j) % 1000;
                network.add_edge(RoadEdge {
                    id: edge_id,
                    from_node: i,
                    to_node,
                    distance_m: 100.0 + (i as f64),
                    geometry: LineString::from(vec![
                        (i as f64 * 0.001, i as f64 * 0.001),
                        (to_node as f64 * 0.001, to_node as f64 * 0.001),
                    ]),
                    road_type: match i % 4 {
                        0 => RoadType::Highway,
                        1 => RoadType::Arterial,
                        2 => RoadType::Residential,
                        _ => RoadType::ServiceRoad,
                    },
                    speed_limit_mps: 15.0,
                });
                edge_id += 1;
            }
        }

        let duration = start.elapsed();

        // Verify network was built correctly
        assert_eq!(network.node_count(), 1000);
        assert_eq!(network.edge_count(), 3000);

        // Check that duration is under 10ms
        println!("Network construction time: {:?}", duration);
        assert!(
            duration.as_millis() < 10,
            "Network construction took {:?}, expected < 10ms",
            duration
        );
    }
}
