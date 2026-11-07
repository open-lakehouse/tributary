//! End-to-end checks for `caspers run --no-tui`.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A range-mode headless run must exit once it reaches `--duration` steps.
///
/// Regression test: `KitchenSimulation::run_controlled_for` pauses at the step
/// limit (so the TUI stays alive), and the headless driver used to wait for a
/// `SimulationEnded` that never came.
#[test]
fn headless_run_exits_at_step_limit() {
    // Run in a scratch dir: the CLI writes `.caspers/` and `output/` under cwd.
    let workdir = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_caspers"))
        .args(["run", "--no-tui", "--duration", "20", "--seed", "7"])
        .current_dir(workdir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn caspers");

    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait on caspers") {
            break status;
        }
        if Instant::now() > deadline {
            child.kill().ok();
            panic!("`caspers run --no-tui` did not exit within 60s");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(status.success(), "caspers exited with {status}");

    let output = child.wait_with_output().expect("collect output");
    let summary: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("JSON summary on stdout");
    assert_eq!(summary["steps"], 20);
}

/// Headless runs report failures through the exit code (they used to print a
/// summary and exit 0 whatever happened).
#[test]
fn headless_run_fails_when_events_cannot_be_written() {
    let workdir = tempfile::tempdir().unwrap();
    // A file where the output directory should be makes the write fail.
    std::fs::write(workdir.path().join("blocked"), "").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_caspers"))
        .args([
            "run",
            "--no-tui",
            "--duration",
            "5",
            "--events-out",
            "blocked/events.parquet",
        ])
        .current_dir(workdir.path())
        .output()
        .expect("run caspers");
    assert!(!output.status.success(), "expected a non-zero exit");
}

/// `--events-out` and `--artifacts-out` produce output that `caspers validate`
/// accepts.
#[test]
fn headless_output_validates() {
    let workdir = tempfile::tempdir().unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_caspers"))
        .args([
            "run",
            "--no-tui",
            "--duration",
            "120",
            "--seed",
            "3",
            "--events-out",
            "events.parquet",
            "--artifacts-out",
            "artifacts",
        ])
        .current_dir(workdir.path())
        .output()
        .expect("run caspers");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let validate = Command::new(env!("CARGO_BIN_EXE_caspers"))
        .args([
            "validate",
            "--events",
            "events.parquet",
            "--artifacts",
            "artifacts",
            "--kitchen-capacity",
            "15",
        ])
        .current_dir(workdir.path())
        .output()
        .expect("run caspers validate");
    assert!(
        validate.status.success(),
        "{}",
        String::from_utf8_lossy(&validate.stdout)
    );
}
