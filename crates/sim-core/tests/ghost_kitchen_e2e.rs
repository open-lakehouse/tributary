//! End-to-End Ghost Kitchen Delivery Simulation
//!
//! This test demonstrates the complete integration of the simulation framework:
//! - Road network routing with realistic travel
//! - Time-of-day velocity modeling (rush hour impact)
//! - Multi-agent coordination (delivery drivers)
//! - Complete telemetry generation (logs, traces, metrics)
//! - OTLP export for observability backends
//!
//! Scenario: A ghost kitchen in San Francisco processes delivery orders.
//! 10 delivery drivers handle 100 orders over 1 simulated hour.

use chrono::{Duration, TimeZone, Utc};
use geo_types::Point;
use serde_json::json;
use sim_core::utils::idents::{AgentId, JourneyId};
use sim_telemetry::core::{TelemetryCollector, TelemetryRecord};
use sim_telemetry::formats::OtlpExporter;
use sim_telemetry::generators::{
    EventToLogGenerator, EventToMetricGenerator, EventToTraceGenerator,
};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

// Ghost kitchen location in San Francisco (approximate downtown location)
const KITCHEN_LOCATION: (f64, f64) = (-122.4194, 37.7749);

// Delivery destination coordinates (various SF neighborhoods)
// These are real locations in San Francisco spread across the city
const DELIVERY_LOCATIONS: &[(f64, f64)] = &[
    (-122.4094, 37.7849), // North - Russian Hill
    (-122.4294, 37.7649), // South - Mission District
    (-122.3894, 37.7749), // East - Financial District
    (-122.4494, 37.7749), // West - Richmond District
    (-122.4144, 37.7949), // Northwest - Marina
    (-122.4344, 37.7549), // Southwest - Noe Valley
    (-122.4044, 37.7649), // Southeast - SoMa
    (-122.4394, 37.7849), // West Central - Western Addition
    (-122.4244, 37.7899), // North Central - Pacific Heights
    (-122.4144, 37.7599), // South Central - Castro
];

/// Delivery order state
#[derive(Debug, Clone)]
struct DeliveryOrder {
    order_id: String,
    destination: Point<f64>,
    #[allow(dead_code)]
    order_time: chrono::DateTime<chrono::Utc>,
    assigned_driver_id: Option<AgentId>,
    journey_id: Option<JourneyId>,
    #[allow(dead_code)]
    completion_time: Option<chrono::DateTime<chrono::Utc>>,
}

/// Delivery driver state
#[derive(Debug, Clone)]
enum DriverStatus {
    Idle,
    EnRouteToDelivery {
        order_id: String,
        journey_id: JourneyId,
    },
    Returning {
        journey_id: JourneyId,
    },
}

#[derive(Debug, Clone)]
struct DeliveryDriver {
    driver_id: AgentId,
    current_location: Point<f64>,
    status: DriverStatus,
}

impl DeliveryDriver {
    fn new(driver_id: AgentId, location: Point<f64>) -> Self {
        Self {
            driver_id,
            current_location: location,
            status: DriverStatus::Idle,
        }
    }

    fn is_idle(&self) -> bool {
        matches!(self.status, DriverStatus::Idle)
    }
}

/// Simulation statistics for reporting
#[derive(Debug, Default)]
struct SimulationStats {
    total_orders: usize,
    completed_deliveries: usize,
    orders_assigned: usize,
    total_distance_m: f64,
    total_delivery_time_secs: i64,
    rush_hour_deliveries: usize,
    off_peak_deliveries: usize,
    night_deliveries: usize,
}

/// Helper to determine time period
fn get_time_period(time: chrono::DateTime<chrono::Utc>) -> &'static str {
    use sim_geo::movement::velocity::TimeOfDayVelocity;

    if TimeOfDayVelocity::is_rush_hour(time) {
        "rush_hour"
    } else if TimeOfDayVelocity::is_night_time(time) {
        "night"
    } else {
        "off_peak"
    }
}

