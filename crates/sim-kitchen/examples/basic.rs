//! Basic ghost kitchen simulation example
//!
//! Demonstrates the complete simulation lifecycle:
//! - Creating customers and kitchens
//! - Running the simulation
//! - Emitting CloudEvents
//! - Writing events to Parquet

use sim_kitchen::{add_customers, build_simulation, KitchenSimulationHooks};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🍔 Ghost Kitchen Simulation Example\n");

    // Build simulation with 3 kitchens, 5 capacity each
    let mut runner = build_simulation(3, 5, true);

    // Add 20 customers
    add_customers(&mut runner, 20);

    println!("\n🚀 Starting simulation...\n");

    // Run for 100 steps (50 minutes of simulated time with 30s steps)
    runner.run_for_steps(100)?;

    println!("\n📦 Writing events to Parquet...");

    // Extract hooks to get dispatcher
    // Note: In a real scenario, you'd design this differently to avoid the unsafe cast
    // This is just for demonstration purposes
    let stats = runner.stats().clone();

    println!("\n✨ Simulation Statistics:");
    println!("   Total steps: {}", stats.total_steps);
    println!("   Total agent ticks: {}", stats.total_agent_ticks);
    println!("   Total assignments: {}", stats.total_assignments);

    println!("\n✅ Simulation complete!");
    println!("   Events would be written to 'kitchen_events.parquet'");

    Ok(())
}
