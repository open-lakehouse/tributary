use pyo3::prelude::*;
use sim_kitchen::KitchenSimulation;
use sim_logistics::LogisticsSimulation;

use crate::error::Error;

mod error;

/// Start time of a seeded run, matching `caspers run --seed` without
/// `--start-time`: a fixed instant makes the event times reproducible.
const SEEDED_START_TIME: &str = "2026-01-05T11:00:00Z";

/// The fixed start time for a seeded run, or `None` (start now) without a seed
fn seeded_start(seed: Option<u64>) -> Option<chrono::DateTime<chrono::Utc>> {
    seed.map(|_| SEEDED_START_TIME.parse().expect("valid RFC 3339"))
}

/// Result returned from a kitchen simulation run.
#[pyclass]
pub struct KitchenResult {
    #[pyo3(get)]
    pub events_path: String,
    #[pyo3(get)]
    pub total_steps: u64,
    #[pyo3(get)]
    pub total_agent_ticks: u64,
    #[pyo3(get)]
    pub total_assignments: u64,
}

/// Result returned from a logistics simulation run.
#[pyclass]
pub struct LogisticsResult {
    #[pyo3(get)]
    pub events_path: String,
    #[pyo3(get)]
    pub packages_created: u64,
    #[pyo3(get)]
    pub packages_delivered: u64,
    #[pyo3(get)]
    pub driver_utilization_percent: f64,
    #[pyo3(get)]
    pub average_delivery_time_secs: f64,
}

/// Run the ghost kitchen simulation and write events to Parquet.
///
/// Args:
///     steps: Number of simulation steps to run.
///     num_kitchens: Number of kitchens (default 3).
///     kitchen_capacity: Concurrent order capacity per kitchen (default 5).
///     num_customers: Number of customer agents (default 20).
///     output_dir: Directory to write events.parquet (and artifacts/) into (default ".caspers/sim-kitchen").
///     seed: Global seed (default None). A seeded run starts at 2026-01-05T11:00:00Z
///         and reproduces its events (all but the CloudEvents ids) and artifacts.
///
/// Returns:
///     KitchenResult with events_path and run statistics.
#[pyfunction]
#[pyo3(signature = (steps=100, num_kitchens=3, kitchen_capacity=5, num_customers=20, output_dir=".caspers/sim-kitchen", seed=None))]
fn run_kitchen_simulation(
    steps: usize,
    num_kitchens: usize,
    kitchen_capacity: usize,
    num_customers: usize,
    output_dir: &str,
    seed: Option<u64>,
) -> PyResult<KitchenResult> {
    let mut sim = KitchenSimulation::new_seeded(num_kitchens, kitchen_capacity, false, seed)
        .with_artifact_dir(std::path::Path::new(output_dir).join("artifacts"));
    if let Some(start) = seeded_start(seed) {
        sim.runner_mut().context_mut().set_current_time(start);
    }
    sim.add_customers(num_customers);
    sim.run_for_steps(steps)
        .map_err(|e| Error::Simulation(e.to_string()))?;

    let stats = sim.runner().stats().clone();

    let events_path = std::path::PathBuf::from(output_dir).join("events.parquet");
    sim.save_events(&events_path)
        .map_err(|e| Error::Simulation(e.to_string()))?;

    // Canonicalize after save_events has created the directory.
    let abs_path =
        std::fs::canonicalize(output_dir).unwrap_or_else(|_| std::path::PathBuf::from(output_dir));
    let events_path_full = abs_path.join("events.parquet");

    Ok(KitchenResult {
        events_path: events_path_full.to_string_lossy().into_owned(),
        total_steps: stats.total_steps as u64,
        total_agent_ticks: stats.total_agent_ticks as u64,
        total_assignments: stats.total_assignments as u64,
    })
}

/// Run the logistics (package delivery) simulation and write events to Parquet.
///
/// Args:
///     steps: Number of simulation steps to run (30 seconds each, 120 steps/hour).
///     output_dir: Directory to write events.parquet into (default ".caspers/sim-logistics").
///     seed: Global seed (default None). A seeded run starts at 2026-01-05T11:00:00Z
///         and reproduces its events (all but the CloudEvents ids).
///
/// Returns:
///     LogisticsResult with events_path and delivery metrics.
#[pyfunction]
#[pyo3(signature = (steps=120, output_dir=".caspers/sim-logistics", seed=None))]
fn run_logistics_simulation(
    steps: usize,
    output_dir: &str,
    seed: Option<u64>,
) -> PyResult<LogisticsResult> {
    let mut sim = LogisticsSimulation::new_seeded(seed);
    if let Some(start) = seeded_start(seed) {
        sim.runner_mut().context_mut().set_current_time(start);
    }
    for _ in 0..steps {
        sim.step().map_err(|e| Error::Simulation(e.to_string()))?;
    }

    let metrics = sim.get_metrics();

    let events_path = std::path::PathBuf::from(output_dir).join("events.parquet");
    sim.save_events(&events_path)
        .map_err(|e| Error::Simulation(e.to_string()))?;

    // Canonicalize after save_events has created the directory.
    let abs_path =
        std::fs::canonicalize(output_dir).unwrap_or_else(|_| std::path::PathBuf::from(output_dir));
    let events_path_full = abs_path.join("events.parquet");

    Ok(LogisticsResult {
        events_path: events_path_full.to_string_lossy().into_owned(),
        packages_created: metrics.packages_created,
        packages_delivered: metrics.packages_delivered,
        driver_utilization_percent: metrics.driver_utilization_percent,
        average_delivery_time_secs: metrics.average_delivery_time_secs,
    })
}

#[pymodule]
fn _internal(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<KitchenResult>()?;
    m.add_class::<LogisticsResult>()?;
    m.add_function(wrap_pyfunction!(run_kitchen_simulation, m)?)?;
    m.add_function(wrap_pyfunction!(run_logistics_simulation, m)?)?;
    Ok(())
}