/// Main ghost kitchen delivery simulation test
#[test]
fn test_ghost_kitchen_delivery_simulation() {
    println!("\n========================================");
    println!("GHOST KITCHEN DELIVERY SIMULATION - E2E");
    println!("========================================\n");

    // 1. Setup infrastructure
    println!("1. Setting up simulation infrastructure...");

    let network = load_san_francisco_network();
    println!(
        "   - Loaded road network: {} nodes, {} edges",
        network.nodes.len(),
        network.edges.len()
    );

    let velocity_model = Box::new(sim_geo::movement::velocity::TimeOfDayVelocity::default());
    let mut movement_handler = sim_geo::MovementHandler::new()
        .with_network(network)
        .with_velocity_model(velocity_model);
    println!("   - Initialized movement handler with time-of-day velocity model");

    let mut telemetry = setup_telemetry_collector();
    println!("   - Configured telemetry collector (logs, traces, metrics)");

    // 2. Initialize simulation state
    println!("\n2. Initializing simulation state...");

    let kitchen = Point::new(KITCHEN_LOCATION.0, KITCHEN_LOCATION.1);
    let mut drivers = create_drivers(10, kitchen);
    let mut orders = create_orders(100);
    println!("   - Created {} drivers at kitchen location", drivers.len());
    println!("   - Generated {} delivery orders", orders.len());

    // 3. Run simulation starting at 8 AM (rush hour)
    println!("\n3. Running simulation...");

    let start_time = Utc.with_ymd_and_hms(2024, 1, 15, 8, 0, 0).unwrap();
    let mut current_time = start_time;
    let end_time = start_time + Duration::hours(1);

    let mut completed_orders = Vec::new();
    let mut stats = SimulationStats {
        total_orders: orders.len(),
        ..Default::default()
    };

    let mut tick = 0u64;
    let time_step = Duration::seconds(30); // 30-second time steps

    // Record simulation start
    emit_event(
        &mut telemetry,
        json!({
            "id": format!("sim-start-{}", tick),
            "type": "com.simulation.started",
            "source": "urn:sim:ghost_kitchen",
            "time": current_time.to_rfc3339(),
            "data": {
                "total_orders": stats.total_orders,
                "total_drivers": drivers.len(),
                "start_time": current_time.to_rfc3339(),
                "end_time": end_time.to_rfc3339()
            }
        }),
    );

    while current_time < end_time && (!orders.is_empty() || has_active_deliveries(&drivers)) {
        // Assign orders to idle drivers
        assign_orders(
            &mut drivers,
            &mut orders,
            &kitchen,
            &mut movement_handler,
            current_time,
            &mut telemetry,
            &mut stats,
        );

        // Advance all journeys
        if let Err(e) = movement_handler.advance_journeys(current_time, time_step) {
            eprintln!("   ERROR: Failed to advance journeys: {}", e);
            break;
        }

        // Check for completed deliveries
        let completed_journeys = movement_handler.get_completed_journeys();
        for journey in &completed_journeys {
            handle_completed_journey(
                journey,
                &mut drivers,
                &mut completed_orders,
                &kitchen,
                &mut movement_handler,
                current_time,
                &mut telemetry,
                &mut stats,
            );
        }

        // Remove completed journeys
        movement_handler.remove_completed();

        // Emit periodic telemetry
        if tick.is_multiple_of(10) {
            emit_event(
                &mut telemetry,
                json!({
                    "id": format!("tick-{}", tick),
                    "type": "com.simulation.tick",
                    "source": "urn:sim:ghost_kitchen",
                    "time": current_time.to_rfc3339(),
                    "data": {
                        "tick": tick,
                        "active_drivers": drivers.iter().filter(|d| !d.is_idle()).count(),
                        "idle_drivers": drivers.iter().filter(|d| d.is_idle()).count(),
                        "pending_orders": orders.len(),
                        "completed_orders": completed_orders.len(),
                        "active_journeys": movement_handler.active_count(),
                        "time_period": get_time_period(current_time)
                    }
                }),
            );
        }

        // Advance time
        current_time += time_step;
        tick += 1;

        // Safety limit
        if tick > 1000 {
            println!("   WARNING: Hit tick limit (1000), stopping simulation");
            break;
        }
    }

    // Record simulation end
    emit_event(
        &mut telemetry,
        json!({
            "id": format!("sim-end-{}", tick),
            "type": "com.simulation.ended",
            "source": "urn:sim:ghost_kitchen",
            "time": current_time.to_rfc3339(),
            "data": {
                "total_ticks": tick,
                "completed_orders": completed_orders.len(),
                "remaining_orders": orders.len(),
                "final_time": current_time.to_rfc3339()
            }
        }),
    );

    println!("   - Simulation completed after {} ticks", tick);
    println!(
        "   - Simulation time: {} seconds",
        (current_time - start_time).num_seconds()
    );

    // 4. Collect and analyze telemetry
    println!("\n4. Collecting telemetry data...");
    let telemetry_records = telemetry.flush();

    let log_count = telemetry_records
        .iter()
        .filter(|r| matches!(r, TelemetryRecord::Log { .. }))
        .count();
    let trace_count = telemetry_records
        .iter()
        .filter(|r| matches!(r, TelemetryRecord::Trace { .. }))
        .count();
    let metric_count = telemetry_records
        .iter()
        .filter(|r| matches!(r, TelemetryRecord::Metric { .. }))
        .count();

    println!("   - Total telemetry records: {}", telemetry_records.len());
    println!("     - Logs: {}", log_count);
    println!("     - Traces: {}", trace_count);
    println!("     - Metrics: {}", metric_count);

    // 5. Export to OTLP
    println!("\n5. Exporting telemetry to OTLP format...");
    let export_results = export_telemetry(&telemetry_records);
    println!("   - Exported {} data types to OTLP", export_results.len());

    // 6. Generate summary report
    println!("\n6. Simulation Results:");
    print_simulation_summary(&stats, start_time, current_time);

    // 7. Print sample telemetry
    println!("\n7. Sample Telemetry Records:");
    print_sample_telemetry(&telemetry_records);

    // 8. Assertions
    println!("\n8. Validation:");
    assert!(
        !completed_orders.is_empty(),
        "Should complete at least some deliveries"
    );
    println!("   ✓ Completed deliveries: {}", completed_orders.len());

    assert!(!telemetry_records.is_empty(), "Should generate telemetry");
    println!(
        "   ✓ Generated telemetry records: {}",
        telemetry_records.len()
    );

    assert!(log_count > 0, "Should generate log records");
    println!("   ✓ Log records present");

    assert!(trace_count > 0, "Should generate trace records");
    println!("   ✓ Trace records present");

    assert!(metric_count > 0, "Should generate metric records");
    println!("   ✓ Metric records present");

    assert!(stats.completed_deliveries > 0, "Should complete deliveries");
    println!("   ✓ Deliveries completed: {}", stats.completed_deliveries);

    println!("\n========================================");
    println!("SIMULATION COMPLETED SUCCESSFULLY");
    println!("========================================\n");
}

