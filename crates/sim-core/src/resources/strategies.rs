//! Built-in allocation strategies

use super::traits::{AllocationStrategy, Resource, Task};
use crate::utils::idents::ResourceId;
use rand::Rng;

/// Round-robin allocation strategy
///
/// Assigns tasks to resources in a rotating fashion. This ensures even distribution
/// but doesn't account for current load.
pub struct RoundRobin {
    counter: std::sync::atomic::AtomicUsize,
}

impl RoundRobin {
    pub fn new() -> Self {
        Self {
            counter: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

impl Default for RoundRobin {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: Resource, T: Task> AllocationStrategy<R, T> for RoundRobin {
    fn assign(&self, _task: &T, available: &[&R]) -> Option<ResourceId> {
        if available.is_empty() {
            return None;
        }

        let idx = self
            .counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            % available.len();
        Some(available[idx].id())
    }
}

/// Least-loaded allocation strategy
///
/// Assigns tasks to the resource with the lowest current load.
/// This provides good load balancing when tasks have varying durations.
pub struct LeastLoaded;

impl<R: Resource, T: Task> AllocationStrategy<R, T> for LeastLoaded {
    fn assign(&self, _task: &T, available: &[&R]) -> Option<ResourceId> {
        available
            .iter()
            .min_by_key(|r| r.current_load())
            .map(|r| r.id())
    }
}

/// Random allocation strategy
///
/// Assigns tasks randomly to available resources.
/// Useful for testing or when no specific strategy is needed.
pub struct Random;

impl<R: Resource, T: Task> AllocationStrategy<R, T> for Random {
    fn assign(&self, _task: &T, available: &[&R]) -> Option<ResourceId> {
        if available.is_empty() {
            return None;
        }

        let idx = rand::rng().random_range(0..available.len());
        Some(available[idx].id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::traits::Resource;

    #[derive(Debug)]
    struct TestResource {
        id: ResourceId,
        load: usize,
    }

    impl Resource for TestResource {
        fn id(&self) -> ResourceId {
            self.id
        }
        fn resource_type(&self) -> &str {
            "test"
        }
        fn capacity(&self) -> usize {
            10
        }
        fn current_load(&self) -> usize {
            self.load
        }
    }

    #[derive(Debug)]
    struct TestTask {
        id: crate::utils::idents::TaskId,
    }

    impl Task for TestTask {
        fn id(&self) -> crate::utils::idents::TaskId {
            self.id
        }
        fn required_resource_type(&self) -> &str {
            "test"
        }
        fn estimated_duration(&self) -> chrono::Duration {
            chrono::Duration::seconds(10)
        }
    }

    #[test]
    fn test_round_robin() {
        let strategy = RoundRobin::new();
        let resources = vec![
            TestResource {
                id: ResourceId::new(),
                load: 0,
            },
            TestResource {
                id: ResourceId::new(),
                load: 0,
            },
        ];
        let refs: Vec<&TestResource> = resources.iter().collect();
        let task = TestTask {
            id: crate::utils::idents::TaskId::new(),
        };

        let id1 = strategy.assign(&task, &refs).unwrap();
        let id2 = strategy.assign(&task, &refs).unwrap();

        // Should alternate between resources
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_least_loaded() {
        let strategy = LeastLoaded;
        let resources = vec![
            TestResource {
                id: ResourceId::new(),
                load: 5,
            },
            TestResource {
                id: ResourceId::new(),
                load: 2,
            },
            TestResource {
                id: ResourceId::new(),
                load: 8,
            },
        ];
        let refs: Vec<&TestResource> = resources.iter().collect();
        let task = TestTask {
            id: crate::utils::idents::TaskId::new(),
        };

        let chosen_id = strategy.assign(&task, &refs).unwrap();
        // Should choose the resource with load=2
        assert_eq!(chosen_id, resources[1].id());
    }

    #[test]
    fn test_random() {
        let strategy = Random;
        let resources = vec![
            TestResource {
                id: ResourceId::new(),
                load: 0,
            },
            TestResource {
                id: ResourceId::new(),
                load: 0,
            },
        ];
        let refs: Vec<&TestResource> = resources.iter().collect();
        let task = TestTask {
            id: crate::utils::idents::TaskId::new(),
        };

        let chosen_id = strategy.assign(&task, &refs);
        assert!(chosen_id.is_some());
        assert!(chosen_id == Some(resources[0].id()) || chosen_id == Some(resources[1].id()));
    }

    #[test]
    fn test_empty_resources() {
        let strategy = LeastLoaded;
        let task = TestTask {
            id: crate::utils::idents::TaskId::new(),
        };

        let empty: Vec<&TestResource> = vec![];
        let chosen_id = strategy.assign(&task, &empty);
        assert!(chosen_id.is_none());
    }
}
