use std::collections::HashMap;

use chrono::{DateTime, Utc};
use crossterm::event::{Event, KeyCode, KeyEventKind};
use sim_core::{
    ControlCommand, SimEvent, SimulationManager, SimulationManagerBackend, TimeMode,
    control_channel,
};
use sim_kitchen::KitchenSimulation;
use uuid::Uuid;

use crate::app::{App, Dialog, Screen, SimListEntry, short_event_type};
use crate::error::Result;
use crate::time_parse::parse_sim_time;
use crate::tui::Tui;

/// Time mode for the simulation run
#[derive(Debug, Clone, Default, clap::ValueEnum)]
pub enum RunMode {
    /// Advance by time_step each tick; stop at --end-time if given (default)
    #[default]
    Range,
    /// Simulation time tracks wall clock; sleeps between ticks
    Live,
    /// Fast-forward from --start-time to now, then switch to Live
    Catchup,
}

#[derive(Debug, Clone, clap::Parser)]
pub(crate) struct RunArgs {
    #[arg(short, long, default_value_t = 100)]
    duration: usize,

    #[arg(long, default_value_t = 3)]
    num_kitchens: usize,

    #[arg(long, default_value_t = 5)]
    kitchen_capacity: usize,

    #[arg(long, default_value_t = 20)]
    num_customers: usize,

    /// Time mode: range (default), live, or catchup
    #[arg(long, default_value = "range")]
    mode: RunMode,

    /// Simulation start time.
    /// Accepted: now  now+3h  now-30m  2026-01-01  2026-01-01T08:00:00Z
    #[arg(long)]
    start_time: Option<String>,

    /// Simulation end time (range mode only).
    /// Accepted: now  now+3h  now-30m  2026-01-01  2026-01-01T08:00:00Z
    #[arg(long)]
    end_time: Option<String>,

    /// Skip TUI; print a JSON summary to stdout when done (useful for scripting/CI).
    #[arg(long)]
    no_tui: bool,

    /// Global RNG seed for reproducible runs. When set, RNG-driven decisions
    /// (demand, customer ordering) are identical run-to-run for the same params.
    #[arg(long)]
    seed: Option<u64>,

    /// Write the run's events to this Parquet file when the simulation ends.
    /// Required input for `caspers validate`.
    #[arg(long, value_name = "FILE")]
    events_out: Option<std::path::PathBuf>,

    /// Directory for generated artifacts (JSON Lines), written when the run ends.
    #[arg(long, value_name = "DIR", default_value = "output/artifacts")]
    artifacts_out: std::path::PathBuf,
}

impl RunArgs {
    /// True when the run should skip the TUI (headless / scripting mode).
    pub(crate) fn is_headless(&self) -> bool {
        self.no_tui
    }
}

/// Optional `(start, end)` simulation times
type TimeRange = (Option<DateTime<Utc>>, Option<DateTime<Utc>>);

/// Parse `--start-time` / `--end-time`
fn parse_times(args: &RunArgs) -> Result<TimeRange> {
    let parse = |flag: &str, value: &Option<String>| {
        value
            .as_deref()
            .map(|s| {
                parse_sim_time(s)
                    .map_err(|e| crate::error::Error::Other(format!("invalid --{flag}: {e}")))
            })
            .transpose()
    };
    Ok((
        parse("start-time", &args.start_time)?,
        parse("end-time", &args.end_time)?,
    ))
}

/// The simulation catalog under `.caspers`, or an in-memory one if that fails
async fn open_manager() -> Result<SimulationManager> {
    match SimulationManager::new_with_persistence(".caspers").await {
        Ok(manager) => Ok(manager),
        Err(e) => {
            tracing::warn!("cannot open .caspers catalog ({e}); using an in-memory catalog");
            SimulationManager::new(SimulationManagerBackend::Memory)
                .await
                .map_err(|e| crate::error::Error::Other(format!("in-memory catalog: {e}")))
        }
    }
}

