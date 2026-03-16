// Custom Resource Implementation Template
//
// This template provides a well-commented boilerplate for creating new resource types.
// Resources represent capacity-constrained processors like machines, workstations, or workers.

use sim_core::{Resource, ResourceId, Task, TaskId};
use chrono::Duration;
use std::fmt;

// =============================================================================
// Step 1: Define Your Resource Struct
// =============================================================================

/// Your custom resource type.
/// Resources must track capacity and current load.
#[derive(Debug, Clone)]
pub struct MyResource {
    /// Unique identifier (required)
    id: ResourceId,

    /// Resource name for debugging/logging
    name: String,

    /// Maximum concurrent tasks (required)
    capacity: usize,

    /// Current number of active tasks (required)
    current_load: usize,

    // Add your domain-specific fields here:
    /// Example: Resource efficiency factor
    efficiency: f64,

    /// Example: Maintenance schedule
    next_maintenance: Option<chrono::DateTime<chrono::Utc>>,

    /// Example: Accumulated statistics
    total_tasks_processed: usize,
}

// =============================================================================
// Step 2: Implement the Resource Trait
// =============================================================================

impl Resource for MyResource {
    // -------------------------------------------------------------------------
    // Required: Unique identifier
    // -------------------------------------------------------------------------
    fn id(&self) -> ResourceId {
        self.id
    }

    // -------------------------------------------------------------------------
    // Required: Resource type for task matching
    // -------------------------------------------------------------------------
    fn resource_type(&self) -> &str {
        "my_resource"  // Change to your domain-specific type
    }

    // -------------------------------------------------------------------------
    // Required: Maximum capacity
    // -------------------------------------------------------------------------
    fn capacity(&self) -> usize {
        self.capacity
    }

    // -------------------------------------------------------------------------
    // Required: Current load
    // -------------------------------------------------------------------------
    fn current_load(&self) -> usize {
        self.current_load
    }

    // -------------------------------------------------------------------------
    // Optional: Custom availability logic
    // -------------------------------------------------------------------------
    fn is_available(&self) -> bool {
        // Default implementation checks: current_load < capacity
        // Override if you need custom logic (e.g., maintenance windows)

        // Check if under maintenance
        if let Some(maintenance_time) = self.next_maintenance {
            if chrono::Utc::now() >= maintenance_time {
                return false;
            }
        }

        // Check standard capacity
        self.current_load < self.capacity
    }

    // -------------------------------------------------------------------------
    // Optional: Custom available capacity calculation
    // -------------------------------------------------------------------------
    fn available_capacity(&self) -> usize {
        // Default: capacity - current_load
        // Override for custom logic (e.g., efficiency-adjusted capacity)

        let raw_available = self.capacity.saturating_sub(self.current_load);

        // Example: Adjust for efficiency
        (raw_available as f64 * self.efficiency) as usize
    }
}

// =============================================================================
// Step 3: Define Corresponding Task Type
// =============================================================================

/// Tasks that this resource processes.
#[derive(Debug, Clone)]
pub struct MyTask {
    /// Unique identifier (required)
    id: TaskId,

    /// Task description
    description: String,

    /// How long this task takes to complete
    estimated_duration: Duration,

    /// Optional: Task priority (higher = more urgent)
    priority: i32,

    /// Optional: Workflow instance if part of a workflow
    workflow_instance_id: Option<sim_core::WorkflowInstanceId>,

    // Add your domain-specific fields:
    /// Example: Complexity factor
    complexity: f64,

    /// Example: Required skill level
    required_skill: u8,
}

// =============================================================================
// Step 4: Implement the Task Trait
// =============================================================================

impl Task for MyTask {
    // -------------------------------------------------------------------------
    // Required: Unique identifier
    // -------------------------------------------------------------------------
    fn id(&self) -> TaskId {
        self.id
    }

    // -------------------------------------------------------------------------
    // Required: Required resource type (must match Resource::resource_type)
    // -------------------------------------------------------------------------
    fn required_resource_type(&self) -> &str {
        "my_resource"  // Must match MyResource::resource_type()
    }

    // -------------------------------------------------------------------------
    // Required: Estimated completion time
    // -------------------------------------------------------------------------
    fn estimated_duration(&self) -> Duration {
        // Example: Adjust duration by complexity
        Duration::seconds((self.estimated_duration.num_seconds() as f64 * self.complexity) as i64)
    }

    // -------------------------------------------------------------------------
    // Optional: Task priority
    // -------------------------------------------------------------------------
    fn priority(&self) -> i32 {
        self.priority
    }

    // -------------------------------------------------------------------------
    // Optional: Workflow instance ID
    // -------------------------------------------------------------------------
    fn workflow_instance_id(&self) -> Option<sim_core::WorkflowInstanceId> {
        self.workflow_instance_id
    }
}

// =============================================================================
// Step 5: Constructor and Helper Methods
// =============================================================================

impl MyResource {
    /// Create a new resource with default configuration
    pub fn new(id: ResourceId, name: String, capacity: usize) -> Self {
        Self {
            id,
            name,
            capacity,
            current_load: 0,
            efficiency: 1.0,
            next_maintenance: None,
            total_tasks_processed: 0,
        }
    }

    /// Create with custom efficiency
    pub fn with_efficiency(mut self, efficiency: f64) -> Self {
        self.efficiency = efficiency.clamp(0.0, 1.0);
        self
    }

    /// Schedule maintenance
    pub fn schedule_maintenance(&mut self, time: chrono::DateTime<chrono::Utc>) {
        self.next_maintenance = Some(time);
    }

    /// Update load (called by ResourceAllocator automatically)
    pub fn update_load(&mut self, new_load: usize) {
        self.current_load = new_load;
    }

