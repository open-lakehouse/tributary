//! Integration tests for the complete road network system
//!
//! These tests verify the end-to-end functionality of:
//! - Journey planning with road networks
//! - Journey execution and completion
//! - Time-of-day velocity modeling
//! - Network vs straight-line routing
//! - Graceful fallback when network is unavailable

use chrono::{Duration, TimeZone, Utc};
use geo_types::Point;
use sim_core::utils::idents::AgentId;
use sim_geo::{MovementHandler, RoadNetwork, TimeOfDayVelocity};

/// Helper to create test data path
fn test_data_path(filename: &str) -> String {
    format!("test_data/{}", filename)
}

/// Test 1: Full journey lifecycle (plan → execute → complete)
///
/// This test verifies the complete journey lifecycle from planning through
/// execution to completion using a real road network.
#[test]
fn test_full_journey_lifecycle() {
    // Load real network
    let network = RoadNetwork::load_from_file(&test_data_path("small_city_graph.json"))
        .expect("Failed to load test network");

    let mut handler = MovementHandler::new().with_network(network.clone());

    // Get two distant nodes from the network
    let node_ids: Vec<_> = network.nodes.keys().copied().collect();
    let origin_node = network.get_node(node_ids[0]).unwrap();
    let dest_node = network.get_node(node_ids[node_ids.len() - 1]).unwrap();

    let agent_id = AgentId::new();
    let start_time = Utc::now();

    // Phase 1: Plan the journey
    let journey = handler
        .plan_journey(
            agent_id,
            origin_node.position,
            dest_node.position,
            start_time,
        )
        .expect("Journey planning should succeed");

    assert_eq!(journey.agent_id, agent_id);
    assert_eq!(journey.progress, 0.0);
    assert!(journey.distance_m > 0.0);
    assert!(journey.route.coords().count() >= 2);

    let journey_id = journey.id;
    let total_distance = journey.distance_m;

    println!("Journey planned: {} meters", total_distance);

    // Phase 2: Execute the journey
    handler.add_journey(journey);
    assert_eq!(handler.active_count(), 1);

    // Advance journey in steps
    let mut step_count = 0;
    let time_step = Duration::seconds(10);
    let max_steps = 500; // Safety limit

    while step_count < max_steps {
        handler
            .advance_journeys(time_step)
            .expect("Journey advancement should succeed");
        step_count += 1;

        // Check if journey completed
        let completed = handler.get_completed_journeys();
        if !completed.is_empty() {
            break;
        }
    }

    println!("Journey completed in {} steps", step_count);
    assert!(
        step_count < max_steps,
        "Journey should complete within step limit"
    );

    // Phase 3: Verify completion
    let completed = handler.remove_completed();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].id, journey_id);
    assert_eq!(completed[0].progress, 1.0);
    assert_eq!(handler.active_count(), 0);
}

