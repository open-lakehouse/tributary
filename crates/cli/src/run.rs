use chrono::{DateTime, Utc};
use crossterm::event::{Event, KeyCode, KeyEventKind};
use sim_core::{ControlCommand, TimeMode, control_channel};
use sim_kitchen::{KitchenSimulation, SimulationMessage};
use tokio::sync::mpsc;

use crate::app::App;
use crate::error::Result;
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

    /// Simulation start time (RFC 3339, e.g. 2026-01-01T00:00:00Z).
    /// Applies to all modes; defaults to now.
    #[arg(long)]
    start_time: Option<String>,

    /// Simulation end time (RFC 3339). Only used with --mode range.
    #[arg(long)]
    end_time: Option<String>,
}

pub(super) async fn handle(args: RunArgs) -> Result<()> {
    let duration = args.duration;

    // Parse optional start/end times
    let start_time: Option<DateTime<Utc>> = args
        .start_time
        .as_deref()
        .map(|s| {
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| crate::error::Error::Other(format!("invalid --start-time: {e}")))
        })
        .transpose()?;

    let end_time: Option<DateTime<Utc>> = args
        .end_time
        .as_deref()
        .map(|s| {
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| crate::error::Error::Other(format!("invalid --end-time: {e}")))
        })
        .transpose()?;

    // Controller lives on the TUI side; receiver goes to the sim thread.
    let (controller, receiver) = control_channel(128);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<SimulationMessage>();

    let mode = args.mode.clone();
    let sim_handle = std::thread::spawn(move || {
        let mut sim = KitchenSimulation::new(args.num_kitchens, args.kitchen_capacity, false);
        sim.add_customers(args.num_customers);

        // Apply start time if provided
        if let Some(st) = start_time {
            sim.runner_mut().context_mut().set_current_time(st);
        }

        // Configure time mode
        let time_mode = match mode {
            RunMode::Range => TimeMode::Range { end_time },
            RunMode::Live => TimeMode::Live,
            RunMode::Catchup => TimeMode::Catchup,
        };
        sim.runner_mut().set_time_mode(time_mode);

        // For range mode, attach TUI channel so progress messages stream
        let is_range = matches!(args.mode, RunMode::Range);
        if is_range {
            sim.set_tui_channel(event_tx.clone(), duration);
            if let Err(e) = sim.run_for_steps(duration) {
                let _ = event_tx.send(SimulationMessage::Error(e.to_string()));
            }
            return;
        }

        // For Live / Catchup: run a step loop, poll control channel for stop
        // signal, send progress messages manually.
        let tx = event_tx.clone();
        for i in 0..duration {
            // Poll the control receiver for a Stop command
            {
                let cmds = receiver.drain_commands();
                let mut should_stop = false;
                for cmd in cmds {
                    if matches!(cmd, ControlCommand::Stop) {
                        should_stop = true;
                    }
                }
                if should_stop {
                    break;
                }
            }

            match sim.step() {
                Ok(()) => {
                    let step = sim.runner().context().step();
                    let sim_time = sim.runner().context().current_time();
                    let _ = tx.send(SimulationMessage::StepCompleted {
                        step,
                        total_steps: duration,
                        sim_time,
                        event_counts: Default::default(),
                    });
                }
                Err(e) => {
                    let _ = tx.send(SimulationMessage::Error(e.to_string()));
                    return;
                }
            }
            let _ = i; // step counter used implicitly via range
        }

        let _ = tx.send(SimulationMessage::Completed {
            steps: sim.runner().context().step(),
            final_time: sim.runner().context().current_time(),
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
                if let Some(Event::Key(key)) = event
                    && key.kind == KeyEventKind::Press {
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
                            _ => {}
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
                if app.current_step < app.total_steps {
                    eprintln!("\n⚠️  Warning: Simulation completed early!");
                    eprintln!(
                        "   Expected {} steps, but only completed {}",
                        app.total_steps, app.current_step
                    );
                    eprintln!("   This might indicate an issue in the simulation core.");
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
        Some(SimulationMessage::Completed { steps, final_time }) => {
            tracing::info!(
                "Simulation completed successfully: {} steps, final time: {}",
                steps,
                final_time
            );
            app.mark_completed();
        }
        Some(SimulationMessage::Error(err)) => {
            tracing::error!("simulation error: {err}");
            app.set_error(err);
            app.mark_completed();
        }
        None => {
            tracing::warn!("Simulation channel closed unexpectedly");
            app.mark_completed();
        }
    }
}
