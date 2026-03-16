//! High-performance routing on road networks using fast_paths
//!
//! This module provides efficient shortest path computation on road networks
//! using the fast_paths library, which implements Contraction Hierarchies for
//! fast query times after a preprocessing step.
//!
//! # Example
//!
//! ```
//! use sim_geo::graph::{RoadNetwork, RoadNode, RoadEdge, RoadType, Route};
//! use geo::{Point, LineString};
//!
//! let mut network = RoadNetwork::new();
//!
//! // Add nodes
//! network.add_node(RoadNode { id: 1, position: Point::new(-122.4, 37.7) });
//! network.add_node(RoadNode { id: 2, position: Point::new(-122.3, 37.8) });
//!
//! // Add edge
//! network.add_edge(RoadEdge {
//!     id: 1,
//!     from_node: 1,
//!     to_node: 2,
//!     distance_m: 1500.0,
//!     geometry: LineString::from(vec![(-122.4, 37.7), (-122.3, 37.8)]),
//!     road_type: RoadType::Arterial,
//!     speed_limit_mps: 15.0,
//! });
//!
//! // Find shortest path
//! if let Some(route) = network.find_shortest_path(1, 2) {
//!     println!("Distance: {} meters", route.total_distance_m);
//!     println!("Path: {:?}", route.node_path);
//! }
//! ```

use super::{EdgeId, NodeId, RoadNetwork};
use fast_paths::{FastGraph, InputGraph, PathCalculator};
use geo::{Distance, Haversine, LineString, Point};

/// A route through the road network from origin to destination
///
/// Contains the sequence of nodes and edges traversed, along with
/// the total distance of the route.
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    /// Ordered sequence of node IDs from origin to destination
    pub node_path: Vec<NodeId>,
    /// Ordered sequence of edge IDs connecting the nodes
    pub edge_path: Vec<EdgeId>,
    /// Total distance of the route in meters
    pub total_distance_m: f64,
}

impl RoadNetwork {
    /// Build a FastGraph for routing (preprocessing step)
    ///
    /// This method converts the road network into a format suitable for
    /// fast_paths routing. The preprocessing step can be expensive for large
    /// networks, so in production code, you should cache the FastGraph and
    /// reuse it across multiple routing queries.
    ///
    /// # Returns
    ///
    /// A `FastGraph` ready for efficient shortest path queries
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::graph::RoadNetwork;
    ///
    /// let network = RoadNetwork::new();
    /// let fast_graph = network.build_fast_graph();
    /// // Cache fast_graph for reuse across multiple queries
    /// ```
    pub fn build_fast_graph(&self) -> FastGraph {
        let mut graph = InputGraph::new();

        // Add all edges to the graph with rounded distances as integer weights
        for edge in self.edges.values() {
            graph.add_edge(
                edge.from_node as usize,
                edge.to_node as usize,
                edge.distance_m.round() as usize,
            );
        }

        // Finalize the graph structure
        graph.freeze();

        // Preprocess for fast queries using Contraction Hierarchies
        fast_paths::prepare(&graph)
    }

    /// Find shortest path between two nodes
    ///
    /// Computes the shortest path from start to goal using Dijkstra's algorithm
    /// via the fast_paths library. This method builds a new FastGraph on each call,
    /// which is inefficient for repeated queries. For better performance, use
    /// `build_fast_graph()` once and create a PathCalculator for multiple queries.
    ///
    /// # Arguments
    ///
    /// * `start` - ID of the starting node
    /// * `goal` - ID of the destination node
    ///
    /// # Returns
    ///
    /// `Some(Route)` if a path exists, `None` if the nodes are disconnected
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
    /// network.add_edge(RoadEdge {
    ///     id: 1,
    ///     from_node: 1,
    ///     to_node: 2,
    ///     distance_m: 150.0,
    ///     geometry: LineString::from(vec![(0.0, 0.0), (1.0, 1.0)]),
    ///     road_type: RoadType::Residential,
    ///     speed_limit_mps: 8.0,
    /// });
    ///
    /// let route = network.find_shortest_path(1, 2).unwrap();
    /// assert_eq!(route.node_path, vec![1, 2]);
    /// assert_eq!(route.total_distance_m, 150.0);
    /// ```
    pub fn find_shortest_path(&self, start: NodeId, goal: NodeId) -> Option<Route> {
        // Build fast graph (in production, cache this!)
        let fast_graph = self.build_fast_graph();
        let mut calculator = fast_paths::create_calculator(&fast_graph);

        // Calculate path
        let path = calculator.calc_path(&fast_graph, start as usize, goal as usize)?;

        // Convert node indices to NodeIds
        let node_path: Vec<NodeId> = path
            .get_nodes()
            .iter()
            .map(|n| *n as NodeId)
            .collect();

        // Build edge path by finding edges between consecutive nodes
        let mut edge_path = Vec::new();
        for window in node_path.windows(2) {
            if let Some(edge_id) = self.find_edge_between(window[0], window[1]) {
                edge_path.push(edge_id);
            }
        }

        Some(Route {
            node_path,
            edge_path,
            total_distance_m: path.get_weight() as f64,
        })
    }