/// Test 2: Rush hour impact on travel time
///
/// This test verifies that the TimeOfDayVelocity model correctly adjusts
/// travel speeds based on time of day, with rush hour being significantly slower.
#[test]
fn test_rush_hour_impact_on_travel_time() {
    // Load network
    let network = RoadNetwork::load_from_file(&test_data_path("small_city_graph.json"))
        .expect("Failed to load test network");

    let node_ids: Vec<_> = network.nodes.keys().copied().collect();
    let origin = network.get_node(node_ids[0]).unwrap().position;
    let destination = network.get_node(node_ids[10]).unwrap().position;

    // Test 1: Rush hour journey (8 AM)
    let rush_hour_time = Utc.with_ymd_and_hms(2024, 1, 15, 8, 0, 0).unwrap();
    let velocity_model = Box::new(TimeOfDayVelocity::default());
    let mut rush_handler = MovementHandler::new()
        .with_network(network.clone())
        .with_velocity_model(velocity_model);

    let rush_journey = rush_handler
        .plan_journey(AgentId::new(), origin, destination, rush_hour_time)
        .expect("Rush hour journey planning should succeed");

    let rush_distance = rush_journey.distance_m;
    rush_handler.add_journey(rush_journey);

    // Advance until complete
    let mut rush_steps = 0;
    let time_step = Duration::seconds(10);
    while rush_steps < 1000 {
        rush_handler.advance_journeys(time_step).unwrap();
        rush_steps += 1;

        // Check if journey completed
        let completed = rush_handler.get_completed_journeys();
        if !completed.is_empty() {
            break;
        }
    }
    let rush_time_seconds = rush_steps * 10;

    println!(
        "Rush hour: {} meters in {} seconds ({} steps)",
        rush_distance, rush_time_seconds, rush_steps
    );

    // Test 2: Off-peak journey (2 PM)
    let off_peak_time = Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap();
    let velocity_model = Box::new(TimeOfDayVelocity::default());
    let mut off_peak_handler = MovementHandler::new()
        .with_network(network.clone())
        .with_velocity_model(velocity_model);

    let off_peak_journey = off_peak_handler
        .plan_journey(AgentId::new(), origin, destination, off_peak_time)
        .expect("Off-peak journey planning should succeed");

    off_peak_handler.add_journey(off_peak_journey);

    // Advance until complete
    let mut off_peak_steps = 0;
    while off_peak_steps < 1000 {
        off_peak_handler.advance_journeys(time_step).unwrap();
        off_peak_steps += 1;

        // Check if journey completed
        let completed = off_peak_handler.get_completed_journeys();
        if !completed.is_empty() {
            break;
        }
    }
    let off_peak_time_seconds = off_peak_steps * 10;

    println!(
        "Off-peak: {} meters in {} seconds ({} steps)",
        rush_distance, off_peak_time_seconds, off_peak_steps
    );

    // Test 3: Night time journey (2 AM)
    let night_time = Utc.with_ymd_and_hms(2024, 1, 15, 2, 0, 0).unwrap();
    let velocity_model = Box::new(TimeOfDayVelocity::default());
    let mut night_handler = MovementHandler::new()
        .with_network(network)
        .with_velocity_model(velocity_model);

    let night_journey = night_handler
        .plan_journey(AgentId::new(), origin, destination, night_time)
        .expect("Night journey planning should succeed");

    night_handler.add_journey(night_journey);

    // Advance until complete
    let mut night_steps = 0;
    while night_steps < 1000 {
        night_handler.advance_journeys(time_step).unwrap();
        night_steps += 1;

        // Check if journey completed
        let completed = night_handler.get_completed_journeys();
        if !completed.is_empty() {
            break;
        }
    }
    let night_time_seconds = night_steps * 10;

    println!(
        "Night time: {} meters in {} seconds ({} steps)",
        rush_distance, night_time_seconds, night_steps
    );

    // Verify relationships:
    // Rush hour (0.5x) should take roughly 2x longer than off-peak (1.0x)
    // Night time (1.2x) should be faster or equal to off-peak (due to discrete timesteps)
    assert!(
        rush_steps > off_peak_steps,
        "Rush hour should take more steps than off-peak"
    );
    assert!(
        night_steps <= off_peak_steps,
        "Night time should take fewer or equal steps compared to off-peak"
    );

    // Check rough multiplier relationship (with some tolerance)
    let rush_off_peak_ratio = rush_steps as f64 / off_peak_steps as f64;
    assert!(
        rush_off_peak_ratio > 1.5 && rush_off_peak_ratio < 2.5,
        "Rush hour should take roughly 2x longer (actual: {:.2}x)",
        rush_off_peak_ratio
    );

    // Night should be faster in total time (may have same steps due to discrete timesteps)
    assert!(
        night_time_seconds <= off_peak_time_seconds,
        "Night time should take less or equal time compared to off-peak"
    );
}

