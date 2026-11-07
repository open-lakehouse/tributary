//! Journey planning and tracking
//!
//! This module provides journey planning with optional road network routing.
//! When a network is available, routes follow actual roads; otherwise, straight-line
//! paths are used.

pub mod velocity;

use crate::graph::RoadNetwork;
use chrono::{DateTime, Duration, Utc};
use geo::{Haversine, Length};
use geo_types::{LineString, Point};
use serde::{Deserialize, Serialize};
use sim_core::utils::idents::{AgentId, JourneyId};
use std::collections::HashMap;

/// State of a journey
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyState {
    /// Unique identifier
    pub id: JourneyId,
    /// Agent making the journey
    pub agent_id: AgentId,
    /// Route geometry
    pub route: LineString<f64>,
    /// Total distance in meters
    pub distance_m: f64,
    /// Current progress (0.0 to 1.0)
    pub progress: f64,
    /// Start time
    pub start_time: DateTime<Utc>,
    /// Optional metadata
    pub metadata: HashMap<String, String>,
}

/// A journey (alias for compatibility)
pub type Journey = JourneyState;

/// Model for journey velocity
pub trait VelocityModel: Send + Sync {
    /// Get velocity in meters per second
    ///
    /// # Arguments
    ///
    /// * `journey` - The journey being traveled
    /// * `progress` - Current progress (0.0 to 1.0)
    ///
    /// # Returns
    ///
    /// Velocity in m/s
    fn velocity_m_s(&self, journey: &JourneyState, progress: f64) -> f64;

    /// Get velocity in meters per second with explicit current time
    ///
    /// This method allows velocity models to adjust speeds based on the current
    /// simulation time (e.g., for rush hour modeling). The default implementation
    /// delegates to `velocity_m_s` for backward compatibility.
    ///
    /// # Arguments
    ///
    /// * `journey` - The journey being traveled
    /// * `progress` - Current progress (0.0 to 1.0)
    /// * `current_time` - Current simulation time
    ///
    /// # Returns
    ///
    /// Velocity in m/s
    fn velocity_m_s_at_time(
        &self,
        journey: &JourneyState,
        progress: f64,
        _current_time: DateTime<Utc>,
    ) -> f64 {
        // Default implementation for backward compatibility
        self.velocity_m_s(journey, progress)
    }
}

/// Constant velocity model
pub struct ConstantVelocity {
    pub velocity_m_s: f64,
}

impl ConstantVelocity {
    pub fn new(velocity_m_s: f64) -> Self {
        Self { velocity_m_s }
    }
}

impl Default for ConstantVelocity {
    fn default() -> Self {
        Self::new(5.0) // 5 m/s (~18 km/h, walking/cycling speed)
    }
}

impl VelocityModel for ConstantVelocity {
    fn velocity_m_s(&self, _journey: &JourneyState, _progress: f64) -> f64 {
        self.velocity_m_s
    }
}

/// Movement handler with optional road network routing
///
/// Manages active journeys and provides journey planning. When a road network
/// is loaded, routes follow actual road segments. Without a network, straight-line
/// paths are used.
pub struct MovementHandler {
    active_journeys: HashMap<JourneyId, JourneyState>,
    velocity_model: Box<dyn VelocityModel>,
    network: Option<RoadNetwork>,
}

impl MovementHandler {
    /// Create a new movement handler
    pub fn new() -> Self {
        Self {
            active_journeys: HashMap::new(),
            velocity_model: Box::new(ConstantVelocity::default()),
            network: None,
        }
    }

    /// Set the velocity model
    pub fn with_velocity_model(mut self, model: Box<dyn VelocityModel>) -> Self {
        self.velocity_model = model;
        self
    }

    /// Set the road network for routing (builder pattern)
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::{MovementHandler, RoadNetwork};
    ///
    /// let network = RoadNetwork::new();
    /// let handler = MovementHandler::new().with_network(network);
    /// ```
    pub fn with_network(mut self, network: RoadNetwork) -> Self {
        self.network = Some(network);
        self
    }