/// Build the kitchen simulation the flags describe
fn build_sim(
    args: &RunArgs,
    sim_id: Uuid,
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
) -> KitchenSimulation {
    let mut sim =
        KitchenSimulation::new_seeded(args.num_kitchens, args.kitchen_capacity, false, args.seed)
            .with_artifact_dir(&args.artifacts_out);
    sim.add_customers(args.num_customers);
    sim.runner_mut().context_mut().set_simulation_id(sim_id);
    if let Some(st) = start_time {
        sim.runner_mut().context_mut().set_current_time(st);
    }
    sim.runner_mut().set_time_mode(match args.mode {
        RunMode::Range => TimeMode::Range { end_time },
        RunMode::Live => TimeMode::Live,
        RunMode::Catchup => TimeMode::Catchup,
    });
    sim
}

/// Step limit for `run_controlled_for`: `--duration` in range mode, unbounded otherwise
fn max_steps(args: &RunArgs) -> usize {
    match args.mode {
        RunMode::Range => args.duration,
        _ => usize::MAX,
    }
}

/// Write events to `--events-out`, if given
fn save_events(sim: &mut KitchenSimulation, path: Option<&std::path::Path>) -> Result<()> {
    if let Some(path) = path {
        let n = sim.save_events(path).map_err(|e| {
            crate::error::Error::Other(format!("writing events to {}: {e}", path.display()))
        })?;
        eprintln!("Wrote {n} events to {}", path.display());
    }
    Ok(())
}

pub(super) async fn handle(args: RunArgs) -> Result<()> {
    if args.no_tui {
        return handle_headless(args).await;
    }
    handle_tui(args).await
}

