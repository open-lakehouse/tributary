//! Road Network Demo: TimeOfDay Velocity Model
//!
//! This example demonstrates:
//! - Loading a road network from JSON
//! - Planning journeys with network routing
//! - Using TimeOfDayVelocity model for rush hour simulation
//! - Comparing travel times across different times of day

use chrono::{Duration, TimeZone, Utc};
use sim_core::utils::idents::AgentId;
use sim_geo::{MovementHandler, RoadNetwork, RoadType, TimeOfDayVelocity};

fn main() {
    println!("========================================");
    println!("   Road Network Time-of-Day Demo");
    println!("========================================\n");

    // Load the road network
    println!("Loading road network from test_data/small_city_graph.json...");
    let network = match RoadNetwork::load_from_file("test_data/small_city_graph.json") {
        Ok(net) => {
            println!("✓ Network loaded successfully!");
            println!("  - Nodes: {}", net.node_count());
            println!("  - Edges: {}", net.edge_count());
            net
        }
        Err(e) => {
            eprintln!("✗ Failed to load network: {}", e);
            eprintln!("\nMake sure you run this from the sim-geo directory:");
            eprintln!("  cargo run --example road_network_demo");
            return;
        }
    };

    println!();

    // Analyze the network
    println!("Network Statistics:");
    println!("------------------");

    let mut road_type_counts = std::collections::HashMap::new();
    let mut total_distance = 0.0;

    for edge in network.edges.values() {
        *road_type_counts
            .entry(format!("{}", edge.road_type))
            .or_insert(0) += 1;
        total_distance += edge.distance_m;
    }

    println!("Road types:");
    for (road_type, count) in &road_type_counts {
        println!("  - {}: {} edges", road_type, count);
    }
    println!("Total network length: {:.2} km\n", total_distance / 1000.0);

    // Select origin and destination
    let node_ids: Vec<_> = network.nodes.keys().copied().collect();
    let origin_position = network.get_node(node_ids[0]).unwrap().position;
    let dest_position = network
        .get_node(node_ids[50.min(node_ids.len() - 1)])
        .unwrap()
        .position;

    println!("Journey Setup:");
    println!("--------------");
    println!(
        "Origin:      ({:.6}, {:.6})",
        origin_position.x(),
        origin_position.y()
    );
    println!(
        "Destination: ({:.6}, {:.6})",
        dest_position.x(),
        dest_position.y()
    );
    println!();

    // Demonstrate TimeOfDayVelocity model
    println!("Time-of-Day Velocity Model:");
    println!("---------------------------");
    let velocity_model = TimeOfDayVelocity::default();
    println!("Configuration:");
    println!(
        "  - Rush hour multiplier: {}",
        velocity_model.rush_hour_multiplier
    );
    println!(
        "  - Off-peak multiplier:  {}",
        velocity_model.off_peak_multiplier
    );
    println!(
        "  - Night multiplier:     {}",
        velocity_model.night_multiplier
    );
    println!();

    // Show speed calculations for different times and road types
    println!("Speed Examples (for different road types and times):");
    println!();

    let times = vec![
        (
            Utc.with_ymd_and_hms(2024, 1, 15, 8, 0, 0).unwrap(),
            "Rush Hour (8 AM)",
        ),
        (
            Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap(),
            "Off-Peak (2 PM)",
        ),
        (
            Utc.with_ymd_and_hms(2024, 1, 15, 2, 0, 0).unwrap(),
            "Night (2 AM)",
        ),
    ];

    let road_types = vec![
        RoadType::Highway,
        RoadType::Arterial,
        RoadType::Residential,
        RoadType::ServiceRoad,
    ];

    for (time, label) in &times {
        println!("{:20} | Highway | Arterial | Residential | Service", label);
        println!("{:-<20}-+---------+----------+-------------+---------", "");

        let speeds: Vec<f64> = road_types
            .iter()
            .map(|rt| velocity_model.calculate_velocity(rt, *time))
            .collect();

        print!("{:20} |", "Speed (m/s)");
        for speed in &speeds {
            print!(" {:7.1} |", speed);
        }
        println!();

        print!("{:20} |", "Speed (km/h)");
        for speed in &speeds {
            print!(" {:7.1} |", speed * 3.6);
        }
        println!("\n");
    }

    // Simulate journeys at different times
    println!("\n========================================");
    println!("   Journey Simulation Comparison");
    println!("========================================\n");

    let scenarios = vec![
        (
            Utc.with_ymd_and_hms(2024, 1, 15, 8, 0, 0).unwrap(),
            "Morning Rush Hour (8:00 AM)",
        ),
        (
            Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap(),
            "Off-Peak (2:00 PM)",
        ),
        (
            Utc.with_ymd_and_hms(2024, 1, 15, 18, 0, 0).unwrap(),
            "Evening Rush Hour (6:00 PM)",
        ),
        (
            Utc.with_ymd_and_hms(2024, 1, 15, 2, 0, 0).unwrap(),
            "Night Time (2:00 AM)",
        ),
    ];

    let mut results = Vec::new();

    for (start_time, label) in scenarios {
        println!("Scenario: {}", label);
        println!("{}", "-".repeat(label.len() + 10));

        // Create handler with velocity model
        let velocity_model = Box::new(TimeOfDayVelocity::default());
        let mut handler = MovementHandler::new()
            .with_network(network.clone())
            .with_velocity_model(velocity_model);

        // Plan journey
        let journey = handler
            .plan_journey(AgentId::new(), origin_position, dest_position, start_time)
            .expect("Journey planning should succeed");

        let distance = journey.distance_m;
        let waypoints = journey.route.coords().count();

        println!("  Route distance: {:.2} meters", distance);
        println!("  Waypoints:      {}", waypoints);

        // Add and execute journey
        handler.add_journey(journey);

        let time_step = Duration::seconds(10);
        let mut steps = 0;
        let max_steps = 1000;

        while steps < max_steps {
            handler.advance_journeys(time_step).unwrap();
            steps += 1;

            // Check if journey completed
            let completed = handler.get_completed_journeys();
            if !completed.is_empty() {
                break;
            }
        }

        let completed = handler.remove_completed();
        if !completed.is_empty() {
            let travel_time = steps * 10;
            let avg_speed = distance / travel_time as f64;

            println!(
                "  Travel time:    {} seconds ({:.1} minutes)",
                travel_time,
                travel_time as f64 / 60.0
            );
            println!(
                "  Avg speed:      {:.2} m/s ({:.1} km/h)",
                avg_speed,
                avg_speed * 3.6
            );

            results.push((label, distance, travel_time, avg_speed));
        } else {
            println!("  ✗ Journey did not complete");
        }

        println!();
    }

    // Summary comparison
    println!("========================================");
    println!("   Summary Comparison");
    println!("========================================\n");

    if results.len() >= 2 {
        let (off_peak_label, _off_peak_dist, off_peak_time, off_peak_speed) = results[1];

        println!("Relative to Off-Peak ({})", off_peak_label);
        println!("{:-<60}", "");

        for (label, _distance, travel_time, avg_speed) in &results {
            let time_ratio = *travel_time as f64 / off_peak_time as f64;
            let speed_ratio = avg_speed / off_peak_speed;

            println!(
                "{:30} | Time: {:5.1}x | Speed: {:5.1}x",
                label, time_ratio, speed_ratio
            );
        }

        println!();
        println!("Key Insights:");
        println!("  • Rush hour journeys take approximately 2x longer");
        println!("  • Night time journeys are ~20% faster");
        println!("  • Same distance, different travel times based on congestion");
    }

    // Demonstrate route details
    println!("\n========================================");
    println!("   Route Details Example");
    println!("========================================\n");

    let handler = MovementHandler::new().with_network(network);
    let journey = handler
        .plan_journey(AgentId::new(), origin_position, dest_position, Utc::now())
        .unwrap();

    println!("Route waypoints (first 10):");
    for (i, coord) in journey.route.coords().enumerate().take(10) {
        println!("  {}: ({:.6}, {:.6})", i, coord.x, coord.y);
    }

    if journey.route.coords().count() > 10 {
        println!(
            "  ... ({} more waypoints)",
            journey.route.coords().count() - 10
        );
    }

    println!("\n========================================");
    println!("   Demo Complete!");
    println!("========================================");
}