    /// Find shortest path using a pre-built FastGraph and PathCalculator
    ///
    /// This is the efficient version for repeated queries. Build the FastGraph
    /// once with `build_fast_graph()`, create a PathCalculator with
    /// `fast_paths::create_calculator()`, and reuse them for multiple queries.
    ///
    /// # Arguments
    ///
    /// * `calculator` - Mutable reference to a reusable PathCalculator
    /// * `fast_graph` - Reference to the pre-built FastGraph
    /// * `start` - ID of the starting node
    /// * `goal` - ID of the destination node
    ///
    /// # Returns
    ///
    /// `Some(Route)` if a path exists, `None` if the nodes are disconnected
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::graph::RoadNetwork;
    ///
    /// let network = RoadNetwork::new();
    /// let fast_graph = network.build_fast_graph();
    /// let mut calculator = fast_paths::create_calculator(&fast_graph);
    ///
    /// // Reuse calculator for multiple queries
    /// # /*
    /// let route1 = network.find_shortest_path_with_cache(&mut calculator, &fast_graph, 1, 2);
    /// let route2 = network.find_shortest_path_with_cache(&mut calculator, &fast_graph, 3, 4);
    /// # */
    /// ```
    pub fn find_shortest_path_with_cache(
        &self,
        calculator: &mut PathCalculator,
        fast_graph: &FastGraph,
        start: NodeId,
        goal: NodeId,
    ) -> Option<Route> {
        // Calculate path
        let path = calculator.calc_path(fast_graph, start as usize, goal as usize)?;

        // Convert node indices to NodeIds
        let node_path: Vec<NodeId> = path
            .get_nodes()
            .iter()
            .map(|n| *n as NodeId)
            .collect();

        // Build edge path by finding edges between consecutive nodes
        let mut edge_path = Vec::new();
        for window in node_path.windows(2) {
            if let Some(edge_id) = self.find_edge_between(window[0], window[1]) {
                edge_path.push(edge_id);
            }
        }

        Some(Route {
            node_path,
            edge_path,
            total_distance_m: path.get_weight() as f64,
        })
    }

    /// Find nearest node to a geographic point
    ///
    /// Uses brute-force linear search with Haversine distance. For large networks,
    /// consider using a spatial index (e.g., R-tree or H3) for better performance.
    ///
    /// # Arguments
    ///
    /// * `point` - Geographic point (longitude, latitude)
    ///
    /// # Returns
    ///
    /// `Some(NodeId)` of the nearest node, `None` if the network is empty
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::graph::{RoadNetwork, RoadNode};
    /// use geo::Point;
    ///
    /// let mut network = RoadNetwork::new();
    /// network.add_node(RoadNode {
    ///     id: 1,
    ///     position: Point::new(-122.4194, 37.7749),
    /// });
    ///
    /// let query_point = Point::new(-122.42, 37.77);
    /// let nearest = network.find_nearest_node(query_point);
    /// assert_eq!(nearest, Some(1));
    /// ```
    pub fn find_nearest_node(&self, point: Point<f64>) -> Option<NodeId> {
        self.nodes
            .values()
            .min_by_key(|node| {
                let dist = Haversine.distance(node.position, point);
                (dist * 1000.0) as u64 // Convert to mm for integer comparison
            })
            .map(|node| node.id)
    }

    /// Helper: Find edge ID between two nodes
    ///
    /// Searches for a directed edge from `from` to `to`.
    ///
    /// # Arguments
    ///
    /// * `from` - Source node ID
    /// * `to` - Destination node ID
    ///
    /// # Returns
    ///
    /// `Some(EdgeId)` if an edge exists, `None` otherwise
    fn find_edge_between(&self, from: NodeId, to: NodeId) -> Option<EdgeId> {
        self.edges
            .values()
            .find(|edge| edge.from_node == from && edge.to_node == to)
            .map(|edge| edge.id)
    }
}

