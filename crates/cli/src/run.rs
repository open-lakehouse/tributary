use arrow::array::AsArray;
use caspers_universe::Error as UniverseError;
use caspers_universe::{SimulationContext, SimulationRunnerBuilder, resolve_url};
use dialoguer::Select;

use crate::error::Result;

#[derive(Debug, Clone, clap::Parser)]
pub(crate) struct RunArgs {
    #[arg(short, long, default_value_t = 100)]
    duration: usize,

    #[arg(short, long)]
    /// Path where basic simulation setup is stored.
    working_directory: Option<String>,

    #[arg(long, default_value_t = false)]
    dry_run: bool,
}

pub(super) async fn handle(args: RunArgs) -> Result<()> {
    let caspers_directory = resolve_url(args.working_directory)?;
    let mut builder =
        SimulationContext::builder().with_working_directory(caspers_directory.clone());

    let simulations = builder
        .load_simulations()
        .await?
        .select_columns(&["id"])
        .map_err(UniverseError::from)?
        .collect()
        .await
        .map_err(UniverseError::from)?;

    let selections = (0..simulations.len())
        .flat_map(|idx| simulations[idx].column(0).as_string_view().iter())
        .flatten()
        .collect::<Vec<_>>();

    let Some(sim_selection) = Select::new()
        .with_prompt("Which simulation to run?")
        .items(&selections)
        .interact_opt()?
    else {
        return Ok(());
    };

    let simulation_id =
        uuid::Uuid::try_parse(selections[sim_selection]).map_err(UniverseError::from)?;
    builder = builder.with_simulation_id(simulation_id);

    let snapshots = builder
        .load_snapshots()
        .await?
        .select_columns(&["id"])
        .map_err(UniverseError::from)?
        .collect()
        .await
        .map_err(UniverseError::from)?;

    let selections = (0..snapshots.len())
        .flat_map(|idx| snapshots[idx].column(0).as_string_view().iter())
        .flatten()
        .collect::<Vec<_>>();

    let Some(sn_selection) = Select::new()
        .with_prompt("Which snapshot to start from?")
        .items(&selections)
        .interact_opt()?
    else {
        return Ok(());
    };

    let snapshot_id =
        uuid::Uuid::try_parse(selections[sn_selection]).map_err(UniverseError::from)?;
    let ctx = builder.with_snapshot_id(snapshot_id).build().await?;

    let mut runner = SimulationRunnerBuilder::new(ctx)
        .with_dry_run(args.dry_run)
        .build()
        .await?;

    runner.run(args.duration).await?;

    Ok(())
}
