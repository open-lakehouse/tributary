use arrow::array::AsArray;
use caspers_universe::Error as UniverseError;
use caspers_universe::{SimulationContext, SimulationMessage, SimulationRunnerBuilder, resolve_url};
use crossterm::event::{Event, KeyCode, KeyEventKind};
use dialoguer::Select;
use tokio::sync::mpsc;

use crate::app::App;
use crate::error::Result;
use crate::tui::Tui;

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

    let duration = args.duration;

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<SimulationMessage>();

    let sim_handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to create simulation runtime");
        rt.block_on(async move {
            if let Err(e) = runner.run_with_channel(duration, event_tx.clone()).await {
                let _ = event_tx.send(SimulationMessage::Error(e.to_string()));
            }
        });
    });

    let mut tui = Tui::new()?;
    tui.enter()?;

    let mut app = App::new(duration);
    let tick_rate = std::time::Duration::from_millis(50);

    loop {
        tui.draw(|f| app.render(f))?;

        let timeout = tokio::time::sleep(tick_rate);
        tokio::pin!(timeout);

        tokio::select! {
            event = tui.next_event() => {
                if let Some(Event::Key(key)) = event {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                app.should_quit = true;
                            }
                            KeyCode::Char('c')
                                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                            {
                                app.should_quit = true;
                            }
                            _ => {}
                        }
                    }
                }
            }
            msg = event_rx.recv() => {
                process_message(&mut app, msg);
            }
            _ = &mut timeout => {}
        }

        while let Ok(msg) = event_rx.try_recv() {
            process_message(&mut app, Some(msg));
        }

        if app.should_quit {
            break;
        }
    }

    tui.exit()?;

    if app.completed {
        if sim_handle.join().is_err() {
            tracing::error!("simulation thread panicked");
        }
    }

    Ok(())
}

fn process_message(app: &mut App, msg: Option<SimulationMessage>) {
    match msg {
        Some(SimulationMessage::StepCompleted {
            step,
            total_steps: _,
            sim_time,
            event_counts,
        }) => {
            app.push_step(step, sim_time, event_counts);
        }
        Some(SimulationMessage::Completed { .. }) => {
            app.mark_completed();
        }
        Some(SimulationMessage::Error(err)) => {
            tracing::error!("simulation error: {err}");
            app.mark_completed();
        }
        None => {
            app.mark_completed();
        }
    }
}