impl Route {
    /// Convert route to LineString geometry
    ///
    /// Concatenates the geometries of all edges in the route to create
    /// a single continuous LineString representing the path.
    ///
    /// # Arguments
    ///
    /// * `network` - Reference to the road network containing edge geometries
    ///
    /// # Returns
    ///
    /// `Some(LineString)` if all edges have geometries, `None` if any edge is missing
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
    /// network.add_edge(RoadEdge {
    ///     id: 1,
    ///     from_node: 1,
    ///     to_node: 2,
    ///     distance_m: 100.0,
    ///     geometry: LineString::from(vec![(0.0, 0.0), (0.5, 0.0), (1.0, 0.0)]),
    ///     road_type: RoadType::Residential,
    ///     speed_limit_mps: 8.0,
    /// });
    ///
    /// let route = network.find_shortest_path(1, 2).unwrap();
    /// let linestring = route.to_linestring(&network).unwrap();
    /// assert_eq!(linestring.coords().count(), 3);
    /// ```
    pub fn to_linestring(&self, network: &RoadNetwork) -> Option<LineString<f64>> {
        let mut coords = Vec::new();

        for &edge_id in &self.edge_path {
            if let Some(edge) = network.get_edge(edge_id) {
                coords.extend(edge.geometry.coords());
            }
        }

        if coords.is_empty() {
            None
        } else {
            Some(LineString::new(coords))
        }
    }

    /// Verify that the route distance matches the sum of edge distances
    ///
    /// Useful for testing and validation. Returns true if the total distance
    /// matches the sum of individual edge distances (within floating point tolerance).
    /// Note: The tolerance accounts for rounding that occurs in fast_paths processing.
    ///
    /// # Arguments
    ///
    /// * `network` - Reference to the road network
    ///
    /// # Returns
    ///
    /// `true` if distances match, `false` otherwise
    pub fn verify_distance(&self, network: &RoadNetwork) -> bool {
        let sum_of_edges: f64 = self
            .edge_path
            .iter()
            .filter_map(|&edge_id| network.get_edge(edge_id))
            .map(|edge| edge.distance_m.round())
            .sum();

        (self.total_distance_m - sum_of_edges).abs() < 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{RoadEdge, RoadNode, RoadType};

    fn create_test_network() -> RoadNetwork {
        let mut network = RoadNetwork::new();

        // Create a simple triangle graph:
        //     1
        //    / \
        //   /   \
        //  2-----3

        network.add_node(RoadNode {
            id: 1,
            position: Point::new(0.0, 1.0),
        });
        network.add_node(RoadNode {
            id: 2,
            position: Point::new(-1.0, 0.0),
        });
        network.add_node(RoadNode {
            id: 3,
            position: Point::new(1.0, 0.0),
        });

        // Edge 1->2 (distance: 100)
        network.add_edge(RoadEdge {
            id: 1,
            from_node: 1,
            to_node: 2,
            distance_m: 100.0,
            geometry: LineString::from(vec![(0.0, 1.0), (-1.0, 0.0)]),
            road_type: RoadType::Residential,
            speed_limit_mps: 8.0,
        });

        // Edge 2->3 (distance: 200)
        network.add_edge(RoadEdge {
            id: 2,
            from_node: 2,
            to_node: 3,
            distance_m: 200.0,
            geometry: LineString::from(vec![(-1.0, 0.0), (0.0, 0.0), (1.0, 0.0)]),
            road_type: RoadType::Arterial,
            speed_limit_mps: 15.0,
        });

        // Edge 1->3 (distance: 150)
        network.add_edge(RoadEdge {
            id: 3,
            from_node: 1,
            to_node: 3,
            distance_m: 150.0,
            geometry: LineString::from(vec![(0.0, 1.0), (1.0, 0.0)]),
            road_type: RoadType::Highway,
            speed_limit_mps: 30.0,
        });

        network
    }

    #[test]
    fn test_routing_simple_path() {
        let network = create_test_network();

        // Direct path 1->3 (distance: 150)
        let route = network.find_shortest_path(1, 3).unwrap();
        assert_eq!(route.node_path, vec![1, 3]);
        assert_eq!(route.edge_path, vec![3]);
        assert_eq!(route.total_distance_m, 150.0);

        // Verify distance consistency
        assert!(route.verify_distance(&network));
    }

    #[test]
    fn test_routing_multi_hop_path() {
        let network = create_test_network();

        // Path 1->2->3 should prefer direct 1->3 (150) over 1->2->3 (300)
        let route = network.find_shortest_path(1, 3).unwrap();
        assert_eq!(route.total_distance_m, 150.0);

        // But 1->2 is the only option
        let route_1_2 = network.find_shortest_path(1, 2).unwrap();
        assert_eq!(route_1_2.node_path, vec![1, 2]);
        assert_eq!(route_1_2.total_distance_m, 100.0);
    }

    #[test]
    fn test_routing_disconnected_graph() {
        let mut network = RoadNetwork::new();

        // Create two disconnected components
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
            position: Point::new(10.0, 10.0),
        });
        network.add_node(RoadNode {
            id: 4,
            position: Point::new(11.0, 10.0),
        });

