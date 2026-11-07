//! Basic ghost kitchen simulation example
//!
//! Demonstrates the complete simulation lifecycle:
//! - Creating customers and kitchens
//! - Running the simulation
//! - Emitting CloudEvents
//! - Writing events to Parquet and artifacts under `<output_dir>/artifacts`
//!
//! Usage:
//!   cargo run --example basic -p sim-kitchen [-- <steps> <output_dir>]

use sim_kitchen::KitchenSimulation;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let steps: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(100);
    let output_dir = args
        .get(2)
        .map(String::as_str)
        .unwrap_or(".caspers/sim-kitchen");

    println!("Ghost Kitchen Simulation Example\n");

    let artifacts_dir = std::path::PathBuf::from(output_dir).join("artifacts");
    let mut sim = KitchenSimulation::new(3, 5, true).with_artifact_dir(&artifacts_dir);
    sim.add_customers(20);

    println!("\nStarting simulation ({steps} steps)...\n");
    sim.run_for_steps(steps)?;

    let stats = sim.runner().stats().clone();
    println!("\nSimulation Statistics:");
    println!("  Total steps: {}", stats.total_steps);
    println!("  Total agent ticks: {}", stats.total_agent_ticks);
    println!("  Total assignments: {}", stats.total_assignments);

    let events_path = std::path::PathBuf::from(output_dir).join("events.parquet");
    let count = sim.save_events(&events_path)?;
    println!("\nWrote {count} events to {}", events_path.display());
    println!("Artifacts in {}", artifacts_dir.display());

    Ok(())
}