    /// Load a road network for routing
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::{MovementHandler, RoadNetwork};
    ///
    /// let mut handler = MovementHandler::new();
    /// let network = RoadNetwork::new();
    /// handler.load_network(network);
    /// ```
    pub fn load_network(&mut self, network: RoadNetwork) {
        self.network = Some(network);
    }

    /// Check if a network is loaded
    pub fn has_network(&self) -> bool {
        self.network.is_some()
    }

    /// Plan a journey from origin to destination
    ///
    /// When a road network is loaded, routes follow actual roads. Otherwise,
    /// a straight-line path is used. Returns a journey ready to be added to
    /// the handler.
    ///
    /// # Arguments
    ///
    /// * `agent_id` - The agent making the journey
    /// * `origin` - Starting point (longitude, latitude)
    /// * `destination` - End point (longitude, latitude)
    ///
    /// # Returns
    ///
    /// A `JourneyState` with the planned route, or an error if routing fails
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::MovementHandler;
    /// use sim_core::utils::idents::AgentId;
    /// use geo_types::Point;
    /// use chrono::Utc;
    ///
    /// let mut handler = MovementHandler::new();
    /// let agent_id = AgentId::new();
    /// let origin = Point::new(-122.4194, 37.7749);
    /// let destination = Point::new(-122.4084, 37.7849);
    ///
    /// let journey = handler.plan_journey(agent_id, origin, destination, Utc::now()).unwrap();
    /// handler.add_journey(journey);
    /// ```
    pub fn plan_journey(
        &self,
        agent_id: AgentId,
        origin: Point<f64>,
        destination: Point<f64>,
        start_time: DateTime<Utc>,
    ) -> crate::Result<JourneyState> {
        let route_geometry = if let Some(network) = &self.network {
            // Network-based routing
            let origin_node = network
                .find_nearest_node(origin)
                .ok_or_else(|| crate::Error::Routing("No nearby node found for origin".into()))?;
            let dest_node = network.find_nearest_node(destination).ok_or_else(|| {
                crate::Error::Routing("No nearby node found for destination".into())
            })?;

            if let Some(route) = network.find_shortest_path(origin_node, dest_node) {
                route
                    .to_linestring(network)
                    .unwrap_or_else(|| LineString::from(vec![origin, destination]))
            } else {
                // No path found, fall back to straight line
                LineString::from(vec![origin, destination])
            }
        } else {
            // No network, use straight line
            LineString::from(vec![origin, destination])
        };

        // Calculate distance
        let distance_m = Haversine.length(&route_geometry);

        // Validate distance
        if distance_m <= 0.0 {
            return Err(crate::Error::Journey(format!(
                "Journey distance is invalid: {}. Origin and destination may be identical.",
                distance_m
            )));
        }

        Ok(JourneyState {
            id: JourneyId::new(),
            agent_id,
            route: route_geometry,
            distance_m,
            progress: 0.0,
            start_time,
            metadata: HashMap::new(),
        })
    }

    /// Track an already-planned journey
    pub fn add_journey(&mut self, journey: JourneyState) {
        let id = journey.id;
        self.active_journeys.insert(id, journey);
    }

    /// Advance all journeys by one time step starting at `now`
    ///
    /// Each journey moves at the velocity model's speed at `now` (so
    /// time-of-day models see the real simulation time) for `time_step`.
    ///
    /// # Errors
    ///
    /// Returns an error if a journey has a non-positive distance.
    pub fn advance_journeys(
        &mut self,
        now: DateTime<Utc>,
        time_step: Duration,
    ) -> crate::Result<()> {
        let time_step_secs = time_step.num_milliseconds() as f64 / 1000.0;

        for journey in self.active_journeys.values_mut() {
            if journey.distance_m <= 0.0 {
                return Err(crate::Error::Journey(format!(
                    "Journey {} has invalid distance: {}. Distance must be positive.",
                    journey.id, journey.distance_m
                )));
            }

            let velocity = self
                .velocity_model
                .velocity_m_s_at_time(journey, journey.progress, now);
            let progress_delta = velocity * time_step_secs / journey.distance_m;
            journey.progress = (journey.progress + progress_delta).min(1.0);
        }

        Ok(())
    }

