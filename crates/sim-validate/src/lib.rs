//! Output validation for simulation runs.
//!
//! Given a completed run's events (read from Parquet via
//! [`sim_events::ParquetEventReader`]) and artifacts (JSONL on disk), this crate
//! checks correctness invariants: event-count consistency, causal ordering,
//! lifecycle-pair matching, no duplicate terminal events, and resource capacity.
//!
//! The framework is domain-generic. A [`DomainProfile`] supplies the event-type
//! strings and lifecycle pairs for a domain; built-in profiles exist for the
//! ghost-kitchen ([`DomainProfile::kitchen`]) and logistics
//! ([`DomainProfile::logistics`]) domains. Rules implement [`InvariantRule`] and
//! report [`Violation`]s rather than panicking, so callers (the CLI, tests) can
//! aggregate and present them.
//!
//! # Example
//!
//! ```no_run
//! use sim_validate::{DomainProfile, validate_events};
//! use sim_events::ParquetEventReader;
//!
//! let events = ParquetEventReader::read_file("events.parquet").unwrap();
//! let report = validate_events(&events, &DomainProfile::kitchen());
//! if !report.is_ok() {
//!     for v in &report.violations {
//!         eprintln!("{v}");
//!     }
//! }
//! ```

mod artifacts;
mod profile;
mod rules;

pub use artifacts::{ArtifactStats, check_artifacts, read_artifact_dir};
pub use profile::{DomainProfile, LifecyclePair};
pub use rules::{
    CapacityRule, CausalOrderingRule, CountConsistencyRule, InvariantRule, LifecyclePairRule,
    NoDuplicateTerminalRule,
};

use sim_events::EventRecord;
use std::fmt;

/// A single invariant violation found during validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The rule that produced this violation (e.g. `"count-consistency"`).
    pub rule: String,
    /// Human-readable description of what failed.
    pub message: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.rule, self.message)
    }
}

/// The result of validating a run: the violations found plus per-type counts.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// All violations across every rule that ran.
    pub violations: Vec<Violation>,
    /// Count of events seen per event type, sorted for stable display.
    pub event_counts: Vec<(String, usize)>,
    /// Total number of events validated.
    pub total_events: usize,
}

impl Report {
    /// True when no invariant was violated.
    pub fn is_ok(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Validate an event stream against a domain profile's default rule set.
///
/// Runs the profile's rules and aggregates their violations into a [`Report`].
pub fn validate_events(events: &[EventRecord], profile: &DomainProfile) -> Report {
    let mut event_counts: std::collections::BTreeMap<String, usize> = Default::default();
    for e in events {
        *event_counts.entry(e.event_type.clone()).or_default() += 1;
    }

    let mut violations = Vec::new();
    for rule in profile.rules() {
        violations.extend(rule.check(events));
    }

    Report {
        violations,
        event_counts: event_counts.into_iter().collect(),
        total_events: events.len(),
    }
}