/// Test that rush hour significantly impacts delivery times
#[test]
fn test_rush_hour_impact_on_deliveries() {
    println!("\n=== Testing Rush Hour Impact ===\n");

    // Test the velocity model directly
    use sim_geo::RoadType;
    use sim_geo::movement::velocity::TimeOfDayVelocity;

    let model = TimeOfDayVelocity::default();

    // Rush hour (8 AM)
    let rush_time = Utc.with_ymd_and_hms(2024, 1, 15, 8, 0, 0).unwrap();
    let rush_speed = model.calculate_velocity(&RoadType::Residential, rush_time);

    // Off-peak (2 PM)
    let off_peak_time = Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap();
    let off_peak_speed = model.calculate_velocity(&RoadType::Residential, off_peak_time);

    // Night (2 AM)
    let night_time = Utc.with_ymd_and_hms(2024, 1, 15, 2, 0, 0).unwrap();
    let night_speed = model.calculate_velocity(&RoadType::Residential, night_time);

    println!("Velocity Model (Residential roads):");
    println!("  Rush hour (8 AM): {:.1} m/s", rush_speed);
    println!("  Off-peak (2 PM): {:.1} m/s", off_peak_speed);
    println!("  Night (2 AM): {:.1} m/s", night_speed);

    // Verify expected speed relationships
    assert_eq!(rush_speed, 4.0, "Rush hour should be 4.0 m/s (8.0 * 0.5)");
    assert_eq!(
        off_peak_speed, 8.0,
        "Off-peak should be 8.0 m/s (8.0 * 1.0)"
    );
    assert_eq!(night_speed, 9.6, "Night should be 9.6 m/s (8.0 * 1.2)");

    // Rush hour is 50% slower than off-peak (2x as long)
    let rush_off_peak_ratio = off_peak_speed / rush_speed;
    println!("\n  Rush hour slowdown factor: {:.2}x", rush_off_peak_ratio);
    assert_eq!(rush_off_peak_ratio, 2.0, "Rush hour should be 2x slower");

    // Night is 20% faster than off-peak
    let night_off_peak_ratio = night_speed / off_peak_speed;
    println!("  Night speedup factor: {:.2}x", night_off_peak_ratio);
    assert_eq!(night_off_peak_ratio, 1.2, "Night should be 1.2x faster");

    println!("\n  ✓ Rush hour impact verified (2x slower during rush hour)");
    println!("  ✓ Night speedup verified (1.2x faster at night)");
    println!("\n✓ Time-of-day velocity model working correctly\n");
}