async fn handle_headless(args: RunArgs) -> Result<()> {
    let (start_time, end_time) = parse_times(&args)?;
    let mut sim_manager = open_manager().await?;
    let sim_id = sim_manager
        .create_simulation(None, None)
        .await
        .map_err(|e| crate::error::Error::Other(format!("registering simulation: {e}")))?;

    let (controller, receiver) = control_channel(128);
    let mut sim = build_sim(&args, sim_id, start_time, end_time);
    let max = max_steps(&args);
    let events_out = args.events_out.clone();
    let sim_handle = std::thread::spawn(move || -> Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let _guard = rt.enter();
        sim.runner_mut().attach_control(receiver);
        sim.run_controlled_for(max)
            .map_err(|e| crate::error::Error::Other(format!("simulation failed: {e}")))?;
        save_events(&mut sim, events_out.as_deref())
    });

    let mut cumulative: HashMap<String, u64> = HashMap::new();
    let mut steps = 0usize;
    let mut sim_time: Option<DateTime<Utc>> = None;
    let mut sim_error: Option<String> = None;

    loop {
        match controller.try_recv() {
            Some(SimEvent::TickCompleted {
                step,
                sim_time: t,
                event_counts,
            }) => {
                steps = step;
                sim_time = Some(t);
                for (k, v) in event_counts {
                    *cumulative.entry(k).or_default() += v;
                }
                if step % 10 == 0 {
                    eprint!(".");
                }
            }
            // KitchenSimulation pauses at the step limit so the TUI stays
            // alive; headless has nobody to resume it, so stop the run.
            Some(SimEvent::Paused { .. }) => {
                let _ = controller.send(ControlCommand::Stop);
            }
            Some(SimEvent::SimulationEnded { .. }) => break,
            Some(SimEvent::Error(e)) => {
                sim_error = Some(e);
                break;
            }
            Some(_) => {}
            None => {
                if sim_handle.is_finished() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }
    eprintln!();

    let outcome = sim_handle
        .join()
        .map_err(|_| crate::error::Error::Other("simulation thread panicked".into()))?;

    let output = serde_json::json!({
        "steps": steps,
        "sim_time": sim_time.map(|t| t.to_rfc3339()),
        "events_by_type": cumulative,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&output).expect("summary serializes")
    );

    outcome?;
    match sim_error {
        Some(e) => Err(crate::error::Error::Other(format!(
            "simulation failed: {e}"
        ))),
        None => Ok(()),
    }
}

async fn handle_tui(args: RunArgs) -> Result<()> {
    let duration = args.duration;
    let (start_time, end_time) = parse_times(&args)?;

    let initial_mode_str = match args.mode {
        RunMode::Range => "Range",
        RunMode::Live => "Live",
        RunMode::Catchup => "Catchup",
    }
    .to_string();

    // ── Phase 1: Launch screen ────────────────────────────────────────────────
    let mut sim_manager = open_manager().await?;

    let existing_sims = sim_manager.list_simulations().await.unwrap_or_default();

    // Build sim list with snapshot counts
    let mut sim_list: Vec<SimListEntry> = Vec::new();
    for rec in &existing_sims {
        let snap_count = sim_manager
            .list_snapshots(rec.id)
            .await
            .map(|s| s.len())
            .unwrap_or(0);
        sim_list.push(SimListEntry {
            id: rec.id,
            label: rec.label.clone(),
            created_at: rec.created_at,
            snapshot_count: snap_count,
        });
    }

    // Enter the TUI for the Launch screen
    let mut tui = Tui::new()?;
    tui.enter()?;

    let mut app = crate::app::App::new_launch(sim_list);
    let tick_rate = std::time::Duration::from_millis(50);

    // Oneshot channel fired when the user confirms a simulation choice
    let (sim_start_tx, mut sim_start_rx) =
        tokio::sync::oneshot::channel::<(Uuid, Option<String>)>();
    let mut sim_start_tx = Some(sim_start_tx);

    // Run the Launch screen loop until user selects/creates a sim
    'launch: loop {
        tui.draw(|f| app.render(f))?;

        let timeout = tokio::time::sleep(tick_rate);
        tokio::pin!(timeout);

        tokio::select! {
            event = tui.next_event() => {
                if let Some(Event::Key(key)) = event
                    && key.kind == KeyEventKind::Press {
                        if !matches!(app.dialog, Dialog::None) {
                            // NewSimName dialog — enter key creates sim and fires channel
                            if matches!(app.dialog, Dialog::NewSimName { .. }) {
                                match key.code {
                                    KeyCode::Char(c) => {
                                        if let Dialog::NewSimName { input } = &mut app.dialog {
                                            input.push(c);
                                        }
                                    }
                                    KeyCode::Backspace => {
                                        if let Dialog::NewSimName { input } = &mut app.dialog {
                                            input.pop();
                                        }
                                    }
                                    KeyCode::Enter => {
                                        let label = if let Dialog::NewSimName { input } = &app.dialog {
                                            let s = input.trim().to_string();
                                            if s.is_empty() { None } else { Some(s) }
                                        } else {
                                            None
                                        };
                                        let id = sim_manager
                                            .create_simulation(label.as_deref(), None)
                                            .await
                                            .unwrap_or_else(|_| Uuid::now_v7());
                                        app.active_simulation_id = Some(id);
                                        app.dialog = Dialog::None;
                                        if let Some(tx) = sim_start_tx.take() {
                                            let _ = tx.send((id, label));
                                        }
                                        break 'launch;
                                    }
                                    KeyCode::Esc => {
                                        app.dialog = Dialog::None;
                                    }
                                    _ => {}
                                }
                            } else {
                                // dummy controller — dialogs here don't need it
                                let (dummy_ctrl, _dummy_rx) = control_channel(1);
                                let _ = app.handle_dialog_key(key.code, &dummy_ctrl);
                            }
                        } else {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    app.should_quit = true;
                                    break 'launch;
                                }
                                KeyCode::Char('n') => {
                                    app.dialog = Dialog::NewSimName { input: String::new() };
                                }
                                KeyCode::Up => {
                                    app.sim_list_cursor = app.sim_list_cursor.saturating_sub(1);
                                }
                                KeyCode::Down
                                    if !app.sim_list.is_empty() => {
                                        app.sim_list_cursor = (app.sim_list_cursor + 1)
                                            .min(app.sim_list.len() - 1);
                                    }
                                KeyCode::Enter if !app.sim_list.is_empty() => {
                                    let entry = &app.sim_list[app.sim_list_cursor];
                                    let id = entry.id;
                                    let label = entry.label.clone();
                                    app.active_simulation_id = Some(id);
                                    if let Some(tx) = sim_start_tx.take() {
                                        let _ = tx.send((id, label));
                                    }
                                    break 'launch;
                                }
                                _ => {}
                            }
                        }
                }
            }
            _ = &mut timeout => {}
        }
    }

    if app.should_quit {
        tui.exit()?;
        return Ok(());
    }

    // Get the confirmed simulation ID
    let sim_id = match sim_start_rx.try_recv() {
        Ok((id, _)) => id,
        Err(_) => {
            tui.exit()?;
            return Ok(());
        }
    };

    let mut sim = build_sim(&args, sim_id, start_time, end_time);

    // Transition app to Simulation screen
    app.screen = Screen::Simulation;
    app.total_steps = duration;
    app.time_mode = initial_mode_str.clone();
    app.time_step_secs = sim.runner().context().time_step().num_seconds();
    app.range_end_time = end_time;

    // ── Phase 2: Spawn simulation thread ─────────────────────────────────────
    // Controller lives on the TUI side; receiver goes to the sim thread.
    let (controller, receiver) = control_channel(128);

    let max = max_steps(&args);
    let events_out = args.events_out.clone();
    let sim_handle = std::thread::spawn(move || {
        // Snapshot I/O needs a runtime; the sim thread has no ambient one.
        let result =
            (|| -> Result<()> {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                let _guard = rt.enter();
                rt.block_on(sim.runner_mut().set_snapshot_backend(
                    sim_core::SnapshotBackend::Parquet(format!(".caspers/snapshots/{sim_id}")),
                ))
                .map_err(|e| crate::error::Error::Other(format!("snapshot storage: {e}")))?;
                sim.runner_mut().attach_control(receiver);
                sim.run_controlled_for(max)
                    .map_err(|e| crate::error::Error::Other(e.to_string()))?;
                save_events(&mut sim, events_out.as_deref())
            })();
        if let Err(e) = &result {
            sim.runner_mut().emit_event(SimEvent::Error(e.to_string()));
        }
        result
    });
    let tick_rate = std::time::Duration::from_millis(50);

    loop {
        tui.draw(|f| app.render(f))?;

        let timeout = tokio::time::sleep(tick_rate);
        tokio::pin!(timeout);

        tokio::select! {
            event = tui.next_event() => {
                if let Some(Event::Key(key)) = event
                    && key.kind == KeyEventKind::Press {
                        // If a dialog is open, route keys there first
                        if !matches!(app.dialog, Dialog::None) {
                            app.handle_dialog_key(key.code, &controller)?;
                        } else {
                            match &app.screen {
                                Screen::Simulation => {
                                    match key.code {
                                        KeyCode::Char('q') | KeyCode::Esc => {
                                            let _ = controller.send(ControlCommand::Stop);
                                            app.should_quit = true;
                                        }
                                        KeyCode::Char('c')
                                            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                                        {
                                            let _ = controller.send(ControlCommand::Stop);
                                            app.should_quit = true;
                                        }
                                        KeyCode::Char(' ') if !app.completed => {
                                            if app.paused {
                                                let _ = controller.send(ControlCommand::Resume);
                                                app.paused = false;
                                            } else {
                                                let _ = controller.send(ControlCommand::Pause);
                                            }
                                        }
                                        KeyCode::Char('s') if app.paused => {
                                            let _ = controller.send(ControlCommand::StepN(1));
                                        }
                                        KeyCode::Char('f') if app.paused => {
                                            let _ = controller.send(ControlCommand::StepN(10));
                                        }
                                        KeyCode::Char('m') if !app.completed => {
                                            app.dialog = Dialog::ModeSelect;
                                        }
                                        KeyCode::Char('a')
                                            if !app.completed && app.time_mode == "Range" =>
                                        {
                                            app.dialog = Dialog::AdvanceBy {
                                                input: String::new(),
                                                error: None,
                                            };
                                        }
                                        KeyCode::Char('S') if !app.completed => {
                                            app.screen = Screen::SnapshotBrowser;
                                        }
                                        // Speed controls (Live mode only, not completed)
                                        KeyCode::Char('+') | KeyCode::Char('=')
                                            if !app.completed && app.time_mode == "Live" =>
                                        {
                                            let new_speed = (app.live_speed * 2.0).min(16.0);
                                            let _ = controller.send(ControlCommand::SetSpeed(new_speed));
                                            app.live_speed = new_speed;
                                        }
                                        KeyCode::Char('-')
                                            if !app.completed && app.time_mode == "Live" =>
                                        {
                                            let new_speed = (app.live_speed / 2.0).max(0.125);
                                            let _ = controller.send(ControlCommand::SetSpeed(new_speed));
                                            app.live_speed = new_speed;
                                        }
                                        _ => {}
                                    }
                                }
                                Screen::SnapshotBrowser => {
                                    match key.code {
                                        KeyCode::Esc => {
                                            app.screen = Screen::Simulation;
                                        }
                                        KeyCode::Up => {
                                            app.snapshot_list_cursor =
                                                app.snapshot_list_cursor.saturating_sub(1);
                                        }
                                        KeyCode::Down
                                            if !app.snapshot_list.is_empty() => {
                                                app.snapshot_list_cursor = (app.snapshot_list_cursor + 1)
                                                    .min(app.snapshot_list.len() - 1);
                                            }
                                        KeyCode::Char('t') => {
                                            app.dialog = Dialog::TakeSnapshot {
                                                input: String::new(),
                                            };
                                        }
                                        KeyCode::Enter if !app.snapshot_list.is_empty() => {
                                            let snap = &app.snapshot_list[app.snapshot_list_cursor];
                                            app.dialog = Dialog::ConfirmRestore {
                                                snapshot_id: snap.id,
                                                label: snap.label.clone(),
                                            };
                                        }
                                        _ => {}
                                    }
                                }
                                Screen::Launch => {
                                    // Launch screen key handling (used before sim starts)
                                    if let KeyCode::Char('q') = key.code {
                                        app.should_quit = true;
                                    }
                                }
                            }
                        }
                    }
            }
            _ = &mut timeout => {
                while let Some(event) = controller.try_recv() {
                    process_sim_event(&mut app, event);
                }
            }
        }

        // Drain any remaining events after timeout
        while let Some(event) = controller.try_recv() {
            process_sim_event(&mut app, event);
        }

        if app.should_quit || app.completed {
            // Drain remaining events before deciding to exit
            while let Some(event) = controller.try_recv() {
                process_sim_event(&mut app, event);
            }
            if app.should_quit {
                break;
            }
            // Stay in loop after completion so user can press q
        }
    }

    tui.exit()?;

    match sim_handle.join() {
        Ok(_) => {
            if app.error.is_some() {
                eprintln!("\n❌ Simulation failed with error:");
                if let Some(err) = &app.error {
                    eprintln!("   {}", err);
                }
                eprintln!(
                    "\nSimulation stopped at step {}/{}",
                    app.current_step, app.total_steps
                );
                std::process::exit(1);
            } else if app.completed {
                // Print end-of-run summary before the status line
                print_run_summary(&app);

                if app.current_step < app.total_steps {
                    eprintln!("\n⚠️  Warning: Simulation completed early!");
                    eprintln!(
                        "   Expected {} steps, but only completed {}",
                        app.total_steps, app.current_step
                    );
                } else {
                    eprintln!("\n✅ Simulation completed successfully!");
                    eprintln!("   Completed {} steps", app.current_step);
                }
            } else if app.should_quit {
                eprintln!(
                    "\n⚠️  Simulation aborted by user at step {}/{}",
                    app.current_step, app.total_steps
                );
            }
        }
        Err(panic_info) => {
            eprintln!("\n💥 FATAL: Simulation thread panicked!");
            eprintln!(
                "   Panic occurred at step {}/{}",
                app.current_step, app.total_steps
            );

            if let Some(msg) = panic_info.downcast_ref::<&str>() {
                eprintln!("   Panic message: {}", msg);
            } else if let Some(msg) = panic_info.downcast_ref::<String>() {
                eprintln!("   Panic message: {}", msg);
            } else {
                eprintln!("   Panic message: <unknown>");
            }

            eprintln!("\nThis is a critical error. Please check the simulation logs.");
            std::process::exit(2);
        }
    }

    Ok(())
}

