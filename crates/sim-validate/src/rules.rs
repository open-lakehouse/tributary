//! Invariant rules. Each rule inspects an event stream and reports violations.

use crate::Violation;
use sim_events::EventRecord;
use std::collections::{HashMap, HashSet};

/// A check over a completed run's event stream.
///
/// Rules are intentionally side-effect-free and return all violations they find
/// rather than stopping at the first, so a single validation pass surfaces every
/// problem at once.
pub trait InvariantRule {
    /// Short stable identifier used to label violations (e.g. `"causal-ordering"`).
    fn name(&self) -> &str;
    /// Inspect the events and return any violations.
    fn check(&self, events: &[EventRecord]) -> Vec<Violation>;
}

/// Extract a string field from an event's JSON `data` payload.
fn data_field(record: &EventRecord, key: &str) -> Option<String> {
    record
        .data
        .get(key)
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Count events of a given type.
fn count(events: &[EventRecord], event_type: &str) -> usize {
    events.iter().filter(|e| e.event_type == event_type).count()
}

/// A lifecycle stage that an entity passes through, identified by its event type
/// and the `data` key that carries the entity id.
#[derive(Debug, Clone)]
pub struct Stage {
    /// The event type string (e.g. `"io.kitchen.order.created"`).
    pub event_type: String,
    /// The `data` key carrying the entity id (e.g. `"order_id"`).
    pub id_key: String,
}

impl Stage {
    pub fn new(event_type: impl Into<String>, id_key: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            id_key: id_key.into(),
        }
    }
}

/// Rule: counts must be monotonically non-increasing along an ordered pipeline.
///
/// For stages `[created, assigned, completed]`, asserts
/// `count(created) >= count(assigned) >= count(completed)`. A later stage having
/// more events than an earlier one indicates lost or fabricated entities.
pub struct CountConsistencyRule {
    pub stages: Vec<String>,
}

impl InvariantRule for CountConsistencyRule {
    fn name(&self) -> &str {
        "count-consistency"
    }

    fn check(&self, events: &[EventRecord]) -> Vec<Violation> {
        let mut violations = Vec::new();
        for pair in self.stages.windows(2) {
            let (earlier, later) = (&pair[0], &pair[1]);
            let c_earlier = count(events, earlier);
            let c_later = count(events, later);
            if c_later > c_earlier {
                violations.push(Violation {
                    rule: self.name().to_string(),
                    message: format!(
                        "more `{later}` events ({c_later}) than `{earlier}` ({c_earlier})"
                    ),
                });
            }
        }
        violations
    }
}

/// Rule: every entity reaching a later lifecycle stage must have passed through
/// each earlier stage (matched by entity id).
pub struct LifecyclePairRule {
    pub stages: Vec<Stage>,
}

impl InvariantRule for LifecyclePairRule {
    fn name(&self) -> &str {
        "lifecycle-pair"
    }

    fn check(&self, events: &[EventRecord]) -> Vec<Violation> {
        let mut violations = Vec::new();

        // Collect the id set seen at each stage.
        let ids_at: Vec<HashSet<String>> = self
            .stages
            .iter()
            .map(|stage| {
                events
                    .iter()
                    .filter(|e| e.event_type == stage.event_type)
                    .filter_map(|e| data_field(e, &stage.id_key))
                    .collect()
            })
            .collect();

        // Each id at stage N must appear at every earlier stage.
        for n in 1..self.stages.len() {
            for id in &ids_at[n] {
                for (m, earlier) in self.stages.iter().enumerate().take(n) {
                    if !ids_at[m].contains(id) {
                        violations.push(Violation {
                            rule: self.name().to_string(),
                            message: format!(
                                "`{}` for id `{id}` has no preceding `{}`",
                                self.stages[n].event_type, earlier.event_type
                            ),
                        });
                    }
                }
            }
        }
        violations
    }
}

/// Rule: per entity, lifecycle events must appear in emission order
/// (created before assigned before completed, etc.).
pub struct CausalOrderingRule {
    pub stages: Vec<Stage>,
}

impl InvariantRule for CausalOrderingRule {
    fn name(&self) -> &str {
        "causal-ordering"
    }

    fn check(&self, events: &[EventRecord]) -> Vec<Violation> {
        let mut violations = Vec::new();
        let stage_rank: HashMap<&str, usize> = self
            .stages
            .iter()
            .enumerate()
            .map(|(i, s)| (s.event_type.as_str(), i))
            .collect();

        // For each entity id, record the (rank, position) of each lifecycle event
        // in emission order, then check ranks are non-decreasing.
        let mut per_id: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
        for (pos, e) in events.iter().enumerate() {
            if let Some(&rank) = stage_rank.get(e.event_type.as_str()) {
                let stage = &self.stages[rank];
                if let Some(id) = data_field(e, &stage.id_key) {
                    per_id.entry(id).or_default().push((rank, pos));
                }
            }
        }

        for (id, mut seq) in per_id {
            seq.sort_by_key(|(_, pos)| *pos);
            for pair in seq.windows(2) {
                if pair[1].0 < pair[0].0 {
                    violations.push(Violation {
                        rule: self.name().to_string(),
                        message: format!(
                            "id `{id}`: `{}` emitted before `{}`",
                            self.stages[pair[1].0].event_type, self.stages[pair[0].0].event_type
                        ),
                    });
                }
            }
        }
        violations
    }
}

/// Rule: terminal lifecycle events must not be duplicated per entity.
///
/// E.g. an order should be `created`, `assigned`, and `completed` at most once.
pub struct NoDuplicateTerminalRule {
    pub stages: Vec<Stage>,
}

impl InvariantRule for NoDuplicateTerminalRule {
    fn name(&self) -> &str {
        "no-duplicate-terminal"
    }

    fn check(&self, events: &[EventRecord]) -> Vec<Violation> {
        let mut violations = Vec::new();
        for stage in &self.stages {
            let mut seen: HashSet<String> = HashSet::new();
            for e in events.iter().filter(|e| e.event_type == stage.event_type) {
                if let Some(id) = data_field(e, &stage.id_key)
                    && !seen.insert(id.clone())
                {
                    violations.push(Violation {
                        rule: self.name().to_string(),
                        message: format!("duplicate `{}` for id `{id}`", stage.event_type),
                    });
                }
            }
        }
        violations
    }
}

/// Rule: at no point may concurrently-active entities exceed a capacity bound.
///
/// Walks the stream counting `+1` on `acquire` events and `-1` on `release`
/// events (matched by id), tracking the running peak.
pub struct CapacityRule {
    pub acquire: String,
    pub release: String,
    pub capacity: usize,
}

impl InvariantRule for CapacityRule {
    fn name(&self) -> &str {
        "capacity"
    }

    fn check(&self, events: &[EventRecord]) -> Vec<Violation> {
        let mut active: i64 = 0;
        let mut peak: i64 = 0;
        for e in events {
            if e.event_type == self.acquire {
                active += 1;
                peak = peak.max(active);
            } else if e.event_type == self.release {
                active -= 1;
            }
        }
        if peak > self.capacity as i64 {
            vec![Violation {
                rule: self.name().to_string(),
                message: format!(
                    "peak concurrent active ({peak}) exceeded capacity ({})",
                    self.capacity
                ),
            }]
        } else {
            Vec::new()
        }
    }
}
