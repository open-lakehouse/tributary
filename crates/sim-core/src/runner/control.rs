//! Bidirectional control channel for `SimulationRunner`
//!
//! This module provides a typed command/event channel so an external caller
//! (TUI, HTTP server, test harness) can control a simulation running on a
//! background thread without sharing the runner directly.
//!
//! # Usage
//!
//! ```rust,ignore
//! use sim_core::{control_channel, ControlCommand, SimEvent};
//!
//! let (controller, receiver) = control_channel(32);
//!
//! // Move `receiver` into the sim thread; keep `controller` on the caller side.
//! std::thread::spawn(move || {
//!     runner.attach_control(receiver);
//!     runner.run_controlled().unwrap();
//! });
//!
//! // Send commands from the caller side
//! controller.send(ControlCommand::Pause).unwrap();
//! // …
//! controller.send(ControlCommand::Stop).unwrap();
//! ```

use crate::Result;
use crate::context::TimeMode;
use chrono::{DateTime, Utc};
use std::sync::mpsc;
use uuid::Uuid;

// ─── Commands (caller → runner) ───────────────────────────────────────────────

/// Commands that can be sent to a running simulation.
#[derive(Debug)]
pub enum ControlCommand {
    /// Start the run loop (sent implicitly by `run_controlled`; useful for
    /// testing the channel in isolation).
    Start,
    /// Request the simulation to stop at the end of the current tick.
    Stop,
    /// Pause the simulation after the current tick completes.
    Pause,
    /// Resume a paused simulation.
    Resume,
    /// While paused, advance exactly N steps then pause again.
    StepN(usize),
    /// Change the time mode mid-simulation.
    SetMode(TimeMode),
    /// Set the speed multiplier for Live mode (clamped to [0.125, 16.0]).
    SetSpeed(f32),
    /// Request an immediate snapshot. The runner generates a Uuid v7, saves
    /// state, and emits [`SimEvent::SnapshotSaved`] with the ID on success.
    Snapshot(String),
}

// ─── Events (runner → caller) ─────────────────────────────────────────────────

/// Events emitted by the simulation to observers.
#[derive(Debug, Clone)]
pub enum SimEvent {
    /// Emitted after every successful tick.
    TickCompleted {
        step: usize,
        sim_time: DateTime<Utc>,
        event_counts: std::collections::HashMap<String, u64>,
    },
    /// Emitted when the time mode changes (e.g. Catchup → Live).
    ModeChanged { mode: String },
    /// Emitted when the simulation enters the paused state.
    Paused { at_step: usize },
    /// Emitted when the run loop exits normally.
    SimulationEnded {
        total_steps: usize,
        final_time: DateTime<Utc>,
    },
    /// Emitted when the run loop exits due to an error.
    Error(String),
    /// Emitted when the Live-mode speed multiplier changes.
    SpeedChanged { multiplier: f32 },
    /// Emitted after a snapshot is successfully saved via [`ControlCommand::Snapshot`].
    SnapshotSaved {
        snapshot_id: Uuid,
        label: String,
        sim_time: DateTime<Utc>,
        step: usize,
    },
}

// ─── Public controller (caller side) ─────────────────────────────────────────

/// The caller-side handle returned by [`control_channel`].
///
/// Send [`ControlCommand`]s and receive [`SimEvent`]s. `SimulationController`
/// is `Send`, so it can be shared across threads (e.g., moved into a Tokio task
/// while the simulation runs on a `std::thread`).
pub struct SimulationController {
    cmd_tx: mpsc::SyncSender<ControlCommand>,
    event_rx: mpsc::Receiver<SimEvent>,
}

impl SimulationController {
    /// Send a command to the simulation.
    ///
    /// Blocks briefly if the internal channel buffer is full (bounded channel).
    pub fn send(&self, cmd: ControlCommand) -> Result<()> {
        self.cmd_tx
            .send(cmd)
            .map_err(|e| crate::Error::Custom(format!("control channel closed: {e}")))
    }

    /// Block until the next [`SimEvent`] arrives.
    pub fn recv(&self) -> Result<SimEvent> {
        self.event_rx
            .recv()
            .map_err(|_| crate::Error::Custom("event channel closed".to_string()))
    }

    /// Non-blocking poll for the next [`SimEvent`].
    ///
    /// Returns `None` if no event is currently available.
    pub fn try_recv(&self) -> Option<SimEvent> {
        self.event_rx.try_recv().ok()
    }
}