fn process_sim_event(app: &mut App, event: SimEvent) {
    match event {
        SimEvent::TickCompleted {
            step,
            sim_time,
            event_counts,
        } => {
            app.push_step(step, sim_time, event_counts);
        }
        SimEvent::Paused { .. } => {
            app.paused = true;
        }
        SimEvent::ModeChanged { mode } => {
            app.time_mode = mode;
        }
        SimEvent::SimulationEnded { .. } => {
            app.mark_completed();
        }
        SimEvent::Error(err) => {
            tracing::error!("simulation error: {err}");
            app.set_error(err);
            app.mark_completed();
        }
        SimEvent::SpeedChanged { multiplier } => {
            app.live_speed = multiplier;
        }
        SimEvent::SnapshotSaved {
            snapshot_id,
            label,
            sim_time,
            step,
        } => {
            tracing::info!("snapshot saved: {snapshot_id} ({label})");
            app.notification = Some((
                format!("Snapshot saved: {label}"),
                std::time::Instant::now(),
            ));
            // Add to snapshot list
            app.snapshot_list.push(crate::app::SnapshotSummary {
                id: snapshot_id,
                label: Some(label),
                sim_time,
                created_at: chrono::Utc::now(),
                step,
                agent_count: 0,
            });
        }
        SimEvent::SnapshotRestored {
            snapshot_id,
            label,
            sim_time,
            step,
        } => {
            tracing::info!("snapshot restored: {snapshot_id}");
            let label_str = label.as_deref().unwrap_or("snapshot");
            app.notification = Some((format!("Restored: {label_str}"), std::time::Instant::now()));
            app.sim_time = Some(sim_time);
            app.current_step = step;
        }
        SimEvent::SnapshotsUpdated { .. } => {
            // Snapshot list updates are handled via SnapshotSaved/SnapshotRestored events.
            // This event is a signal that a refresh occurred; no additional action needed here.
        }
    }
}