/// Test 3: Concurrent journeys on same network
///
/// This test verifies that multiple agents can have simultaneous journeys
/// on the same network without interference.
#[test]
fn test_concurrent_journeys_on_same_network() {
    let network = RoadNetwork::load_from_file(&test_data_path("small_city_graph.json"))
        .expect("Failed to load test network");

    let mut handler = MovementHandler::new().with_network(network.clone());

    // Get multiple node pairs
    let node_ids: Vec<_> = network.nodes.keys().copied().collect();
    let num_journeys = 5;
    let start_time = Utc::now();

    // Plan multiple journeys
    let mut journey_ids = Vec::new();
    for i in 0..num_journeys {
        let origin_node = network.get_node(node_ids[i * 10]).unwrap();
        let dest_node = network.get_node(node_ids[(i * 10) + 15]).unwrap();

        let journey = handler
            .plan_journey(
                AgentId::new(),
                origin_node.position,
                dest_node.position,
                start_time,
            )
            .expect("Journey planning should succeed");

        journey_ids.push(journey.id);
        handler.add_journey(journey);
    }

    assert_eq!(handler.active_count(), num_journeys);
    println!("Started {} concurrent journeys", num_journeys);

    // Execute all journeys
    let time_step = Duration::seconds(10);
    let mut step_count = 0;
    let max_steps = 500;

    while handler.active_count() > 0 && step_count < max_steps {
        handler
            .advance_journeys(time_step)
            .expect("Should handle concurrent journeys");
        step_count += 1;
    }

    // Verify all completed
    let completed = handler.remove_completed();
    assert_eq!(
        completed.len(),
        num_journeys,
        "All journeys should complete"
    );

    for journey in &completed {
        assert_eq!(journey.progress, 1.0);
        assert!(journey_ids.contains(&journey.id));
    }

    println!(
        "All {} journeys completed in {} steps",
        num_journeys, step_count
    );
}

/// Test 4: Network vs straight-line routing comparison
///
/// This test compares network-based routing with straight-line routing
/// to verify that network routes are typically longer (following roads).
#[test]
fn test_network_vs_straight_line_routing() {
    let network = RoadNetwork::load_from_file(&test_data_path("small_city_graph.json"))
        .expect("Failed to load test network");

    let node_ids: Vec<_> = network.nodes.keys().copied().collect();
    let origin = network.get_node(node_ids[0]).unwrap().position;
    let destination = network.get_node(node_ids[50]).unwrap().position;

    let start_time = Utc::now();
    let agent_id = AgentId::new();

    // Journey with network routing
    let handler_with_network = MovementHandler::new().with_network(network);
    let network_journey = handler_with_network
        .plan_journey(agent_id, origin, destination, start_time)
        .expect("Network journey should plan");

    let network_distance = network_journey.distance_m;
    let network_coord_count = network_journey.route.coords().count();

    // Journey without network (straight line)
    let handler_without_network = MovementHandler::new();
    let straight_journey = handler_without_network
        .plan_journey(agent_id, origin, destination, start_time)
        .expect("Straight line journey should plan");

    let straight_distance = straight_journey.distance_m;
    let straight_coord_count = straight_journey.route.coords().count();

    println!(
        "Network route: {} meters with {} waypoints",
        network_distance, network_coord_count
    );
    println!(
        "Straight line: {} meters with {} waypoints",
        straight_distance, straight_coord_count
    );

    // Verifications:
    // 1. Network route should have more waypoints (follows roads)
    assert!(
        network_coord_count >= straight_coord_count,
        "Network route should have at least as many waypoints"
    );

    // 2. Network route is typically longer (roads aren't straight)
    // However, in some cases they might be similar if the road is nearly straight
    assert!(
        network_distance >= straight_distance * 0.95,
        "Network distance should be comparable to straight line"
    );

    // 3. Straight line should be exactly 2 points
    assert_eq!(
        straight_coord_count, 2,
        "Straight line should have exactly 2 points"
    );

    // 4. Both should be positive distances
    assert!(network_distance > 0.0);
    assert!(straight_distance > 0.0);
}

