//! `caspers validate` — check a completed run's events and artifacts.

use crate::error::{Error, Result};
use sim_events::ParquetEventReader;
use sim_validate::{DomainProfile, check_artifacts, read_artifact_dir, validate_events};

/// Which domain's invariants to check.
#[derive(Debug, Clone, Default, clap::ValueEnum)]
pub(crate) enum Domain {
    /// Ghost-kitchen order lifecycle.
    #[default]
    Kitchen,
    /// Package-delivery lifecycle.
    Logistics,
}

#[derive(Debug, Clone, clap::Parser)]
pub(crate) struct ValidateArgs {
    /// Path to the events Parquet file (as written by `caspers run --events-out`).
    #[arg(long, value_name = "FILE")]
    events: std::path::PathBuf,

    /// Optional artifacts directory to cross-check (default: skip if not given).
    #[arg(long, value_name = "DIR")]
    artifacts: Option<std::path::PathBuf>,

    /// Domain profile whose invariants to apply.
    #[arg(long, default_value = "kitchen")]
    domain: Domain,

    /// Kitchen capacity bound to enforce (kitchen domain only). When set, peak
    /// concurrent assigned-but-uncompleted orders must not exceed this.
    #[arg(long)]
    kitchen_capacity: Option<usize>,
}

pub(super) async fn handle(args: ValidateArgs) -> Result<()> {
    let events = ParquetEventReader::read_file(&args.events)
        .map_err(|e| Error::Other(format!("reading events {}: {e}", args.events.display())))?;

    let profile = match args.domain {
        Domain::Kitchen => {
            let p = DomainProfile::kitchen();
            match args.kitchen_capacity {
                Some(cap) => p.with_capacity(
                    "io.kitchen.order.assigned",
                    "io.kitchen.order.completed",
                    cap,
                ),
                None => p,
            }
        }
        Domain::Logistics => DomainProfile::logistics(),
    };

    let mut report = validate_events(&events, &profile);

    // Optionally cross-check artifacts against the events.
    let mut artifact_total = None;
    if let Some(dir) = &args.artifacts {
        match read_artifact_dir(dir)
            .map_err(|e| Error::Other(format!("reading artifacts {}: {e}", dir.display())))?
        {
            Some(stats) => {
                report.violations.extend(check_artifacts(&stats, &events));
                artifact_total = Some(stats.total);
            }
            None => eprintln!("(artifacts dir {} not found; skipping)", dir.display()),
        }
    }

    // ── Report ──────────────────────────────────────────────────────────────
    println!("Validation report ({} domain)", profile.name);
    println!("  events: {}", report.total_events);
    for (ty, n) in &report.event_counts {
        println!("    {ty:.<48} {n}");
    }
    if let Some(total) = artifact_total {
        println!("  artifacts: {total}");
    }

    if report.is_ok() {
        println!("\n✅ all invariants hold");
        Ok(())
    } else {
        println!("\n❌ {} violation(s):", report.violations.len());
        for v in &report.violations {
            println!("  {v}");
        }
        Err(Error::Other(format!(
            "{} invariant violation(s)",
            report.violations.len()
        )))
    }
}
