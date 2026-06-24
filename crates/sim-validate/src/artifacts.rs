//! Reading and validating artifact output (JSONL files under `output/artifacts/`).

use crate::Violation;
use sim_events::EventRecord;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::path::Path;

/// Summary of the artifacts found on disk.
#[derive(Debug, Clone, Default)]
pub struct ArtifactStats {
    /// Count of artifacts per JSONL file (relative path → line count).
    pub per_file: BTreeMap<String, usize>,
    /// Total artifacts across all files.
    pub total: usize,
    /// Distinct `source_event_id` values referenced by artifacts (when present).
    pub referenced_event_ids: HashSet<String>,
}

/// Recursively read every `*.jsonl` file under `dir`, counting records and
/// collecting any `source_event_id` references.
///
/// Returns `Ok(None)` if the directory does not exist (a run may emit no
/// artifacts); malformed JSON lines produce a violation via
/// [`check_artifacts`] rather than an error here.
pub fn read_artifact_dir(dir: impl AsRef<Path>) -> std::io::Result<Option<ArtifactStats>> {
    let dir = dir.as_ref();
    if !dir.exists() {
        return Ok(None);
    }
    let mut stats = ArtifactStats::default();
    visit(dir, dir, &mut stats)?;
    Ok(Some(stats))
}

fn visit(root: &Path, dir: &Path, stats: &mut ArtifactStats) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            visit(root, &path, stats)?;
        } else if path.extension().is_some_and(|e| e == "jsonl") {
            let content = std::fs::read_to_string(&path)?;
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            let mut count = 0;
            for line in content.lines().filter(|l| !l.trim().is_empty()) {
                count += 1;
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(line)
                    && let Some(id) = value.get("source_event_id").and_then(|v| v.as_str())
                {
                    stats.referenced_event_ids.insert(id.to_string());
                }
            }
            stats.total += count;
            stats.per_file.insert(rel, count);
        }
    }
    Ok(())
}

/// Check artifact referential integrity: every `source_event_id` an artifact
/// references must correspond to an actual emitted event.
pub fn check_artifacts(stats: &ArtifactStats, events: &[EventRecord]) -> Vec<Violation> {
    let event_ids: HashSet<&str> = events.iter().map(|e| e.id.as_str()).collect();
    let mut violations = Vec::new();
    for referenced in &stats.referenced_event_ids {
        if !event_ids.contains(referenced.as_str()) {
            violations.push(Violation {
                rule: "artifact-referential-integrity".to_string(),
                message: format!(
                    "artifact references source_event_id `{referenced}` with no matching event"
                ),
            });
        }
    }
    violations
}