    /// Get completed journeys (progress >= 1.0)
    pub fn get_completed_journeys(&self) -> Vec<JourneyState> {
        self.active_journeys
            .values()
            .filter(|j| j.progress >= 1.0)
            .cloned()
            .collect()
    }

    /// Remove completed journeys
    pub fn remove_completed(&mut self) -> Vec<JourneyState> {
        let completed_ids: Vec<_> = self
            .active_journeys
            .iter()
            .filter(|(_, j)| j.progress >= 1.0)
            .map(|(id, _)| *id)
            .collect();

        completed_ids
            .into_iter()
            .filter_map(|id| self.active_journeys.remove(&id))
            .collect()
    }

    /// Get active journey count
    pub fn active_count(&self) -> usize {
        self.active_journeys.len()
    }
}

impl Default for MovementHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo_types::Point;

    #[test]
    fn test_journey_progression() {
        let mut handler = MovementHandler::new();

        let journey = JourneyState {
            id: JourneyId::new(),
            agent_id: AgentId::new(),
            route: LineString::new(vec![
                Point::new(0.0, 0.0).into(),
                Point::new(1.0, 1.0).into(),
            ]),
            distance_m: 1000.0,
            progress: 0.0,
            start_time: Utc::now(),
            metadata: HashMap::new(),
        };

        handler.add_journey(journey);

        // Advance by 100 seconds at 5 m/s = 500m
        handler
            .advance_journeys(Utc::now(), Duration::seconds(100))
            .unwrap();

        let journeys: Vec<_> = handler.active_journeys.values().collect();
        assert_eq!(journeys.len(), 1);
        assert!(journeys[0].progress > 0.4 && journeys[0].progress < 0.6);
    }

    #[test]
    fn test_journey_completion() {
        let mut handler = MovementHandler::new();

        let journey = JourneyState {
            id: JourneyId::new(),
            agent_id: AgentId::new(),
            route: LineString::new(vec![
                Point::new(0.0, 0.0).into(),
                Point::new(1.0, 1.0).into(),
            ]),
            distance_m: 500.0,
            progress: 0.0,
            start_time: Utc::now(),
            metadata: HashMap::new(),
        };

        handler.add_journey(journey);

        // Advance by 200 seconds at 5 m/s = 1000m (more than distance)
        handler
            .advance_journeys(Utc::now(), Duration::seconds(200))
            .unwrap();

        let completed = handler.get_completed_journeys();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].progress, 1.0);
    }

    #[test]
    fn test_zero_distance_journey() {
        let mut handler = MovementHandler::new();

        let journey = JourneyState {
            id: JourneyId::new(),
            agent_id: AgentId::new(),
            route: LineString::new(vec![
                Point::new(0.0, 0.0).into(),
                Point::new(0.0, 0.0).into(),
            ]),
            distance_m: 0.0, // Zero distance - should cause error
            progress: 0.0,
            start_time: Utc::now(),
            metadata: HashMap::new(),
        };

        handler.add_journey(journey);

        // This should fail with Journey error
        let result = handler.advance_journeys(Utc::now(), Duration::seconds(10));
        assert!(result.is_err());
        if let Err(crate::Error::Journey(msg)) = result {
            assert!(msg.contains("invalid distance"));
        } else {
            panic!("Expected Journey error for zero distance");
        }
    }

    #[test]
    fn test_negative_distance_journey() {
        let mut handler = MovementHandler::new();

        let journey = JourneyState {
            id: JourneyId::new(),
            agent_id: AgentId::new(),
            route: LineString::new(vec![
                Point::new(0.0, 0.0).into(),
                Point::new(1.0, 1.0).into(),
            ]),
            distance_m: -100.0, // Negative distance - should cause error
            progress: 0.0,
            start_time: Utc::now(),
            metadata: HashMap::new(),
        };

        handler.add_journey(journey);

        // This should fail with Journey error
        let result = handler.advance_journeys(Utc::now(), Duration::seconds(10));
        assert!(result.is_err());
        if let Err(crate::Error::Journey(msg)) = result {
            assert!(msg.contains("invalid distance"));
        } else {
            panic!("Expected Journey error for negative distance");
        }
    }

    // ===== NETWORK INTEGRATION TESTS =====

    #[test]
    fn test_plan_journey_without_network() {
        // Test backward compatibility - no network should use straight line
        let handler = MovementHandler::new();
        let agent_id = AgentId::new();

        let origin = Point::new(-122.4194, 37.7749);
        let destination = Point::new(-122.4084, 37.7849);

        let journey = handler
            .plan_journey(agent_id, origin, destination, Utc::now())
            .unwrap();

        // Should have straight line route
        assert_eq!(journey.route.coords().count(), 2);
        assert_eq!(journey.agent_id, agent_id);
        assert!(journey.distance_m > 0.0);
        assert_eq!(journey.progress, 0.0);

        // Verify it's actually a straight line
        let coords: Vec<_> = journey.route.coords().collect();
        assert_eq!(coords[0].x, origin.x());
        assert_eq!(coords[0].y, origin.y());
        assert_eq!(coords[1].x, destination.x());
        assert_eq!(coords[1].y, destination.y());
    }

    #[test]
    fn test_plan_journey_with_network() {
        use crate::graph::{RoadEdge, RoadNode, RoadType};

        // Create a simple network
        let mut network = RoadNetwork::new();

        // Add nodes in a triangle
        network.add_node(RoadNode {
            id: 1,
            position: Point::new(0.0, 0.0),
        });
        network.add_node(RoadNode {
            id: 2,
            position: Point::new(0.01, 0.0),
        });
        network.add_node(RoadNode {
            id: 3,
            position: Point::new(0.0, 0.01),
        });

        // Add edges
        network.add_edge(RoadEdge {
            id: 1,
            from_node: 1,
            to_node: 2,
            distance_m: 1000.0,
            geometry: LineString::from(vec![(0.0, 0.0), (0.005, 0.0), (0.01, 0.0)]),
            road_type: RoadType::Residential,
            speed_limit_mps: 8.0,
        });

        network.add_edge(RoadEdge {
            id: 2,
            from_node: 1,
            to_node: 3,
            distance_m: 1100.0,
            geometry: LineString::from(vec![(0.0, 0.0), (0.0, 0.005), (0.0, 0.01)]),
            road_type: RoadType::Residential,
            speed_limit_mps: 8.0,
        });

        let handler = MovementHandler::new().with_network(network);

        // Plan journey that should use network routing
        let origin = Point::new(0.0001, 0.0001); // Near node 1
        let destination = Point::new(0.0099, 0.0001); // Near node 2

        let journey = handler
            .plan_journey(AgentId::new(), origin, destination, Utc::now())
            .unwrap();

        // Route should follow network (3 points from edge geometry)
        assert_eq!(journey.route.coords().count(), 3);
        assert!(journey.distance_m > 0.0);

        // Distance should be close to the edge distance (1000m)
        // Allow some tolerance for coordinate snapping
        assert!(
            (journey.distance_m - 1000.0).abs() < 200.0,
            "Expected distance ~1000m, got {}m",
            journey.distance_m
        );
    }

    #[test]
    fn test_network_routes_follow_roads() {
        use crate::graph::{RoadEdge, RoadNode, RoadType};

        let mut network = RoadNetwork::new();

        // Create a path with intermediate waypoints
        network.add_node(RoadNode {
            id: 1,
            position: Point::new(0.0, 0.0),
        });
        network.add_node(RoadNode {
            id: 2,
            position: Point::new(0.01, 0.01),
        });

        // Edge with curved geometry (not straight line)
        network.add_edge(RoadEdge {
            id: 1,
            from_node: 1,
            to_node: 2,
            distance_m: 1500.0,
            geometry: LineString::from(vec![
                (0.0, 0.0),
                (0.005, 0.003), // Curve point
                (0.008, 0.007), // Another curve point
                (0.01, 0.01),
            ]),
            road_type: RoadType::Arterial,
            speed_limit_mps: 15.0,
        });

        let handler = MovementHandler::new().with_network(network);

        let origin = Point::new(0.0001, 0.0001);
        let destination = Point::new(0.0099, 0.0099);

        let journey = handler
            .plan_journey(AgentId::new(), origin, destination, Utc::now())
            .unwrap();

        // Route should have multiple points (following the curved road)
        assert_eq!(
            journey.route.coords().count(),
            4,
            "Route should follow road geometry with 4 points"
        );

        // Verify it's not a straight line by checking middle points
        let coords: Vec<_> = journey.route.coords().collect();
        assert_eq!(coords[0].x, 0.0);
        assert_eq!(coords[1].x, 0.005); // First curve point
        assert_eq!(coords[2].x, 0.008); // Second curve point
        assert_eq!(coords[3].x, 0.01);
    }

    #[test]
    fn test_fallback_to_straight_line_when_disconnected() {
        use crate::graph::{RoadEdge, RoadNode, RoadType};

        let mut network = RoadNetwork::new();

        // Create two disconnected components
        network.add_node(RoadNode {
            id: 1,
            position: Point::new(0.0, 0.0),
        });
        network.add_node(RoadNode {
            id: 2,
            position: Point::new(0.01, 0.0),
        });
        network.add_node(RoadNode {
            id: 3,
            position: Point::new(0.1, 0.1),
        });
        network.add_node(RoadNode {
            id: 4,
            position: Point::new(0.11, 0.1),
        });

        // Connect 1-2 only
        network.add_edge(RoadEdge {
            id: 1,
            from_node: 1,
            to_node: 2,
            distance_m: 1000.0,
            geometry: LineString::from(vec![(0.0, 0.0), (0.01, 0.0)]),
            road_type: RoadType::Residential,
            speed_limit_mps: 8.0,
        });

        // Connect 3-4 only (separate component)
        network.add_edge(RoadEdge {
            id: 2,
            from_node: 3,
            to_node: 4,
            distance_m: 1000.0,
            geometry: LineString::from(vec![(0.1, 0.1), (0.11, 0.1)]),
            road_type: RoadType::Residential,
            speed_limit_mps: 8.0,
        });

        let handler = MovementHandler::new().with_network(network);

        // Try to route between disconnected components
        let origin = Point::new(0.0001, 0.0001); // Near node 1
        let destination = Point::new(0.1001, 0.1001); // Near node 3

        let journey = handler
            .plan_journey(AgentId::new(), origin, destination, Utc::now())
            .unwrap();

        // Should fall back to straight line (2 points)
        assert_eq!(
            journey.route.coords().count(),
            2,
            "Should fallback to straight line when nodes are disconnected"
        );
    }

    #[test]
    fn test_network_loading_methods() {
        use crate::graph::RoadNode;

        let mut network = RoadNetwork::new();
        network.add_node(RoadNode {
            id: 1,
            position: Point::new(0.0, 0.0),
        });

        // Test builder pattern
        let handler1 = MovementHandler::new().with_network(network.clone());
        assert!(handler1.has_network());

        // Test load_network method
        let mut handler2 = MovementHandler::new();
        assert!(!handler2.has_network());
        handler2.load_network(network);
        assert!(handler2.has_network());
    }

    #[test]
    fn test_identical_origin_destination_error() {
        let handler = MovementHandler::new();

        let point = Point::new(-122.4194, 37.7749);

        // Should return error for identical points
        let result = handler.plan_journey(AgentId::new(), point, point, Utc::now());

        assert!(result.is_err());
        if let Err(crate::Error::Journey(msg)) = result {
            assert!(msg.contains("invalid"));
        } else {
            panic!("Expected Journey error for identical origin/destination");
        }
    }

    #[test]
    fn test_integration_with_real_graph() {
        // Load real graph data and plan multiple journeys
        use crate::graph::RoadNode;

        let network = match RoadNetwork::load_from_file("test_data/small_city_graph.json") {
            Ok(net) => net,
            Err(_) => {
                // If test data doesn't exist, create a simple test network
                let mut net = RoadNetwork::new();
                for i in 0..10 {
                    net.add_node(RoadNode {
                        id: i,
                        position: Point::new(i as f64 * 0.001, i as f64 * 0.001),
                    });
                }
                net
            }
        };

        let handler = MovementHandler::new().with_network(network.clone());

        // Plan a journey using actual network nodes
        if let Some(first_node) = network.nodes.values().next()
            && let Some(last_node) = network.nodes.values().last()
        {
            let journey = handler
                .plan_journey(
                    AgentId::new(),
                    first_node.position,
                    last_node.position,
                    Utc::now(),
                )
                .unwrap();

            assert!(journey.distance_m > 0.0);
            assert!(journey.route.coords().count() >= 2);
        }
    }

    #[test]
    fn test_journey_with_network_completes() {
        use crate::graph::{RoadEdge, RoadNode, RoadType};

        let mut network = RoadNetwork::new();
        network.add_node(RoadNode {
            id: 1,
            position: Point::new(0.0, 0.0),
        });
        network.add_node(RoadNode {
            id: 2,
            position: Point::new(0.01, 0.0),
        });
        network.add_edge(RoadEdge {
            id: 1,
            from_node: 1,
            to_node: 2,
            distance_m: 500.0,
            geometry: LineString::from(vec![(0.0, 0.0), (0.01, 0.0)]),
            road_type: RoadType::Residential,
            speed_limit_mps: 8.0,
        });

        let mut handler = MovementHandler::new().with_network(network);

        // Plan and add journey
        let journey = handler
            .plan_journey(
                AgentId::new(),
                Point::new(0.0001, 0.0),
                Point::new(0.0099, 0.0),
                Utc::now(),
            )
            .unwrap();

        let journey_distance = journey.distance_m;
        handler.add_journey(journey);

        // Advance journey to completion based on actual distance
        // At 5 m/s, time = distance / velocity
        let time_needed = (journey_distance / 5.0).ceil() as i64 + 10; // Add buffer
        handler
            .advance_journeys(Utc::now(), Duration::seconds(time_needed))
            .unwrap();

        let completed = handler.get_completed_journeys();
        assert_eq!(completed.len(), 1, "Journey should be completed");
        assert_eq!(completed[0].progress, 1.0, "Progress should be 100%");
    }

    #[test]
    fn journeys_move_at_the_speed_of_the_current_time() {
        // Regression: the clock was reconstructed from start time and an
        // assumed 5 m/s, so a journey started off-peak never hit rush hour.
        use chrono::TimeZone;
        let off_peak = Utc.with_ymd_and_hms(2024, 1, 15, 6, 0, 0).unwrap();
        let rush = Utc.with_ymd_and_hms(2024, 1, 15, 8, 0, 0).unwrap();
        let progress_at = |now| {
            let mut handler = MovementHandler::new()
                .with_velocity_model(Box::new(velocity::TimeOfDayVelocity::default()));
            let journey = handler
                .plan_journey(
                    AgentId::new(),
                    Point::new(-122.42, 37.77),
                    Point::new(-122.40, 37.78),
                    off_peak,
                )
                .unwrap();
            let id = journey.id;
            handler.add_journey(journey);
            handler
                .advance_journeys(now, Duration::seconds(60))
                .unwrap();
            handler.active_journeys[&id].progress
        };
        let (normal, slow) = (progress_at(off_peak), progress_at(rush));
        assert!((slow - normal * 0.5).abs() < 1e-9, "{slow} vs {normal}");
    }
}