    /// Increment task counter
    pub fn record_task_completion(&mut self) {
        self.total_tasks_processed += 1;
    }

    /// Get utilization percentage
    pub fn utilization(&self) -> f64 {
        if self.capacity == 0 {
            0.0
        } else {
            (self.current_load as f64 / self.capacity as f64) * 100.0
        }
    }
}

impl MyTask {
    /// Create a new task
    pub fn new(description: String, duration: Duration) -> Self {
        Self {
            id: TaskId::new(),
            description,
            estimated_duration: duration,
            priority: 0,
            workflow_instance_id: None,
            complexity: 1.0,
            required_skill: 1,
        }
    }

    /// Create with priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Create with workflow association
    pub fn for_workflow(mut self, instance_id: sim_core::WorkflowInstanceId) -> Self {
        self.workflow_instance_id = Some(instance_id);
        self
    }

    /// Set complexity factor
    pub fn with_complexity(mut self, complexity: f64) -> Self {
        self.complexity = complexity.max(0.1);
        self
    }

    /// Check if resource can handle this task
    pub fn can_be_processed_by(&self, resource: &MyResource) -> bool {
        // Example: Check efficiency threshold
        if resource.efficiency < 0.5 {
            return false;
        }

        // Example: Check availability
        resource.is_available()
    }
}

// =============================================================================
// Step 6: Custom Allocation Strategy (Optional)
// =============================================================================

use sim_core::AllocationStrategy;

/// Custom allocation strategy for your resource type
pub struct MyAllocationStrategy {
    /// Example: Weight efficiency vs. load
    efficiency_weight: f64,
}

impl MyAllocationStrategy {
    pub fn new(efficiency_weight: f64) -> Self {
        Self { efficiency_weight }
    }

    /// Calculate score for resource-task pairing
    fn score(&self, resource: &MyResource, task: &MyTask) -> f64 {
        let mut score = 0.0;

        // Factor 1: Available capacity (higher is better)
        let capacity_score = resource.available_capacity() as f64 / resource.capacity() as f64;
        score += capacity_score * (1.0 - self.efficiency_weight);

        // Factor 2: Efficiency (higher is better)
        score += resource.efficiency * self.efficiency_weight;

        // Factor 3: Skill match (example domain logic)
        let skill_match = if resource.efficiency >= task.complexity {
            1.0
        } else {
            0.5
        };
        score += skill_match * 0.2;

        score
    }
}

impl AllocationStrategy<MyResource, MyTask> for MyAllocationStrategy {
    fn assign(&self, task: &MyTask, available: &[&MyResource]) -> Option<ResourceId> {
        available
            .iter()
            .filter(|r| task.can_be_processed_by(r))
            .max_by(|a, b| {
                self.score(a, task)
                    .partial_cmp(&self.score(b, task))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|r| r.id())
    }
}

// =============================================================================
// Step 7: Integration Example
// =============================================================================

#[allow(dead_code)]
fn example_usage() -> sim_core::Result<()> {
    use sim_core::ResourceAllocator;

    // Create allocator
    let mut allocator = ResourceAllocator::new();

    // Add resources
    let resource1 = MyResource::new(ResourceId::new(), "Station A".to_string(), 5)
        .with_efficiency(0.9);
    let resource2 = MyResource::new(ResourceId::new(), "Station B".to_string(), 3)
        .with_efficiency(1.0);

    allocator.add_resource(resource1)?;
    allocator.add_resource(resource2)?;

    // Add tasks
    let task1 = MyTask::new("Process order".to_string(), Duration::minutes(15))
        .with_priority(5)
        .with_complexity(1.2);

    let task2 = MyTask::new("Quality check".to_string(), Duration::minutes(5))
        .with_priority(10);

    allocator.add_task(task1)?;
    allocator.add_task(task2)?;

    // Allocate using custom strategy
    let strategy = MyAllocationStrategy::new(0.6);
    let assignments = allocator.assign_tasks(&strategy, chrono::Utc::now())?;

    println!("Assigned {} tasks", assignments.len());

    Ok(())
}

// =============================================================================
// Step 8: Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_creation() {
        let resource = MyResource::new(ResourceId::new(), "Test".to_string(), 10);
        assert_eq!(resource.resource_type(), "my_resource");
        assert_eq!(resource.capacity(), 10);
        assert_eq!(resource.current_load(), 0);
        assert!(resource.is_available());
    }

    #[test]
    fn test_resource_capacity() {
        let mut resource = MyResource::new(ResourceId::new(), "Test".to_string(), 5);
        resource.current_load = 3;

        assert_eq!(resource.available_capacity(), 5);  // 5 - 3 = 2, adjusted by efficiency 1.0
        assert!(resource.is_available());

        resource.current_load = 5;
        assert!(!resource.is_available());
    }

    #[test]
    fn test_task_duration() {
        let task = MyTask::new("Test".to_string(), Duration::seconds(100))
            .with_complexity(2.0);

        // Duration should be adjusted by complexity
        assert_eq!(task.estimated_duration().num_seconds(), 200);
    }

    #[test]
    fn test_allocation_strategy() {
        let resource1 = MyResource::new(ResourceId::new(), "A".to_string(), 5)
            .with_efficiency(0.8);
        let resource2 = MyResource::new(ResourceId::new(), "B".to_string(), 5)
            .with_efficiency(1.0);

        let task = MyTask::new("Test".to_string(), Duration::minutes(5));

        let strategy = MyAllocationStrategy::new(0.7);
        let score1 = strategy.score(&resource1, &task);
        let score2 = strategy.score(&resource2, &task);

        // Resource2 should score higher due to better efficiency
        assert!(score2 > score1);
    }
}
