//! Example comparing journey planning with and without road network routing
//!
//! This example demonstrates the difference between:
//! 1. Straight-line routing (no network)
//! 2. Network-aware routing (follows actual roads)

use geo_types::Point;
use sim_core::utils::idents::AgentId;
use sim_geo::MovementHandler;
use sim_geo::graph::{RoadEdge, RoadNetwork, RoadNode, RoadType};

fn main() {
    println!("=== Network Routing Comparison ===\n");

    // Define journey endpoints
    let origin = Point::new(0.0, 0.0);
    let destination = Point::new(0.01, 0.01);

    // === SCENARIO 1: Without Network (Straight Line) ===
    println!("SCENARIO 1: Without Network (Straight Line)");
    println!("  Origin:      ({}, {})", origin.x(), origin.y());
    println!("  Destination: ({}, {})", destination.x(), destination.y());

    let handler_no_network = MovementHandler::new();
    let journey_straight = handler_no_network
        .plan_journey(AgentId::new(), origin, destination, chrono::Utc::now())
        .unwrap();

    println!(
        "  Route points: {}",
        journey_straight.route.coords().count()
    );
    println!("  Distance: {:.1} meters", journey_straight.distance_m);
    println!("  Path: Direct straight line\n");

    // === SCENARIO 2: With Network (Road Following) ===
    println!("SCENARIO 2: With Network (Road Following)");

    // Create a road network with curved roads
    let mut network = RoadNetwork::new();

    // Add intersection nodes
    network.add_node(RoadNode {
        id: 1,
        position: Point::new(0.0, 0.0),
    });
    network.add_node(RoadNode {
        id: 2,
        position: Point::new(0.005, 0.003),
    });
    network.add_node(RoadNode {
        id: 3,
        position: Point::new(0.01, 0.01),
    });

    // Add roads connecting nodes (with realistic curvature)
    network.add_edge(RoadEdge {
        id: 1,
        from_node: 1,
        to_node: 2,
        distance_m: 750.0,
        geometry: geo_types::LineString::from(vec![
            (0.0, 0.0),
            (0.002, 0.001),
            (0.004, 0.002),
            (0.005, 0.003),
        ]),
        road_type: RoadType::Residential,
        speed_limit_mps: 8.0,
    });

    network.add_edge(RoadEdge {
        id: 2,
        from_node: 2,
        to_node: 3,
        distance_m: 800.0,
        geometry: geo_types::LineString::from(vec![
            (0.005, 0.003),
            (0.006, 0.005),
            (0.008, 0.008),
            (0.01, 0.01),
        ]),
        road_type: RoadType::Arterial,
        speed_limit_mps: 15.0,
    });

    println!(
        "  Network: {} nodes, {} edges",
        network.node_count(),
        network.edge_count()
    );

    let handler_with_network = MovementHandler::new().with_network(network.clone());
    let journey_network = handler_with_network
        .plan_journey(AgentId::new(), origin, destination, chrono::Utc::now())
        .unwrap();

    println!("  Route points: {}", journey_network.route.coords().count());
    println!("  Distance: {:.1} meters", journey_network.distance_m);
    println!("  Path: Follows road network geometry");

    // Show route details
    println!("\n  Route details:");
    for (i, coord) in journey_network.route.coords().enumerate() {
        println!("    Point {}: ({:.5}, {:.5})", i + 1, coord.x, coord.y);
    }

    // === COMPARISON ===
    println!("\n=== COMPARISON ===");
    let distance_increase_pct = ((journey_network.distance_m - journey_straight.distance_m)
        / journey_straight.distance_m)
        * 100.0;

    println!(
        "Distance increase: {:.1}% ({:.1}m -> {:.1}m)",
        distance_increase_pct, journey_straight.distance_m, journey_network.distance_m
    );
    println!(
        "Route complexity: {} points vs {} points",
        journey_network.route.coords().count(),
        journey_straight.route.coords().count()
    );

    println!("\nWith network routing, journeys follow realistic road paths");
    println!("rather than unrealistic straight lines through obstacles.");

    // === SCENARIO 3: Disconnected Network (Fallback) ===
    println!("\n=== SCENARIO 3: Disconnected Network (Fallback) ===");

    let mut disconnected_network = RoadNetwork::new();

    // Add two separate connected components
    disconnected_network.add_node(RoadNode {
        id: 1,
        position: Point::new(0.0, 0.0),
    });
    disconnected_network.add_node(RoadNode {
        id: 2,
        position: Point::new(0.001, 0.0),
    });
    disconnected_network.add_node(RoadNode {
        id: 10,
        position: Point::new(0.009, 0.01),
    });
    disconnected_network.add_node(RoadNode {
        id: 11,
        position: Point::new(0.01, 0.01),
    });

    // Connect first component (1-2)
    disconnected_network.add_edge(RoadEdge {
        id: 1,
        from_node: 1,
        to_node: 2,
        distance_m: 100.0,
        geometry: geo_types::LineString::from(vec![(0.0, 0.0), (0.001, 0.0)]),
        road_type: RoadType::Residential,
        speed_limit_mps: 8.0,
    });

    // Connect second component (10-11) - separate!
    disconnected_network.add_edge(RoadEdge {
        id: 2,
        from_node: 10,
        to_node: 11,
        distance_m: 100.0,
        geometry: geo_types::LineString::from(vec![(0.009, 0.01), (0.01, 0.01)]),
        road_type: RoadType::Residential,
        speed_limit_mps: 8.0,
    });

    let handler_disconnected = MovementHandler::new().with_network(disconnected_network);

    let journey_fallback = handler_disconnected
        .plan_journey(AgentId::new(), origin, destination, chrono::Utc::now())
        .unwrap();

    println!("  No path found in network - falling back to straight line");
    println!(
        "  Route points: {}",
        journey_fallback.route.coords().count()
    );
    println!("  Distance: {:.1} meters", journey_fallback.distance_m);

    println!("\n=== Summary ===");
    println!("MovementHandler intelligently chooses between network routing");
    println!("and straight-line fallback based on network connectivity.");
}
