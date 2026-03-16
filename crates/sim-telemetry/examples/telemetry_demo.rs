//! Comprehensive telemetry demonstration.
//!
//! This example shows end-to-end telemetry generation from a complete simulation:
//! 1. Create simulation with agents and workflows
//! 2. Attach telemetry collection
//! 3. Run simulation and collect telemetry
//! 4. Export to multiple formats (OTLP, JSON, Prometheus)
//! 5. Analyze collected telemetry

use serde_json::json;
use sim_telemetry::{
    EventToLogGenerator, EventToMetricGenerator, EventToTraceGenerator, TelemetryCollector,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Comprehensive Telemetry Demo ===\n");

    // Create telemetry collector with all generators
    let mut collector = TelemetryCollector::new();
    collector.add_generator(Box::new(EventToLogGenerator::new()));
    collector.add_generator(Box::new(EventToMetricGenerator::new()));
    collector.add_generator(Box::new(EventToTraceGenerator::new()));

    println!(
        "Created TelemetryCollector with {} generators",
        collector.generator_count()
    );
    println!();

    // Simulate a complete workflow
    simulate_workflow(&mut collector)?;

    // Flush and analyze telemetry
    let records = collector.flush();
    println!("\n=== Telemetry Summary ===");
    println!("Total records collected: {}", records.len());

    // Analyze by type
    let mut log_count = 0;
    let mut metric_count = 0;
    let mut trace_count = 0;

    for record in &records {
        match record {
            sim_telemetry::TelemetryRecord::Log { .. } => log_count += 1,
            sim_telemetry::TelemetryRecord::Metric { .. } => metric_count += 1,
            sim_telemetry::TelemetryRecord::Trace { .. } => trace_count += 1,
        }
    }

    println!("  Logs:    {}", log_count);
    println!("  Metrics: {}", metric_count);
    println!("  Traces:  {}", trace_count);

    // Show sample records
    println!("\n=== Sample Records ===");
    for (idx, record) in records.iter().take(3).enumerate() {
        println!("\n--- Record {} ---", idx + 1);
        let json_output = serde_json::to_string_pretty(&record)?;
        println!("{}", json_output);
    }

    // Export to different formats
    println!("\n=== Export Formats ===");
    export_examples(&records)?;

    println!("\n=== Demo Complete ===");
    Ok(())
}

fn simulate_workflow(collector: &mut TelemetryCollector) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Simulating Workflow ===\n");

    // Step 1: Simulation start
    let start_event = json!({
        "specversion": "1.0",
        "id": "sim-evt-00000001",
        "source": "urn:sim:runner",
        "type": "com.simulation.started",
        "time": "2026-03-16T10:00:00Z",
        "datacontenttype": "application/json",
        "data": {
            "step": 0,
            "config": {
                "time_step_seconds": 60
            }
        }
    });

    collector.process_event(&start_event)?;
    println!("1. Simulation started");

    // Step 2: Agent ticks (simulate 10 agents)
    for tick in 0..5 {
        let tick_event = json!({
            "specversion": "1.0",
            "id": format!("sim-evt-000000{:02}", tick + 2),
            "source": "urn:sim:runner:population",
            "type": "com.simulation.agents_ticked",
            "time": format!("2026-03-16T10:{:02}:00Z", tick),
            "datacontenttype": "application/json",
            "data": {
                "step": tick,
                "agent_count": 10,
                "simulation_time": format!("2026-03-16T10:{:02}:00Z", tick)
            }
        });

        collector.process_event(&tick_event)?;
    }
    println!("2. Processed 5 ticks (10 agents each)");

    // Step 3: Resource allocation
    let allocation_event = json!({
        "specversion": "1.0",
        "id": "sim-evt-00000010",
        "source": "urn:sim:runner:resources",
        "type": "com.simulation.resources_allocated",
        "time": "2026-03-16T10:02:00Z",
        "datacontenttype": "application/json",
        "data": {
            "step": 2,
            "assignment_count": 5,
            "simulation_time": "2026-03-16T10:02:00Z"
        }
    });

    collector.process_event(&allocation_event)?;
    println!("3. Allocated 5 resources");

    // Step 4: Workflow completion
    let workflow_event = json!({
        "specversion": "1.0",
        "id": "sim-evt-00000011",
        "source": "urn:sim:runner:workflows",
        "type": "com.simulation.workflow_completed",
        "time": "2026-03-16T10:05:00Z",
        "datacontenttype": "application/json",
        "data": {
            "instance_id": "wf-inst-001",
            "workflow_id": "wf-001",
            "completion_time": "2026-03-16T10:05:00Z"
        }
    });

    collector.process_event(&workflow_event)?;
    println!("4. Workflow completed");

    // Step 5: Simulation end
    let end_event = json!({
        "specversion": "1.0",
        "id": "sim-evt-00000012",
        "source": "urn:sim:runner",
        "type": "com.simulation.completed",
        "time": "2026-03-16T10:05:00Z",
        "datacontenttype": "application/json",
        "data": {
            "total_steps": 5,
            "final_time": "2026-03-16T10:05:00Z"
        }
    });

    collector.process_event(&end_event)?;
    println!("5. Simulation completed");

    Ok(())
}

fn export_examples(
    records: &[sim_telemetry::TelemetryRecord],
) -> Result<(), Box<dyn std::error::Error>> {
    // JSON export (simple serialization)
    println!("\n1. JSON Export:");
    let json_export = serde_json::to_string_pretty(&records)?;
    println!("   Generated {} bytes of JSON", json_export.len());

    // Prometheus export example (for metrics only)
    println!("\n2. Prometheus Export:");
    let metric_records: Vec<_> = records
        .iter()
        .filter_map(|r| match r {
            sim_telemetry::TelemetryRecord::Metric {
                name,
                value,
                timestamp,
                ..
            } => Some(format!(
                "# TYPE {} gauge\n{} {} {}",
                name,
                name,
                value,
                timestamp.timestamp()
            )),
            _ => None,
        })
        .collect();
    println!("   Generated {} metric entries", metric_records.len());
    if !metric_records.is_empty() {
        println!(
            "   Sample: {}",
            metric_records[0].lines().next().unwrap_or("")
        );
    }

    // OTLP export format info (would require actual OTLP library)
    println!("\n3. OTLP Export:");
    println!(
        "   Logs:    {} records ready for export",
        records
            .iter()
            .filter(|r| matches!(r, sim_telemetry::TelemetryRecord::Log { .. }))
            .count()
    );
    println!(
        "   Traces:  {} spans ready for export",
        records
            .iter()
            .filter(|r| matches!(r, sim_telemetry::TelemetryRecord::Trace { .. }))
            .count()
    );
    println!(
        "   Metrics: {} datapoints ready for export",
        records
            .iter()
            .filter(|r| matches!(r, sim_telemetry::TelemetryRecord::Metric { .. }))
            .count()
    );

    Ok(())
}