fn print_run_summary(app: &App) {
    eprintln!("\n=== Simulation Summary ===");
    eprintln!("Steps completed : {}", app.current_step);
    if let Some(t) = app.sim_time {
        eprintln!("Sim time        : {}", t.format("%Y-%m-%d %H:%M UTC"));
    }

    let mut events: Vec<_> = app.cumulative_counts.iter().collect();
    events.sort_by(|a, b| b.1.cmp(a.1));

    if events.is_empty() {
        eprintln!("Events          : (none)");
    } else {
        eprintln!("Events:");
        for (k, v) in &events {
            eprintln!("  {:<32} {}", short_event_type(k), v);
        }
    }

    let total: u64 = events.iter().map(|(_, v)| **v).sum();
    if app.current_step > 0 {
        eprintln!(
            "Throughput      : {:.1} events/step",
            total as f64 / app.current_step as f64
        );
    }

    let created: u64 = app
        .cumulative_counts
        .iter()
        .filter(|(k, _)| k.ends_with(".created"))
        .map(|(_, v)| v)
        .sum();
    let completed: u64 = app
        .cumulative_counts
        .iter()
        .filter(|(k, _)| k.ends_with(".completed") || k.ends_with(".delivered"))
        .map(|(_, v)| v)
        .sum();
    if created > 0 {
        eprintln!(
            "Completion      : {completed}/{created} = {:.1}%",
            completed as f64 / created as f64 * 100.0
        );
    }
}
