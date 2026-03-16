//! JSON serialization and deserialization for RoadNetwork

use super::{EdgeId, NodeId, RoadEdge, RoadNetwork, RoadNode, RoadType};
use geo::{LineString, Point};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write as IoWrite};
use std::path::Path;
use thiserror::Error;

/// Errors that can occur during graph I/O operations
#[derive(Debug, Error)]
pub enum GraphError {
    /// I/O error reading or writing file
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON parsing error
    #[error("JSON parse error: {0}")]
    ParseError(#[from] serde_json::Error),

    /// Invalid data in the graph
    #[error("Invalid data: {0}")]
    InvalidData(String),
}

/// Intermediate format for JSON serialization matching test data format
#[derive(Debug, Serialize, Deserialize)]
struct GraphJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    _comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    _schema: Option<serde_json::Value>,
    nodes: Vec<NodeJson>,
    edges: Vec<EdgeJson>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NodeJson {
    id: u32,
    lat: f64,
    lon: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct EdgeJson {
    id: u32,
    from_node: u32,
    to_node: u32,
    distance_m: f64,
    road_type: RoadTypeJson,
    geometry: GeometryJson,
}

#[derive(Debug, Serialize, Deserialize)]
enum RoadTypeJson {
    Highway,
    Arterial,
    Residential,
    ServiceRoad,
}

impl From<RoadTypeJson> for RoadType {
    fn from(rt: RoadTypeJson) -> Self {
        match rt {
            RoadTypeJson::Highway => RoadType::Highway,
            RoadTypeJson::Arterial => RoadType::Arterial,
            RoadTypeJson::Residential => RoadType::Residential,
            RoadTypeJson::ServiceRoad => RoadType::ServiceRoad,
        }
    }
}

impl From<RoadType> for RoadTypeJson {
    fn from(rt: RoadType) -> Self {
        match rt {
            RoadType::Highway => RoadTypeJson::Highway,
            RoadType::Arterial => RoadTypeJson::Arterial,
            RoadType::Residential => RoadTypeJson::Residential,
            RoadType::ServiceRoad => RoadTypeJson::ServiceRoad,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct GeometryJson {
    #[serde(rename = "type")]
    geometry_type: String,
    coordinates: Vec<[f64; 2]>,
}

impl RoadNetwork {
    /// Load a road network from a JSON file
    ///
    /// # Arguments
    /// * `path` - Path to the JSON file
    ///
    /// # Returns
    /// * `Result<Self, GraphError>` - The loaded network or an error
    ///
    /// # Examples
    /// ```no_run
    /// use sim_geo::graph::RoadNetwork;
    ///
    /// let network = RoadNetwork::load_from_file("test_data/small_city_graph.json")?;
    /// # Ok::<(), sim_geo::graph::GraphError>(())
    /// ```
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, GraphError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let graph_json: GraphJson = serde_json::from_reader(reader)?;

        Self::from_graph_json(graph_json)
    }

    /// Save the road network to a JSON file
    ///
    /// # Arguments
    /// * `path` - Path to write the JSON file
    ///
    /// # Returns
    /// * `Result<(), GraphError>` - Success or error
    ///
    /// # Examples
    /// ```no_run
    /// use sim_geo::graph::RoadNetwork;
    ///
    /// # let network = RoadNetwork::new();
    /// network.save_to_file("output.json")?;
    /// # Ok::<(), sim_geo::graph::GraphError>(())
    /// ```
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<(), GraphError> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        let graph_json = self.to_graph_json();
        let json = serde_json::to_string_pretty(&graph_json)?;

        writer.write_all(json.as_bytes())?;
        writer.flush()?;

        Ok(())
    }

    /// Parse a road network from a JSON string
    ///
    /// # Arguments
    /// * `json_str` - JSON string containing the graph data
    ///
    /// # Returns
    /// * `Result<Self, GraphError>` - The parsed network or an error
    ///
    /// # Examples
    /// ```no_run
    /// use sim_geo::graph::RoadNetwork;
    ///
    /// let json = r#"{"nodes": [], "edges": []}"#;
    /// let network = RoadNetwork::from_json(json)?;
    /// # Ok::<(), sim_geo::graph::GraphError>(())
    /// ```
    pub fn from_json(json_str: &str) -> Result<Self, GraphError> {
        let graph_json: GraphJson = serde_json::from_str(json_str)?;
        Self::from_graph_json(graph_json)
    }

    /// Serialize the road network to a JSON string
    ///
    /// # Returns
    /// * `Result<String, GraphError>` - The JSON string or an error
    ///
    /// # Examples
    /// ```no_run
    /// use sim_geo::graph::RoadNetwork;
    ///
    /// # let network = RoadNetwork::new();
    /// let json = network.to_json()?;
    /// # Ok::<(), sim_geo::graph::GraphError>(())
    /// ```
    pub fn to_json(&self) -> Result<String, GraphError> {
        let graph_json = self.to_graph_json();
        Ok(serde_json::to_string_pretty(&graph_json)?)
    }

    /// Convert from intermediate JSON format to RoadNetwork
    fn from_graph_json(graph_json: GraphJson) -> Result<Self, GraphError> {
        // Validate nodes
        let mut node_ids = HashSet::new();
        for node in &graph_json.nodes {
            if !node_ids.insert(node.id) {
                return Err(GraphError::InvalidData(format!(
                    "Duplicate node ID: {}",
                    node.id
                )));
            }
            if !node.lat.is_finite() || !node.lon.is_finite() {
                return Err(GraphError::InvalidData(format!(
                    "Invalid coordinates for node {}: ({}, {})",
                    node.id, node.lat, node.lon
                )));
            }
        }

        // Create network
        let mut network = RoadNetwork::new();

        // Add nodes
        for node_json in graph_json.nodes {
            let node = RoadNode {
                id: node_json.id as NodeId,
                position: Point::new(node_json.lon, node_json.lat),
            };
            network.add_node(node);
        }

        // Validate and add edges
        let mut edge_ids = HashSet::new();

        for edge_json in graph_json.edges {
            // Validate edge ID uniqueness
            if !edge_ids.insert(edge_json.id) {
                return Err(GraphError::InvalidData(format!(
                    "Duplicate edge ID: {}",
                    edge_json.id
                )));
            }

            // Validate distance
            if edge_json.distance_m <= 0.0 || !edge_json.distance_m.is_finite() {
                return Err(GraphError::InvalidData(format!(
                    "Invalid distance for edge {}: {}",
                    edge_json.id, edge_json.distance_m
                )));
            }

            // Validate node references
            if !node_ids.contains(&edge_json.from_node) {
                return Err(GraphError::InvalidData(format!(
                    "Edge {} references non-existent from_node: {}",
                    edge_json.id, edge_json.from_node
                )));
            }
            if !node_ids.contains(&edge_json.to_node) {
                return Err(GraphError::InvalidData(format!(
                    "Edge {} references non-existent to_node: {}",
                    edge_json.id, edge_json.to_node
                )));
            }

            // Validate geometry
            if edge_json.geometry.geometry_type != "LineString" {
                return Err(GraphError::InvalidData(format!(
                    "Edge {} has invalid geometry type: {}",
                    edge_json.id, edge_json.geometry.geometry_type
                )));
            }
            if edge_json.geometry.coordinates.len() < 2 {
                return Err(GraphError::InvalidData(format!(
                    "Edge {} has invalid geometry: need at least 2 coordinates",
                    edge_json.id
                )));
            }

            // Convert geometry - coordinates are [lon, lat] in GeoJSON
            let coords: Vec<(f64, f64)> = edge_json
                .geometry
                .coordinates
                .into_iter()
                .map(|[lon, lat]| (lon, lat))
                .collect();

            let geometry = LineString::from(coords);
            let road_type: RoadType = edge_json.road_type.into();

            let edge = RoadEdge {
                id: edge_json.id as EdgeId,
                from_node: edge_json.from_node as NodeId,
                to_node: edge_json.to_node as NodeId,
                distance_m: edge_json.distance_m,
                geometry,
                road_type: road_type.clone(),
                speed_limit_mps: road_type.default_speed_mps(),
            };

            network.add_edge(edge);
        }

        Ok(network)
    }

    /// Convert RoadNetwork to intermediate JSON format
    fn to_graph_json(&self) -> GraphJson {
        // Collect and sort nodes by ID for consistent output
        let mut nodes: Vec<NodeJson> = self
            .nodes
            .values()
            .map(|n| NodeJson {
                id: n.id as u32,
                lat: n.position.y(),
                lon: n.position.x(),
            })
            .collect();
        nodes.sort_by_key(|n| n.id);

        // Collect and sort edges by ID for consistent output
        let mut edges: Vec<EdgeJson> = self
            .edges
            .values()
            .map(|e| EdgeJson {
                id: e.id as u32,
                from_node: e.from_node as u32,
                to_node: e.to_node as u32,
                distance_m: e.distance_m,
                road_type: e.road_type.clone().into(),
                geometry: GeometryJson {
                    geometry_type: "LineString".to_string(),
                    coordinates: e.geometry.coords().map(|c| [c.x, c.y]).collect(),
                },
            })
            .collect();
        edges.sort_by_key(|e| e.id);

        GraphJson {
            _comment: Some(format!(
                "Generated road network with {} nodes and {} edges",
                nodes.len(),
                edges.len()
            )),
            _schema: None,
            nodes,
            edges,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn create_test_network() -> RoadNetwork {
        let mut network = RoadNetwork::new();

        network.add_node(RoadNode {
            id: 0,
            position: Point::new(-122.4194, 37.7749),
        });
        network.add_node(RoadNode {
            id: 1,
            position: Point::new(-122.4184, 37.7749),
        });
        network.add_node(RoadNode {
            id: 2,
            position: Point::new(-122.4194, 37.7759),
        });

        network.add_edge(RoadEdge {
            id: 0,
            from_node: 0,
            to_node: 1,
            distance_m: 100.0,
            geometry: LineString::from(vec![(-122.4194, 37.7749), (-122.4184, 37.7749)]),
            road_type: RoadType::Residential,
            speed_limit_mps: RoadType::Residential.default_speed_mps(),
        });

        network.add_edge(RoadEdge {
            id: 1,
            from_node: 0,
            to_node: 2,
            distance_m: 110.0,
            geometry: LineString::from(vec![(-122.4194, 37.7749), (-122.4194, 37.7759)]),
            road_type: RoadType::Highway,
            speed_limit_mps: RoadType::Highway.default_speed_mps(),
        });

        network
    }

    #[test]
    fn test_json_roundtrip() {
        let network = create_test_network();

        // Convert to JSON and back
        let json = network.to_json().unwrap();
        let loaded = RoadNetwork::from_json(&json).unwrap();

        // Verify structure
        assert_eq!(loaded.node_count(), network.node_count());
        assert_eq!(loaded.edge_count(), network.edge_count());

        // Verify nodes
        for node_id in 0..3 {
            let orig = network.get_node(node_id).unwrap();
            let loaded_node = loaded.get_node(node_id).unwrap();
            assert_eq!(loaded_node.id, orig.id);
            assert!((loaded_node.position.x() - orig.position.x()).abs() < 1e-10);
            assert!((loaded_node.position.y() - orig.position.y()).abs() < 1e-10);
        }

        // Verify edges
        for edge_id in 0..2 {
            let orig = network.get_edge(edge_id).unwrap();
            let loaded_edge = loaded.get_edge(edge_id).unwrap();
            assert_eq!(loaded_edge.id, orig.id);
            assert_eq!(loaded_edge.from_node, orig.from_node);
            assert_eq!(loaded_edge.to_node, orig.to_node);
            assert!((loaded_edge.distance_m - orig.distance_m).abs() < 1e-10);
        }

        // Verify adjacency index was rebuilt
        assert_eq!(
            loaded.get_neighbors(0).len(),
            network.get_neighbors(0).len()
        );
    }

    #[test]
    fn test_file_roundtrip() {
        let network = create_test_network();
        let temp_path = "/tmp/test_graph_roundtrip.json";

        // Save to file
        network.save_to_file(temp_path).unwrap();

        // Load from file
        let loaded = RoadNetwork::load_from_file(temp_path).unwrap();

        // Verify
        assert_eq!(loaded.node_count(), network.node_count());
        assert_eq!(loaded.edge_count(), network.edge_count());

        // Cleanup
        std::fs::remove_file(temp_path).ok();
    }

    #[test]
    fn test_load_small_city_graph() {
        let start = Instant::now();
        let network = RoadNetwork::load_from_file("test_data/small_city_graph.json").unwrap();
        let duration = start.elapsed();

        println!(
            "Loaded small_city_graph.json in {:?} ({} nodes, {} edges)",
            duration,
            network.node_count(),
            network.edge_count()
        );

        // Verify structure
        assert_eq!(network.node_count(), 100);
        assert_eq!(network.edge_count(), 188);

        // Verify adjacency index was built
        assert!(!network.adjacency.is_empty());

        // Performance check: should load in under 50ms
        assert!(
            duration.as_millis() < 50,
            "Loading took {}ms, expected < 50ms",
            duration.as_millis()
        );

        // Verify a sample node
        let node = network.get_node(0).unwrap();
        assert!((node.position.y() - 37.7749).abs() < 1e-10);
        assert!((node.position.x() - (-122.4194)).abs() < 1e-10);

        // Verify adjacency works
        let outgoing = network.get_neighbors(0);
        assert!(!outgoing.is_empty());
    }

    #[test]
    fn test_load_medium_city_graph() {
        let start = Instant::now();
        let result = RoadNetwork::load_from_file("test_data/medium_city_graph.json");
        let duration = start.elapsed();

        match result {
            Ok(network) => {
                println!(
                    "Loaded medium_city_graph.json in {:?} ({} nodes, {} edges)",
                    duration,
                    network.node_count(),
                    network.edge_count()
                );

                // Verify adjacency index was built
                assert!(!network.adjacency.is_empty());

                // Verify basic structure
                assert!(network.node_count() > 100);
                assert!(network.edge_count() > 100);
            }
            Err(e) => {
                println!("Note: medium_city_graph.json not available: {}", e);
            }
        }
    }

    #[test]
    fn test_error_missing_file() {
        let result = RoadNetwork::load_from_file("nonexistent.json");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GraphError::IoError(_)));
    }

    #[test]
    fn test_error_malformed_json() {
        let result = RoadNetwork::from_json("{invalid json}");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GraphError::ParseError(_)));
    }

    #[test]
    fn test_error_duplicate_node_id() {
        let json = r#"{
            "nodes": [
                {"id": 0, "lat": 37.7749, "lon": -122.4194},
                {"id": 0, "lat": 37.7750, "lon": -122.4195}
            ],
            "edges": []
        }"#;

        let result = RoadNetwork::from_json(json);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GraphError::InvalidData(_)));
    }

    #[test]
    fn test_error_duplicate_edge_id() {
        let json = r#"{
            "nodes": [
                {"id": 0, "lat": 37.7749, "lon": -122.4194},
                {"id": 1, "lat": 37.7750, "lon": -122.4195}
            ],
            "edges": [
                {
                    "id": 0,
                    "from_node": 0,
                    "to_node": 1,
                    "distance_m": 100.0,
                    "road_type": "Residential",
                    "geometry": {"type": "LineString", "coordinates": [[-122.4194, 37.7749], [-122.4195, 37.7750]]}
                },
                {
                    "id": 0,
                    "from_node": 1,
                    "to_node": 0,
                    "distance_m": 100.0,
                    "road_type": "Residential",
                    "geometry": {"type": "LineString", "coordinates": [[-122.4195, 37.7750], [-122.4194, 37.7749]]}
                }
            ]
        }"#;

        let result = RoadNetwork::from_json(json);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GraphError::InvalidData(_)));
    }

    #[test]
    fn test_error_invalid_distance() {
        let json = r#"{
            "nodes": [
                {"id": 0, "lat": 37.7749, "lon": -122.4194},
                {"id": 1, "lat": 37.7750, "lon": -122.4195}
            ],
            "edges": [
                {
                    "id": 0,
                    "from_node": 0,
                    "to_node": 1,
                    "distance_m": -100.0,
                    "road_type": "Residential",
                    "geometry": {"type": "LineString", "coordinates": [[-122.4194, 37.7749], [-122.4195, 37.7750]]}
                }
            ]
        }"#;

        let result = RoadNetwork::from_json(json);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GraphError::InvalidData(_)));
    }

    #[test]
    fn test_error_nonexistent_node_reference() {
        let json = r#"{
            "nodes": [
                {"id": 0, "lat": 37.7749, "lon": -122.4194}
            ],
            "edges": [
                {
                    "id": 0,
                    "from_node": 0,
                    "to_node": 999,
                    "distance_m": 100.0,
                    "road_type": "Residential",
                    "geometry": {"type": "LineString", "coordinates": [[-122.4194, 37.7749], [-122.4195, 37.7750]]}
                }
            ]
        }"#;

        let result = RoadNetwork::from_json(json);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GraphError::InvalidData(_)));
    }

    #[test]
    fn test_error_invalid_geometry() {
        let json = r#"{
            "nodes": [
                {"id": 0, "lat": 37.7749, "lon": -122.4194},
                {"id": 1, "lat": 37.7750, "lon": -122.4195}
            ],
            "edges": [
                {
                    "id": 0,
                    "from_node": 0,
                    "to_node": 1,
                    "distance_m": 100.0,
                    "road_type": "Residential",
                    "geometry": {"type": "LineString", "coordinates": [[-122.4194, 37.7749]]}
                }
            ]
        }"#;

        let result = RoadNetwork::from_json(json);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GraphError::InvalidData(_)));
    }

    #[test]
    fn test_error_invalid_coordinates() {
        let json = r#"{
            "nodes": [
                {"id": 0, "lat": null, "lon": -122.4194}
            ],
            "edges": []
        }"#;

        let result = RoadNetwork::from_json(json);
        assert!(result.is_err());
        // This will be caught by serde_json as a parse error
        assert!(matches!(result.unwrap_err(), GraphError::ParseError(_)));
    }

    #[test]
    fn test_adjacency_index_after_load() {
        let network = RoadNetwork::load_from_file("test_data/small_city_graph.json").unwrap();

        // Verify adjacency index is not empty
        assert!(!network.adjacency.is_empty());

        // Test a node with known outgoing edges
        let outgoing = network.get_neighbors(0);
        assert!(!outgoing.is_empty());

        // Verify all edges in adjacency point to valid edges
        for (node_id, edge_ids) in &network.adjacency {
            for &edge_id in edge_ids {
                let edge = network.get_edge(edge_id).unwrap();
                assert_eq!(edge.from_node, *node_id);
            }
        }
    }

    #[test]
    fn test_road_type_conversion() {
        // Test JSON -> RoadType conversion
        let highway: RoadType = RoadTypeJson::Highway.into();
        assert!(matches!(highway, RoadType::Highway));

        let arterial: RoadType = RoadTypeJson::Arterial.into();
        assert!(matches!(arterial, RoadType::Arterial));

        let residential: RoadType = RoadTypeJson::Residential.into();
        assert!(matches!(residential, RoadType::Residential));

        let service: RoadType = RoadTypeJson::ServiceRoad.into();
        assert!(matches!(service, RoadType::ServiceRoad));

        // Test RoadType -> JSON conversion
        let json_highway: RoadTypeJson = RoadType::Highway.into();
        assert!(matches!(json_highway, RoadTypeJson::Highway));

        let json_service: RoadTypeJson = RoadType::ServiceRoad.into();
        assert!(matches!(json_service, RoadTypeJson::ServiceRoad));
    }
}