/// Test 5: Graceful fallback when network unavailable
///
/// This test verifies that the system gracefully falls back to straight-line
/// routing when nodes are too far from the network or the network is not loaded.
#[test]
fn test_graceful_fallback_when_network_unavailable() {
    let network = RoadNetwork::load_from_file(&test_data_path("small_city_graph.json"))
        .expect("Failed to load test network");

    let handler = MovementHandler::new().with_network(network.clone());

    let start_time = Utc::now();
    let agent_id = AgentId::new();

    // Test 1: Points far from network should fallback to straight line
    let far_origin = Point::new(0.0, 0.0); // Far from San Francisco test network
    let far_destination = Point::new(1.0, 1.0);

    let far_journey = handler
        .plan_journey(agent_id, far_origin, far_destination, start_time)
        .expect("Should plan journey even with distant points");

    // Should fallback to straight line (2 points)
    assert_eq!(
        far_journey.route.coords().count(),
        2,
        "Distant points should use straight line fallback"
    );
    assert!(far_journey.distance_m > 0.0);

    println!("Far points fallback: {} meters", far_journey.distance_m);

    // Test 2: Handler without network should always use straight line
    let no_network_handler = MovementHandler::new();

    let node_ids: Vec<_> = network.nodes.keys().copied().collect();
    let origin = network.get_node(node_ids[0]).unwrap().position;
    let destination = network.get_node(node_ids[20]).unwrap().position;

    let straight_journey = no_network_handler
        .plan_journey(agent_id, origin, destination, start_time)
        .expect("Should plan straight line journey");

    assert_eq!(
        straight_journey.route.coords().count(),
        2,
        "No network should always use straight line"
    );
    assert!(straight_journey.distance_m > 0.0);

    println!("No network: {} meters", straight_journey.distance_m);

    // Test 3: Verify journeys can still complete without network
    let mut handler_no_net = MovementHandler::new();
    handler_no_net.add_journey(straight_journey);

    let time_step = Duration::seconds(10);
    let mut steps = 0;
    while handler_no_net.active_count() > 0 && steps < 500 {
        handler_no_net.advance_journeys(time_step).unwrap();
        steps += 1;
    }

    let completed = handler_no_net.remove_completed();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].progress, 1.0);

    println!("Journey completed without network in {} steps", steps);
}

/// Test 6: Verify TimeOfDayVelocity returns correct speeds
///
/// This is a sanity check to ensure the velocity model is working correctly.
#[test]
fn test_velocity_model_speeds() {
    use sim_geo::RoadType;

    let model = TimeOfDayVelocity::default();

    // Rush hour (8 AM)
    let rush_time = Utc.with_ymd_and_hms(2024, 1, 15, 8, 0, 0).unwrap();
    let rush_speed = model.calculate_velocity(&RoadType::Residential, rush_time);
    println!("Rush hour speed (Residential): {} m/s", rush_speed);
    assert_eq!(rush_speed, 4.0, "Rush hour should be 8.0 * 0.5 = 4.0 m/s");

    // Off-peak (2 PM)
    let off_peak_time = Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap();
    let off_peak_speed = model.calculate_velocity(&RoadType::Residential, off_peak_time);
    println!("Off-peak speed (Residential): {} m/s", off_peak_speed);
    assert_eq!(
        off_peak_speed, 8.0,
        "Off-peak should be 8.0 * 1.0 = 8.0 m/s"
    );

    // Night (2 AM)
    let night_time = Utc.with_ymd_and_hms(2024, 1, 15, 2, 0, 0).unwrap();
    let night_speed = model.calculate_velocity(&RoadType::Residential, night_time);
    println!("Night speed (Residential): {} m/s", night_speed);
    assert_eq!(night_speed, 9.6, "Night should be 8.0 * 1.2 = 9.6 m/s");
}

