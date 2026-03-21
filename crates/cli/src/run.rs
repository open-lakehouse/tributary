use crossterm::event::{Event, KeyCode, KeyEventKind};
use sim_kitchen::{KitchenSimulation, SimulationMessage};
use tokio::sync::mpsc;

use crate::app::App;
use crate::error::Result;
use crate::tui::Tui;

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
}

pub(super) async fn handle(args: RunArgs) -> Result<()> {
    let duration = args.duration;

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<SimulationMessage>();

    let sim_handle = std::thread::spawn(move || {
        let mut sim = KitchenSimulation::new(args.num_kitchens, args.kitchen_capacity, false);
        sim.add_customers(args.num_customers);
        sim.set_tui_channel(event_tx.clone(), duration);

        if let Err(e) = sim.run_for_steps(duration) {
            let _ = event_tx.send(SimulationMessage::Error(e.to_string()));
        }
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

    // Wait for simulation thread and handle any panics
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
            // Channel closed unexpectedly
            tracing::warn!("Simulation channel closed unexpectedly");
            app.mark_completed();
        }
    }
}