/// Test telemetry completeness
#[test]
fn test_telemetry_completeness() {
    println!("\n=== Testing Telemetry Completeness ===\n");

    let mut telemetry = setup_telemetry_collector();

    // Emit various event types
    let events = vec![
        json!({
            "id": "order-assigned-001",
            "type": "com.simulation.order_assigned",
            "source": "urn:sim:driver:driver-001",
            "time": Utc::now().to_rfc3339(),
            "data": {"order_id": "ORDER-001", "driver_id": "driver-001"}
        }),
        json!({
            "id": "delivery-completed-001",
            "type": "com.simulation.delivery_completed",
            "source": "urn:sim:driver:driver-001",
            "time": Utc::now().to_rfc3339(),
            "data": {"order_id": "ORDER-001", "duration_secs": 300}
        }),
        json!({
            "id": "metric-001",
            "type": "com.simulation.metric",
            "source": "urn:sim:metrics",
            "time": Utc::now().to_rfc3339(),
            "data": {"metric_name": "driver_utilization", "value": 0.85}
        }),
    ];

    for event in &events {
        telemetry.process_event(event).unwrap();
    }

    let records = telemetry.flush();

    let log_count = records
        .iter()
        .filter(|r| matches!(r, TelemetryRecord::Log { .. }))
        .count();
    let trace_count = records
        .iter()
        .filter(|r| matches!(r, TelemetryRecord::Trace { .. }))
        .count();
    let metric_count = records
        .iter()
        .filter(|r| matches!(r, TelemetryRecord::Metric { .. }))
        .count();

    println!("Total records: {}", records.len());
    println!("  - Logs: {}", log_count);
    println!("  - Traces: {}", trace_count);
    println!("  - Metrics: {}", metric_count);

    assert!(log_count > 0, "Should generate log records");
    assert!(trace_count > 0, "Should generate trace records");
    assert!(metric_count > 0, "Should generate metric records");

    // Verify OTLP export works
    let otlp_logs = OtlpExporter::export_logs(&records).unwrap();
    let otlp_traces = OtlpExporter::export_traces(&records).unwrap();
    let otlp_metrics = OtlpExporter::export_metrics(&records).unwrap();

    assert!(otlp_logs.get("resourceLogs").is_some());
    assert!(otlp_traces.get("resourceSpans").is_some());
    assert!(otlp_metrics.get("resourceMetrics").is_some());

    println!("✓ Telemetry completeness verified\n");
}

/// Test network routing efficiency
#[test]
fn test_network_routing_efficiency() {
    println!("\n=== Testing Network Routing Efficiency ===\n");

    let network = load_san_francisco_network();
    let kitchen = Point::new(KITCHEN_LOCATION.0, KITCHEN_LOCATION.1);
    let destination = Point::new(DELIVERY_LOCATIONS[0].0, DELIVERY_LOCATIONS[0].1);

    // With network routing
    let handler_with_network = sim_geo::MovementHandler::new().with_network(network);
    let network_journey = handler_with_network
        .plan_journey(AgentId::new(), kitchen, destination, Utc::now())
        .unwrap();

    // Without network (straight line)
    let handler_no_network = sim_geo::MovementHandler::new();
    let straight_journey = handler_no_network
        .plan_journey(AgentId::new(), kitchen, destination, Utc::now())
        .unwrap();

    println!("Network route:");
    println!("  - Distance: {:.0} meters", network_journey.distance_m);
    println!("  - Waypoints: {}", network_journey.route.coords().count());

    println!("Straight line:");
    println!("  - Distance: {:.0} meters", straight_journey.distance_m);
    println!("  - Waypoints: {}", straight_journey.route.coords().count());

    // Network routes should follow roads (more waypoints)
    assert!(
        network_journey.route.coords().count() >= straight_journey.route.coords().count(),
        "Network route should have at least as many waypoints as straight line"
    );

    // Straight line should be exactly 2 points
    assert_eq!(
        straight_journey.route.coords().count(),
        2,
        "Straight line should have exactly 2 points"
    );

    println!("✓ Network routing efficiency verified\n");
}

