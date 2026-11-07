//! Built-in allocation strategies

use super::traits::{AllocationStrategy, ResourceCandidate, TaskCandidate};
use crate::utils::idents::ResourceId;
use rand::SeedableRng;
use rand::rngs::StdRng;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Always pick the first candidate (resources in the order they were added).
#[derive(Debug, Default, Clone, Copy)]
pub struct FirstFit;

impl AllocationStrategy for FirstFit {
    fn assign(
        &self,
        _task: &TaskCandidate<'_>,
        candidates: &[ResourceCandidate<'_>],
    ) -> Option<ResourceId> {
        candidates.first().map(|c| c.id)
    }
}

/// Round-robin allocation strategy
///
/// Rotates through the candidate list on each call. This spreads tasks evenly
/// but ignores current load.
#[derive(Debug, Default)]
pub struct RoundRobin {
    counter: AtomicUsize,
}

impl RoundRobin {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AllocationStrategy for RoundRobin {
    fn assign(
        &self,
        _task: &TaskCandidate<'_>,
        candidates: &[ResourceCandidate<'_>],
    ) -> Option<ResourceId> {
        if candidates.is_empty() {
            return None;
        }
        let idx = self.counter.fetch_add(1, Ordering::Relaxed) % candidates.len();
        Some(candidates[idx].id)
    }
}

/// Least-loaded allocation strategy
///
/// Picks the candidate with the lowest current load. Ties go to the resource
/// added first.
#[derive(Debug, Default, Clone, Copy)]
pub struct LeastLoaded;

impl AllocationStrategy for LeastLoaded {
    fn assign(
        &self,
        _task: &TaskCandidate<'_>,
        candidates: &[ResourceCandidate<'_>],
    ) -> Option<ResourceId> {
        candidates
            .iter()
            .min_by_key(|c| c.current_load())
            .map(|c| c.id)
    }
}

/// Random allocation strategy
///
/// Picks a uniformly random candidate. Use [`Random::with_seed`] (typically
/// with [`derive_seed`](crate::derive_seed)) for reproducible runs.
#[derive(Debug)]
pub struct Random {
    rng: Mutex<StdRng>,
}

impl Random {
    /// Create a random strategy seeded from system entropy (non-reproducible).
    pub fn new() -> Self {
        Self {
            rng: Mutex::new(StdRng::from_seed(rand::random())),
        }
    }

    /// Create a random strategy with a specific seed for reproducibility.
    pub fn with_seed(seed: u64) -> Self {
        Self {
            rng: Mutex::new(StdRng::seed_from_u64(seed)),
        }
    }
}

impl Default for Random {
    fn default() -> Self {
        Self::new()
    }
}

impl AllocationStrategy for Random {
    fn assign(
        &self,
        _task: &TaskCandidate<'_>,
        candidates: &[ResourceCandidate<'_>],
    ) -> Option<ResourceId> {
        if candidates.is_empty() {
            return None;
        }
        use rand::RngExt;
        let mut rng = self.rng.lock().unwrap_or_else(|e| e.into_inner());
        let idx = rng.random_range(0..candidates.len());
        Some(candidates[idx].id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::idents::TaskId;

    fn task() -> TaskCandidate<'static> {
        TaskCandidate {
            id: TaskId::new(),
            required_resource_type: "test",
            priority: 0,
            workflow_instance_id: None,
        }
    }

    fn candidate(load: usize) -> ResourceCandidate<'static> {
        ResourceCandidate {
            id: ResourceId::new(),
            resource_type: "test",
            capacity: 10,
            available: 10 - load,
        }
    }

    #[test]
    fn first_fit_picks_first() {
        let cands = [candidate(5), candidate(0)];
        assert_eq!(FirstFit.assign(&task(), &cands), Some(cands[0].id));
    }

    #[test]
    fn round_robin_rotates() {
        let strategy = RoundRobin::new();
        let cands = [candidate(0), candidate(0)];
        let id1 = strategy.assign(&task(), &cands).unwrap();
        let id2 = strategy.assign(&task(), &cands).unwrap();
        let id3 = strategy.assign(&task(), &cands).unwrap();
        assert_eq!(id1, cands[0].id);
        assert_eq!(id2, cands[1].id);
        assert_eq!(id3, cands[0].id);
    }

    #[test]
    fn least_loaded_picks_min_load() {
        let cands = [candidate(5), candidate(2), candidate(8)];
        assert_eq!(LeastLoaded.assign(&task(), &cands), Some(cands[1].id));
    }

    #[test]
    fn least_loaded_breaks_ties_by_order() {
        let cands = [candidate(1), candidate(1)];
        assert_eq!(LeastLoaded.assign(&task(), &cands), Some(cands[0].id));
    }

    #[test]
    fn random_is_reproducible_with_seed() {
        let cands: Vec<_> = (0..8).map(|_| candidate(0)).collect();
        let a = Random::with_seed(42);
        let b = Random::with_seed(42);
        let picks_a: Vec<_> = (0..20).map(|_| a.assign(&task(), &cands)).collect();
        let picks_b: Vec<_> = (0..20).map(|_| b.assign(&task(), &cands)).collect();
        assert_eq!(picks_a, picks_b);
        assert!(
            picks_a
                .iter()
                .all(|p| cands.iter().any(|c| Some(c.id) == *p))
        );
    }

    #[test]
    fn empty_candidates_yield_none() {
        assert!(LeastLoaded.assign(&task(), &[]).is_none());
        assert!(RoundRobin::new().assign(&task(), &[]).is_none());
        assert!(Random::with_seed(1).assign(&task(), &[]).is_none());
        assert!(FirstFit.assign(&task(), &[]).is_none());
    }
}