// `SimulationController` must be `Send` so it can cross thread boundaries
// (e.g., into a Tokio task or Axum handler).
// SAFETY: `mpsc::SyncSender` and `mpsc::Receiver` are both `Send` when T: Send.
unsafe impl Send for SimulationController {}

// ─── Internal receiver (runner side) ─────────────────────────────────────────

/// The runner-side handle.  Created alongside `SimulationController` by
/// [`control_channel`] and passed to the runner via
/// [`SimulationRunner::attach_control`].
pub struct ControlReceiver {
    cmd_rx: mpsc::Receiver<ControlCommand>,
    event_tx: mpsc::SyncSender<SimEvent>,
}

impl ControlReceiver {
    /// Drain all pending commands without blocking.
    pub fn drain_commands(&self) -> Vec<ControlCommand> {
        let mut cmds = Vec::new();
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            cmds.push(cmd);
        }
        cmds
    }

    /// Emit an event to the caller.  Drops the event silently if the caller
    /// has disconnected (common when the test harness drops the controller).
    pub fn emit(&self, event: SimEvent) {
        let _ = self.event_tx.try_send(event);
    }
}

// ─── Factory function ─────────────────────────────────────────────────────────

/// Create a matched pair of `(SimulationController, ControlReceiver)`.
///
/// # Arguments
///
/// * `bound` — capacity of the bounded channels.  A value of 32–128 is usually
///   sufficient; larger values reduce the chance of `emit()` dropping events.
pub fn control_channel(bound: usize) -> (SimulationController, ControlReceiver) {
    let (cmd_tx, cmd_rx) = mpsc::sync_channel(bound);
    let (event_tx, event_rx) = mpsc::sync_channel(bound);

    let controller = SimulationController { cmd_tx, event_rx };
    let receiver = ControlReceiver { cmd_rx, event_tx };

    (controller, receiver)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests for the channel types themselves (no full runner needed).
    /// Integration tests with a real runner live in `sim-kitchen` / `sim-logistics`.

    #[test]
    fn test_channel_send_recv() {
        let (controller, receiver) = control_channel(8);
        controller.send(ControlCommand::Stop).unwrap();
        let cmds = receiver.drain_commands();
        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], ControlCommand::Stop));
    }

    #[test]
    fn test_event_emit_try_recv() {
        let (controller, receiver) = control_channel(8);
        receiver.emit(SimEvent::Paused { at_step: 5 });
        let event = controller.try_recv();
        assert!(matches!(event, Some(SimEvent::Paused { at_step: 5 })));
    }

    #[test]
    fn test_try_recv_empty() {
        let (controller, _receiver) = control_channel(8);
        assert!(controller.try_recv().is_none());
    }

    #[test]
    fn test_drain_commands_empty() {
        let (_controller, receiver) = control_channel(8);
        let cmds = receiver.drain_commands();
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_stop_command() {
        let (controller, receiver) = control_channel(8);
        controller.send(ControlCommand::Stop).unwrap();
        let cmds = receiver.drain_commands();
        assert!(cmds.iter().any(|c| matches!(c, ControlCommand::Stop)));
    }

    #[test]
    fn test_pause_then_resume() {
        let (controller, receiver) = control_channel(8);
        controller.send(ControlCommand::Pause).unwrap();
        controller.send(ControlCommand::Resume).unwrap();
        let cmds = receiver.drain_commands();
        assert_eq!(cmds.len(), 2);
        assert!(matches!(cmds[0], ControlCommand::Pause));
        assert!(matches!(cmds[1], ControlCommand::Resume));
    }

    #[test]
    fn test_step_n_while_paused() {
        let (controller, receiver) = control_channel(8);
        controller.send(ControlCommand::StepN(3)).unwrap();
        let cmds = receiver.drain_commands();
        assert!(matches!(cmds[0], ControlCommand::StepN(3)));
    }

    #[test]
    fn test_set_mode_via_channel() {
        use crate::context::TimeMode;
        let (controller, receiver) = control_channel(8);
        controller
            .send(ControlCommand::SetMode(TimeMode::Range { end_time: None }))
            .unwrap();
        let cmds = receiver.drain_commands();
        assert!(matches!(
            &cmds[0],
            ControlCommand::SetMode(TimeMode::Range { end_time: None })
        ));
    }
}