// ===== HELPER FUNCTIONS =====

fn load_san_francisco_network() -> sim_geo::RoadNetwork {
    let test_data_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sim-geo")
        .join("test_data")
        .join("small_city_graph.json");

    sim_geo::RoadNetwork::load_from_file(test_data_path.to_str().unwrap())
        .expect("Failed to load test network")
}

fn setup_telemetry_collector() -> TelemetryCollector {
    let mut collector = TelemetryCollector::new();
    collector.add_generator(Box::new(EventToLogGenerator::new()));
    collector.add_generator(Box::new(EventToTraceGenerator::new()));
    collector.add_generator(Box::new(EventToMetricGenerator::new()));
    collector
}

fn create_drivers(count: usize, location: Point<f64>) -> Vec<DeliveryDriver> {
    (0..count)
        .map(|_| DeliveryDriver::new(AgentId::new(), location))
        .collect()
}

fn create_orders(count: usize) -> Vec<DeliveryOrder> {
    let start_time = Utc.with_ymd_and_hms(2024, 1, 15, 8, 0, 0).unwrap();
    (0..count)
        .map(|i| {
            let destination_idx = i % DELIVERY_LOCATIONS.len();
            let (lon, lat) = DELIVERY_LOCATIONS[destination_idx];

            DeliveryOrder {
                order_id: format!("ORDER-{:03}", i + 1),
                destination: Point::new(lon, lat),
                order_time: start_time + Duration::minutes(i as i64 / 2),
                assigned_driver_id: None,
                journey_id: None,
                completion_time: None,
            }
        })
        .collect()
}

fn has_active_deliveries(drivers: &[DeliveryDriver]) -> bool {
    drivers.iter().any(|d| !d.is_idle())
}