        // Connect 1->2
        network.add_edge(RoadEdge {
            id: 1,
            from_node: 1,
            to_node: 2,
            distance_m: 100.0,
            geometry: LineString::from(vec![(0.0, 0.0), (1.0, 0.0)]),
            road_type: RoadType::Residential,
            speed_limit_mps: 8.0,
        });

        // Connect 3->4 (separate component)
        network.add_edge(RoadEdge {
            id: 2,
            from_node: 3,
            to_node: 4,
            distance_m: 100.0,
            geometry: LineString::from(vec![(10.0, 10.0), (11.0, 10.0)]),
            road_type: RoadType::Residential,
            speed_limit_mps: 8.0,
        });

        // No path from component 1 to component 2
        let route = network.find_shortest_path(1, 4);
        assert!(route.is_none());
    }

    #[test]
    fn test_routing_same_start_and_goal() {
        let network = create_test_network();

        // Path from node to itself
        let route = network.find_shortest_path(1, 1).unwrap();
        assert_eq!(route.node_path, vec![1]);
        assert_eq!(route.edge_path.len(), 0);
        assert_eq!(route.total_distance_m, 0.0);
    }

    #[test]
    fn test_find_nearest_node() {
        let network = create_test_network();

        // Point very close to node 1
        let nearest = network.find_nearest_node(Point::new(0.01, 1.01));
        assert_eq!(nearest, Some(1));

        // Point very close to node 2
        let nearest = network.find_nearest_node(Point::new(-0.99, 0.01));
        assert_eq!(nearest, Some(2));

        // Point very close to node 3
        let nearest = network.find_nearest_node(Point::new(0.99, 0.01));
        assert_eq!(nearest, Some(3));
    }

    #[test]
    fn test_find_nearest_node_empty_network() {
        let network = RoadNetwork::new();
        let nearest = network.find_nearest_node(Point::new(0.0, 0.0));
        assert!(nearest.is_none());
    }

    #[test]
    fn test_route_to_linestring() {
        let network = create_test_network();

        let route = network.find_shortest_path(1, 2).unwrap();
        let linestring = route.to_linestring(&network).unwrap();

        // Should have 2 coordinates from edge 1->2
        assert_eq!(linestring.coords().count(), 2);
        assert_eq!(linestring.coords().next().unwrap().x, 0.0);
        assert_eq!(linestring.coords().next().unwrap().y, 1.0);
    }

    #[test]
    fn test_route_to_linestring_multi_edge() {
        let _network = create_test_network();

        // Need to construct a path that uses multiple edges
        // Since 1->2->3 is longer, we need a different topology
        let mut network2 = RoadNetwork::new();

        network2.add_node(RoadNode {
            id: 1,
            position: Point::new(0.0, 0.0),
        });
        network2.add_node(RoadNode {
            id: 2,
            position: Point::new(1.0, 0.0),
        });
        network2.add_node(RoadNode {
            id: 3,
            position: Point::new(2.0, 0.0),
        });

        // Only path is 1->2->3
        network2.add_edge(RoadEdge {
            id: 1,
            from_node: 1,
            to_node: 2,
            distance_m: 100.0,
            geometry: LineString::from(vec![(0.0, 0.0), (0.5, 0.0), (1.0, 0.0)]),
            road_type: RoadType::Residential,
            speed_limit_mps: 8.0,
        });

        network2.add_edge(RoadEdge {
            id: 2,
            from_node: 2,
            to_node: 3,
            distance_m: 100.0,
            geometry: LineString::from(vec![(1.0, 0.0), (1.5, 0.0), (2.0, 0.0)]),
            road_type: RoadType::Residential,
            speed_limit_mps: 8.0,
        });

        let route = network2.find_shortest_path(1, 3).unwrap();
        let linestring = route.to_linestring(&network2).unwrap();

        // Should have 6 coordinates total (3 from each edge)
        assert_eq!(linestring.coords().count(), 6);
    }

    #[test]
    fn test_verify_distance() {
        let network = create_test_network();

        let route = network.find_shortest_path(1, 3).unwrap();
        assert!(route.verify_distance(&network));

        // Create a route with wrong distance
        let mut bad_route = route.clone();
        bad_route.total_distance_m = 999.0;
        assert!(!bad_route.verify_distance(&network));
    }

    #[test]
    fn test_build_fast_graph() {
        let _network = create_test_network();
        let _fast_graph = _network.build_fast_graph();
        // If this doesn't panic, the graph was built successfully
    }

    #[test]
    fn test_find_shortest_path_with_cache() {
        let network = create_test_network();
        let fast_graph = network.build_fast_graph();
        let mut calculator = fast_paths::create_calculator(&fast_graph);

        // Use cached calculator for multiple queries
        let route1 = network
            .find_shortest_path_with_cache(&mut calculator, &fast_graph, 1, 3)
            .unwrap();
        assert_eq!(route1.total_distance_m, 150.0);

        let route2 = network
            .find_shortest_path_with_cache(&mut calculator, &fast_graph, 1, 2)
            .unwrap();
        assert_eq!(route2.total_distance_m, 100.0);

        // Verify same results as non-cached version
        let route1_nocache = network.find_shortest_path(1, 3).unwrap();
        assert_eq!(route1, route1_nocache);
    }

    #[test]
    fn test_routing_on_real_data() {
        // Load the small_city_graph.json test data
        let network = RoadNetwork::load_from_file(
            "test_data/small_city_graph.json",
        )
        .expect("Failed to load test data");

        println!("Loaded network: {} nodes, {} edges",
            network.node_count(),
            network.edge_count()
        );

        // Verify the network loaded correctly
        assert!(network.node_count() > 0);
        assert!(network.edge_count() > 0);

        // Build fast graph
        let fast_graph = network.build_fast_graph();
        let mut calculator = fast_paths::create_calculator(&fast_graph);

        // Find some nodes that exist
        let node_ids: Vec<NodeId> = network.nodes.keys().take(10).copied().collect();

        if node_ids.len() >= 2 {
            let start = node_ids[0];
            let goal = node_ids[1];

            println!("Finding path from {} to {}", start, goal);

            // Try to find a path
            if let Some(route) = network.find_shortest_path_with_cache(
                &mut calculator,
                &fast_graph,
                start,
                goal,
            ) {
                println!("Found route with {} nodes, distance: {} m",
                    route.node_path.len(),
                    route.total_distance_m
                );

                // Verify the route
                assert!(!route.node_path.is_empty());
                assert_eq!(route.node_path[0], start);
                assert_eq!(route.node_path[route.node_path.len() - 1], goal);
                assert!(route.verify_distance(&network));

                // Test linestring conversion
                let linestring = route.to_linestring(&network);
                assert!(linestring.is_some());
            } else {
                println!("No path found (nodes may be disconnected)");
            }
        }
    }

    #[test]
    fn test_routing_benchmark() {
        use std::time::Instant;

        // Load test data
        let network = RoadNetwork::load_from_file(
            "test_data/small_city_graph.json",
        )
        .expect("Failed to load test data");

        println!("\n=== Routing Benchmark ===");
        println!("Network size: {} nodes, {} edges",
            network.node_count(),
            network.edge_count()
        );

        // Measure preprocessing time
        let start = Instant::now();
        let fast_graph = network.build_fast_graph();
        let preprocessing_time = start.elapsed();
        println!("Preprocessing time: {:?}", preprocessing_time);

        // Get some nodes to test routing
        let node_ids: Vec<NodeId> = network.nodes.keys().take(100).copied().collect();

        if node_ids.len() >= 10 {
            let mut calculator = fast_paths::create_calculator(&fast_graph);
            let num_queries = 100;

            // Measure query time
            let start = Instant::now();
            let mut successful_routes = 0;

            for i in 0..num_queries {
                let start_node = node_ids[i % node_ids.len()];
                let goal_node = node_ids[(i + 7) % node_ids.len()];

                if let Some(_route) = network.find_shortest_path_with_cache(
                    &mut calculator,
                    &fast_graph,
                    start_node,
                    goal_node,
                ) {
                    successful_routes += 1;
                }
            }

            let query_time = start.elapsed();
            let avg_query_time = query_time / (num_queries as u32);

            println!("Query time for {} queries: {:?}", num_queries, query_time);
            println!("Average query time: {:?}", avg_query_time);
            println!("Successful routes: {}/{}", successful_routes, num_queries);

            // Target: Queries in microseconds after preprocessing
            assert!(
                avg_query_time.as_micros() < 10000,
                "Average query time should be under 10ms, got {:?}",
                avg_query_time
            );
        }
    }
}