/// Test 7: Verify time-of-day boundaries
///
/// This test verifies exact time boundaries for rush hour detection.
#[test]
fn test_time_of_day_boundaries() {
    use sim_geo::movement::velocity::TimeOfDayVelocity;

    // Test morning rush hour boundary (7 AM - 9 AM)
    let before_morning_rush = Utc.with_ymd_and_hms(2024, 1, 15, 6, 59, 59).unwrap();
    let morning_rush_start = Utc.with_ymd_and_hms(2024, 1, 15, 7, 0, 0).unwrap();
    let morning_rush_mid = Utc.with_ymd_and_hms(2024, 1, 15, 8, 30, 0).unwrap();
    let morning_rush_end = Utc.with_ymd_and_hms(2024, 1, 15, 8, 59, 59).unwrap();
    let after_morning_rush = Utc.with_ymd_and_hms(2024, 1, 15, 9, 0, 0).unwrap();

    assert!(!TimeOfDayVelocity::is_rush_hour(before_morning_rush));
    assert!(TimeOfDayVelocity::is_rush_hour(morning_rush_start));
    assert!(TimeOfDayVelocity::is_rush_hour(morning_rush_mid));
    assert!(TimeOfDayVelocity::is_rush_hour(morning_rush_end));
    assert!(!TimeOfDayVelocity::is_rush_hour(after_morning_rush));

    // Test evening rush hour boundary (5 PM - 7 PM, 17:00 - 19:00)
    let before_evening_rush = Utc.with_ymd_and_hms(2024, 1, 15, 16, 59, 59).unwrap();
    let evening_rush_start = Utc.with_ymd_and_hms(2024, 1, 15, 17, 0, 0).unwrap();
    let evening_rush_mid = Utc.with_ymd_and_hms(2024, 1, 15, 18, 30, 0).unwrap();
    let evening_rush_end = Utc.with_ymd_and_hms(2024, 1, 15, 18, 59, 59).unwrap();
    let after_evening_rush = Utc.with_ymd_and_hms(2024, 1, 15, 19, 0, 0).unwrap();

    assert!(!TimeOfDayVelocity::is_rush_hour(before_evening_rush));
    assert!(TimeOfDayVelocity::is_rush_hour(evening_rush_start));
    assert!(TimeOfDayVelocity::is_rush_hour(evening_rush_mid));
    assert!(TimeOfDayVelocity::is_rush_hour(evening_rush_end));
    assert!(!TimeOfDayVelocity::is_rush_hour(after_evening_rush));

    // Test night time boundary (10 PM - 5 AM, 22:00 - 4:59)
    let before_night = Utc.with_ymd_and_hms(2024, 1, 15, 21, 59, 59).unwrap();
    let night_start = Utc.with_ymd_and_hms(2024, 1, 15, 22, 0, 0).unwrap();
    let midnight = Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap();
    let night_end = Utc.with_ymd_and_hms(2024, 1, 15, 4, 59, 59).unwrap();
    let after_night = Utc.with_ymd_and_hms(2024, 1, 15, 5, 0, 0).unwrap();

    assert!(!TimeOfDayVelocity::is_night_time(before_night));
    assert!(TimeOfDayVelocity::is_night_time(night_start));
    assert!(TimeOfDayVelocity::is_night_time(midnight));
    assert!(TimeOfDayVelocity::is_night_time(night_end));
    assert!(!TimeOfDayVelocity::is_night_time(after_night));
}

/// Test 8: Multiple road types in journey
///
/// This test verifies that journeys can traverse multiple road types.
#[test]
fn test_multiple_road_types_in_journey() {
    let network = RoadNetwork::load_from_file(&test_data_path("small_city_graph.json"))
        .expect("Failed to load test network");

    // Analyze road types in the network
    let mut road_type_counts = std::collections::HashMap::new();
    for edge in network.edges.values() {
        *road_type_counts
            .entry(format!("{}", edge.road_type))
            .or_insert(0) += 1;
    }

    println!("Network road type distribution:");
    for (road_type, count) in &road_type_counts {
        println!("  {}: {} edges", road_type, count);
    }

    assert!(!network.edges.is_empty(), "Network should have edges");

    // Plan a long journey that likely crosses multiple road types
    let node_ids: Vec<_> = network.nodes.keys().copied().collect();
    let origin = network.get_node(node_ids[0]).unwrap().position;
    let destination = network
        .get_node(node_ids[node_ids.len() - 1])
        .unwrap()
        .position;

    let handler = MovementHandler::new().with_network(network);
    let journey = handler
        .plan_journey(AgentId::new(), origin, destination, Utc::now())
        .expect("Should plan long journey");

    assert!(journey.route.coords().count() >= 2);
    assert!(journey.distance_m > 0.0);

    println!(
        "Planned journey: {} meters with {} waypoints",
        journey.distance_m,
        journey.route.coords().count()
    );
}