fn assign_orders(
    drivers: &mut [DeliveryDriver],
    orders: &mut [DeliveryOrder],
    kitchen: &Point<f64>,
    movement_handler: &mut sim_geo::MovementHandler,
    current_time: chrono::DateTime<chrono::Utc>,
    telemetry: &mut TelemetryCollector,
    stats: &mut SimulationStats,
) {
    let mut assigned = Vec::new();

    for driver in drivers.iter_mut() {
        if !driver.is_idle() {
            continue;
        }

        // Find next available order
        if let Some(order) = orders.iter_mut().find(|o| o.assigned_driver_id.is_none()) {
            // Plan journey from kitchen to destination
            match movement_handler.plan_journey(
                driver.driver_id,
                *kitchen,
                order.destination,
                current_time,
            ) {
                Ok(journey) => {
                    let journey_id = journey.id;
                    let order_id = order.order_id.clone();

                    // Update order
                    order.assigned_driver_id = Some(driver.driver_id);
                    order.journey_id = Some(journey_id);

                    // Update driver
                    driver.status = DriverStatus::EnRouteToDelivery {
                        order_id: order_id.clone(),
                        journey_id,
                    };

                    // Add journey
                    movement_handler.add_journey(journey);

                    // Record telemetry
                    emit_event(
                        telemetry,
                        json!({
                            "id": format!("order-assigned-{}", order_id),
                            "type": "com.simulation.order_assigned",
                            "source": format!("urn:sim:driver:{}", driver.driver_id),
                            "time": current_time.to_rfc3339(),
                            "data": {
                                "order_id": order_id,
                                "driver_id": driver.driver_id.to_string(),
                                "journey_id": journey_id.to_string(),
                                "time_period": get_time_period(current_time)
                            }
                        }),
                    );

                    stats.orders_assigned += 1;
                    assigned.push(order_id);
                }
                Err(e) => {
                    eprintln!("Failed to plan journey for order {}: {}", order.order_id, e);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_completed_journey(
    journey: &sim_geo::JourneyState,
    drivers: &mut [DeliveryDriver],
    completed_orders: &mut Vec<String>,
    kitchen: &Point<f64>,
    movement_handler: &mut sim_geo::MovementHandler,
    current_time: chrono::DateTime<chrono::Utc>,
    telemetry: &mut TelemetryCollector,
    stats: &mut SimulationStats,
) {
    // Find the driver for this journey
    if let Some(driver) = drivers.iter_mut().find(|d| match &d.status {
        DriverStatus::EnRouteToDelivery { journey_id, .. } => *journey_id == journey.id,
        DriverStatus::Returning { journey_id } => *journey_id == journey.id,
        _ => false,
    }) {
        match &driver.status {
            DriverStatus::EnRouteToDelivery { order_id, .. } => {
                // Delivery completed
                let order_id = order_id.clone();
                completed_orders.push(order_id.clone());

                // Calculate delivery time
                let delivery_duration = (current_time - journey.start_time).num_seconds();
                stats.completed_deliveries += 1;
                stats.total_distance_m += journey.distance_m;
                stats.total_delivery_time_secs += delivery_duration;

                // Track by time period
                match get_time_period(current_time) {
                    "rush_hour" => stats.rush_hour_deliveries += 1,
                    "night" => stats.night_deliveries += 1,
                    _ => stats.off_peak_deliveries += 1,
                }

                // Update driver location
                driver.current_location = journey
                    .route
                    .coords()
                    .last()
                    .map(|c| Point::new(c.x, c.y))
                    .unwrap_or(*kitchen);

                // Record telemetry
                emit_event(
                    telemetry,
                    json!({
                        "id": format!("delivery-completed-{}", order_id),
                        "type": "com.simulation.delivery_completed",
                        "source": format!("urn:sim:driver:{}", driver.driver_id),
                        "time": current_time.to_rfc3339(),
                        "data": {
                            "order_id": order_id,
                            "driver_id": driver.driver_id.to_string(),
                            "duration_secs": delivery_duration,
                            "distance_m": journey.distance_m,
                            "time_period": get_time_period(current_time)
                        }
                    }),
                );

                // Start return journey to kitchen
                match movement_handler.plan_journey(
                    driver.driver_id,
                    driver.current_location,
                    *kitchen,
                    current_time,
                ) {
                    Ok(return_journey) => {
                        let return_journey_id = return_journey.id;
                        driver.status = DriverStatus::Returning {
                            journey_id: return_journey_id,
                        };
                        movement_handler.add_journey(return_journey);
                    }
                    Err(e) => {
                        eprintln!("Failed to plan return journey: {}", e);
                        driver.status = DriverStatus::Idle;
                    }
                }
            }
            DriverStatus::Returning { .. } => {
                // Driver returned to kitchen
                driver.current_location = *kitchen;
                driver.status = DriverStatus::Idle;

                // Record telemetry
                emit_event(
                    telemetry,
                    json!({
                        "id": format!("driver-returned-{}", driver.driver_id),
                        "type": "com.simulation.driver_returned",
                        "source": format!("urn:sim:driver:{}", driver.driver_id),
                        "time": current_time.to_rfc3339(),
                        "data": {
                            "driver_id": driver.driver_id.to_string(),
                            "journey_distance_m": journey.distance_m
                        }
                    }),
                );
            }
            _ => {}
        }
    }
}

fn emit_event(telemetry: &mut TelemetryCollector, event: serde_json::Value) {
    if let Err(e) = telemetry.process_event(&event) {
        eprintln!("Failed to process telemetry event: {}", e);
    }
}

fn export_telemetry(records: &[TelemetryRecord]) -> HashMap<String, serde_json::Value> {
    let mut results = HashMap::new();

    match OtlpExporter::export_logs(records) {
        Ok(logs) => {
            let json_str = serde_json::to_string_pretty(&logs).unwrap();
            let size = json_str.len();
            results.insert(format!("logs ({} bytes)", size), logs);
        }
        Err(e) => eprintln!("Failed to export logs: {}", e),
    }

    match OtlpExporter::export_traces(records) {
        Ok(traces) => {
            let json_str = serde_json::to_string_pretty(&traces).unwrap();
            let size = json_str.len();
            results.insert(format!("traces ({} bytes)", size), traces);
        }
        Err(e) => eprintln!("Failed to export traces: {}", e),
    }

    match OtlpExporter::export_metrics(records) {
        Ok(metrics) => {
            let json_str = serde_json::to_string_pretty(&metrics).unwrap();
            let size = json_str.len();
            results.insert(format!("metrics ({} bytes)", size), metrics);
        }
        Err(e) => eprintln!("Failed to export metrics: {}", e),
    }

    // Write to temp files for inspection
    for (name, data) in &results {
        let filename = format!(
            "/tmp/ghost_kitchen_otlp_{}.json",
            name.split_whitespace().next().unwrap()
        );
        if let Err(e) = fs::write(&filename, serde_json::to_string_pretty(data).unwrap()) {
            eprintln!("Failed to write {}: {}", filename, e);
        }
    }

    results
}

fn print_simulation_summary(
    stats: &SimulationStats,
    start_time: chrono::DateTime<chrono::Utc>,
    end_time: chrono::DateTime<chrono::Utc>,
) {
    let duration_secs = (end_time - start_time).num_seconds();

    println!("   ========================================");
    println!("   Total Orders: {}", stats.total_orders);
    println!("   Completed Deliveries: {}", stats.completed_deliveries);
    println!("   Orders Assigned: {}", stats.orders_assigned);
    println!("   ----------------------------------------");

    if stats.completed_deliveries > 0 {
        let avg_delivery_time = stats.total_delivery_time_secs / stats.completed_deliveries as i64;
        let avg_distance = stats.total_distance_m / stats.completed_deliveries as f64;

        println!(
            "   Average Delivery Time: {} seconds ({:.1} minutes)",
            avg_delivery_time,
            avg_delivery_time as f64 / 60.0
        );
        println!(
            "   Average Distance: {:.0} meters ({:.1} km)",
            avg_distance,
            avg_distance / 1000.0
        );
        println!("   ----------------------------------------");
        println!("   Deliveries by Time Period:");
        println!(
            "     - Rush Hour: {} ({:.1}%)",
            stats.rush_hour_deliveries,
            stats.rush_hour_deliveries as f64 / stats.completed_deliveries as f64 * 100.0
        );
        println!(
            "     - Off-Peak: {} ({:.1}%)",
            stats.off_peak_deliveries,
            stats.off_peak_deliveries as f64 / stats.completed_deliveries as f64 * 100.0
        );
        println!(
            "     - Night: {} ({:.1}%)",
            stats.night_deliveries,
            stats.night_deliveries as f64 / stats.completed_deliveries as f64 * 100.0
        );
    }

    println!("   ----------------------------------------");
    println!(
        "   Simulation Duration: {} seconds ({:.1} minutes)",
        duration_secs,
        duration_secs as f64 / 60.0
    );
    println!("   ========================================");
}

fn print_sample_telemetry(records: &[TelemetryRecord]) {
    // Print first log
    if let Some(log) = records
        .iter()
        .find(|r| matches!(r, TelemetryRecord::Log { .. }))
        && let TelemetryRecord::Log {
            timestamp,
            level,
            message,
            attributes,
        } = log
    {
        println!("\n   Sample Log Record:");
        println!("     Time: {}", timestamp.to_rfc3339());
        println!("     Level: {:?}", level);
        println!("     Message: {}", message);
        if !attributes.is_empty() {
            println!("     Attributes: {:?}", attributes);
        }
    }

    // Print first trace
    if let Some(trace) = records
        .iter()
        .find(|r| matches!(r, TelemetryRecord::Trace { .. }))
        && let TelemetryRecord::Trace {
            trace_id,
            span_id,
            name,
            start_time,
            end_time,
            ..
        } = trace
    {
        println!("\n   Sample Trace Record:");
        println!("     Trace ID: {}", trace_id);
        println!("     Span ID: {}", span_id);
        println!("     Name: {}", name);
        println!(
            "     Duration: {} ms",
            (end_time.timestamp_millis() - start_time.timestamp_millis())
        );
    }

    // Print first metric
    if let Some(metric) = records
        .iter()
        .find(|r| matches!(r, TelemetryRecord::Metric { .. }))
        && let TelemetryRecord::Metric {
            name,
            value,
            metric_type,
            tags,
            ..
        } = metric
    {
        println!("\n   Sample Metric Record:");
        println!("     Name: {}", name);
        println!("     Value: {}", value);
        println!("     Type: {:?}", metric_type);
        if !tags.is_empty() {
            println!("     Tags: {:?}", tags);
        }
    }
}
