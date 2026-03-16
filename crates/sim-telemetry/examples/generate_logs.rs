//! Example demonstrating log generation from CloudEvents.
//!
//! This example loads sample events from test_data and generates
//! telemetry records in JSON format.

use sim_telemetry::core::TelemetryGenerator;
use sim_telemetry::generators::EventToLogGenerator;
use serde_json::Value;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Telemetry Log Generation Example ===\n");

    // Load sample events (handle both cargo run and cargo run from workspace root)
    let events_file = if std::path::Path::new("test_data/sample_events.json").exists() {
        "test_data/sample_events.json"
    } else {
        "crates/sim-telemetry/test_data/sample_events.json"
    };
    let events_data = fs::read_to_string(events_file)?;
    let events_json: Value = serde_json::from_str(&events_data)?;

    // Create log generator with custom probabilities
    let mut generator = EventToLogGenerator::with_probabilities(0.08, 0.12);

    println!("Processing {} sample events...\n",
        events_json["events"].as_array().map(|a| a.len()).unwrap_or(0));

    // Process first 5 events as examples
    if let Some(events) = events_json["events"].as_array() {
        for (idx, event) in events.iter().take(5).enumerate() {
            println!("--- Event {} ---", idx + 1);

            // Show original event
            println!("Original Event:");
            if let Some(event_type) = event.get("type").and_then(|v| v.as_str()) {
                println!("  Type: {}", event_type);
            }
            if let Some(source) = event.get("source").and_then(|v| v.as_str()) {
                println!("  Source: {}", source);
            }
            if let Some(id) = event.get("id").and_then(|v| v.as_str()) {
                println!("  ID: {}", id);
            }

            // Generate telemetry record
            match generator.generate(event) {
                Ok(records) => {
                    for record in records {
                        println!("\nGenerated Log Record (JSON):");
                        let json_output = serde_json::to_string_pretty(&record)?;
                        println!("{}", json_output);
                    }
                }
                Err(e) => {
                    eprintln!("Error generating telemetry: {}", e);
                }
            }
            println!();
        }
    }

    // Statistics
    println!("\n=== Statistics ===");

    if let Some(events) = events_json["events"].as_array() {
        let mut log_levels = std::collections::HashMap::new();

        for event in events.iter().take(20) {
            if let Ok(records) = generator.generate(event) {
                for record in records {
                    if let sim_telemetry::TelemetryRecord::Log { level, .. } = record {
                        *log_levels.entry(format!("{:?}", level)).or_insert(0) += 1;
                    }
                }
            }
        }

        println!("Log level distribution (20 events):");
        for (level, count) in log_levels {
            println!("  {}: {}", level, count);
        }
    }

    println!("\nDone!");
    Ok(())
}
