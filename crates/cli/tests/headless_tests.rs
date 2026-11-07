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

/// Run `caspers` in `dir` and return its output, failing on a non-zero exit
fn caspers(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    let output = Command::new(env!("CARGO_BIN_EXE_caspers"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run caspers");
    assert!(
        output.status.success(),
        "caspers {args:?} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn summary(output: &std::process::Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).expect("JSON summary on stdout")
}

/// A default headless run writes events and artifacts under `output/`, and
/// `caspers validate` accepts both.
#[test]
fn headless_output_validates() {
    let workdir = tempfile::tempdir().unwrap();
    let dir = workdir.path();
    caspers(
        dir,
        &["run", "--no-tui", "--duration", "120", "--seed", "3"],
    );
    assert!(dir.join("output/events.parquet").is_file());
    caspers(
        dir,
        &[
            "validate",
            "--events",
            "output/events.parquet",
            "--artifacts",
            "output/artifacts",
            "--kitchen-capacity",
            "15",
        ],
    );
}

/// Without `--start-time`, a seeded run starts at a fixed instant instead of
/// the wall clock, so its event times reproduce.
#[test]
fn seeded_runs_start_at_a_fixed_time() {
    let workdir = tempfile::tempdir().unwrap();
    let run = || {
        summary(&caspers(
            workdir.path(),
            &["run", "--no-tui", "--duration", "10", "--seed", "1"],
        ))
    };
    let first = run();
    assert_eq!(first["sim_time"], "2026-01-05T11:05:00+00:00");
    assert_eq!(first, run());
}

/// `--domain logistics` runs the package simulation, and its events pass the
/// logistics invariants.
#[test]
fn logistics_output_validates() {
    let workdir = tempfile::tempdir().unwrap();
    let dir = workdir.path();
    let run = summary(&caspers(
        dir,
        &[
            "run",
            "--no-tui",
            "--domain",
            "logistics",
            "--duration",
            "480",
            "--seed",
            "5",
        ],
    ));
    assert_eq!(run["steps"], 480);
    assert!(
        run["packages_delivered"].as_u64().unwrap() > 0,
        "four hours should deliver packages: {run}"
    );
    let validate = caspers(
        dir,
        &[
            "validate",
            "--domain",
            "logistics",
            "--events",
            "output/events.parquet",
        ],
    );
    let report = String::from_utf8_lossy(&validate.stdout);
    assert!(
        report.contains("io.logistics.package.in_transit"),
        "{report}"
    );
}

/// The TUI only knows the kitchen.
#[test]
fn logistics_needs_no_tui() {
    let workdir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_caspers"))
        .args(["run", "--domain", "logistics", "--duration", "5"])
        .current_dir(workdir.path())
        .output()
        .expect("run caspers");
    assert!(!output.status.success(), "expected a non-zero exit");
}
