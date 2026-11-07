//! Resource allocator
//!
//! Tracks capacity-constrained resources and the tasks queued for them. State
//! lives in plain Rust collections so every mutation is O(1) or O(log n); it
//! is converted to Arrow [`RecordBatch`]es only at the snapshot / query
//! boundary ([`ResourceAllocator::export_batch`] /
//! [`ResourceAllocator::import_batch`]).
//!
//! # Lifecycle of a task
//!
//! 1. [`add_task`](ResourceAllocator::add_task) queues it as *pending*.
//! 2. [`assign_tasks`](ResourceAllocator::assign_tasks) allocates pending tasks
//!    (highest priority first, ties in insertion order) to a resource chosen by
//!    the [`AllocationStrategy`], taking one unit of that resource's capacity.
//! 3. Once `allocated_at + estimated_duration` has passed,
//!    [`get_completed_tasks`](ResourceAllocator::get_completed_tasks) releases
//!    the capacity, removes the task and reports it. Domain code may also end a
//!    task early with [`complete_task`](ResourceAllocator::complete_task) or
//!    drop it with [`remove_task`](ResourceAllocator::remove_task); both release
//!    capacity if the task was allocated.
//!
//! Every task takes exactly one unit of capacity.

use super::schema::{resource_schema, task_schema, task_status};
use super::traits::{AllocationStrategy, Resource, ResourceCandidate, Task, TaskCandidate};
use crate::utils::idents::{ResourceId, TaskId, WorkflowInstanceId};
use crate::{Error, Result};
use arrow::array::{
    Array, ArrayRef, BinaryArray, Int32Array, Int64Array, RecordBatch, StringArray,
};
use chrono::{DateTime, Duration, Utc};
use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::sync::Arc;

/// Assignment of a task to a resource
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Assignment {
    pub task_id: TaskId,
    pub resource_id: ResourceId,
    /// Owning workflow instance, if the task was created for a workflow step
    pub workflow_instance_id: Option<WorkflowInstanceId>,
}

/// A task that finished and was removed from the allocator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletedTask {
    pub task_id: TaskId,
    pub resource_id: ResourceId,
    /// Owning workflow instance, if the task was created for a workflow step
    pub workflow_instance_id: Option<WorkflowInstanceId>,
}

#[derive(Debug, Clone)]
struct ResourceSlot {
    id: ResourceId,
    resource_type: String,
    capacity: usize,
    available: usize,
}

#[derive(Debug, Clone)]
struct TaskRecord {
    id: TaskId,
    resource_type: String,
    priority: i32,
    workflow_instance_id: Option<WorkflowInstanceId>,
    estimated_duration: Duration,
    /// `Some` once allocated
    allocation: Option<(ResourceId, DateTime<Utc>)>,
}

/// Manages resource capacity and task allocation
///
/// # Type Parameters
///
/// * `R` - Resource type implementing the Resource trait
/// * `T` - Task type implementing the Task trait
///
/// The allocator copies what it needs from `R` and `T` when they are added;
/// it does not keep the domain objects.
///
/// # Example
///
/// ```
/// use chrono::{Duration, Utc};
/// use sim_core::resources::{LeastLoaded, Resource, ResourceAllocator, Task};
/// use sim_core::{ResourceId, TaskId};
///
/// #[derive(Debug)]
/// struct Oven(ResourceId);
/// impl Resource for Oven {
///     fn id(&self) -> ResourceId { self.0 }
///     fn resource_type(&self) -> &str { "oven" }
///     fn capacity(&self) -> usize { 2 }
///     fn current_load(&self) -> usize { 0 }
/// }
///
/// #[derive(Debug)]
/// struct Bake(TaskId);
/// impl Task for Bake {
///     fn id(&self) -> TaskId { self.0 }
///     fn required_resource_type(&self) -> &str { "oven" }
///     fn estimated_duration(&self) -> Duration { Duration::minutes(10) }
/// }
///
/// let mut allocator = ResourceAllocator::<Oven, Bake>::new();
/// allocator.add_resource(Oven(ResourceId::new()))?;
/// allocator.add_task(Bake(TaskId::new()))?;
///
/// let t0 = Utc::now();
/// assert_eq!(allocator.assign_tasks(&LeastLoaded, t0)?.len(), 1);
/// assert_eq!(allocator.get_completed_tasks(t0 + Duration::minutes(10))?.len(), 1);
/// # Ok::<(), sim_core::Error>(())
/// ```
pub struct ResourceAllocator<R: Resource, T: Task> {
    /// Resources in insertion order
    resources: Vec<ResourceSlot>,
    resource_index: HashMap<ResourceId, usize>,

    /// Tasks keyed by insertion sequence number (gives deterministic order)
    tasks: BTreeMap<u64, TaskRecord>,
    task_index: HashMap<TaskId, u64>,
    next_seq: u64,

    _marker: PhantomData<fn() -> (R, T)>,
}

impl<R: Resource, T: Task> Default for ResourceAllocator<R, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: Resource, T: Task> ResourceAllocator<R, T> {
    /// Create an empty allocator
    pub fn new() -> Self {
        Self {
            resources: Vec::new(),
            resource_index: HashMap::new(),
            tasks: BTreeMap::new(),
            task_index: HashMap::new(),
            next_seq: 0,
            _marker: PhantomData,
        }
    }

    // ----- resources -------------------------------------------------------

    /// Add a resource
    ///
    /// Its initial free capacity is `resource.available_capacity()`.
    ///
    /// # Errors
    ///
    /// Returns an error if a resource with the same ID is already registered.
    pub fn add_resource(&mut self, resource: R) -> Result<()> {
        let id = resource.id();
        if self.resource_index.contains_key(&id) {
            return Err(Error::Resource(format!("Resource {id} already registered")));
        }
        self.resource_index.insert(id, self.resources.len());
        self.resources.push(ResourceSlot {
            id,
            resource_type: resource.resource_type().to_string(),
            capacity: resource.capacity(),
            available: resource.available_capacity(),
        });
        Ok(())
    }

    /// Add several resources
    pub fn add_resources(&mut self, resources: impl IntoIterator<Item = R>) -> Result<()> {
        for resource in resources {
            self.add_resource(resource)?;
        }
        Ok(())
    }

    /// Remove a resource
    ///
    /// Returns `Ok(false)` if the resource is unknown.
    ///
    /// # Errors
    ///
    /// Returns an error if tasks are still allocated to the resource.
    pub fn remove_resource(&mut self, id: ResourceId) -> Result<bool> {
        let Some(&row) = self.resource_index.get(&id) else {
            return Ok(false);
        };
        if self
            .tasks
            .values()
            .any(|t| matches!(t.allocation, Some((r, _)) if r == id))
        {
            return Err(Error::Resource(format!(
                "Cannot remove resource {id}: tasks are still allocated to it"
            )));
        }
        self.resources.remove(row);
        self.rebuild_resource_index();
        Ok(true)
    }

    /// Number of registered resources
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    /// Total capacity of a resource
    pub fn resource_capacity(&self, id: ResourceId) -> Option<usize> {
        self.slot(id).map(|s| s.capacity)
    }

    /// Free capacity of a resource
    pub fn resource_available(&self, id: ResourceId) -> Option<usize> {
        self.slot(id).map(|s| s.available)
    }

    /// Resources of `resource_type` with free capacity, in insertion order
    pub fn find_available_resources_by_type(&self, resource_type: &str) -> Vec<ResourceId> {
        self.resources
            .iter()
            .filter(|s| s.resource_type == resource_type && s.available > 0)
            .map(|s| s.id)
            .collect()
    }

    // ----- tasks -----------------------------------------------------------

    /// Queue a pending task
    ///
    /// # Errors
    ///
    /// Returns an error if a task with the same ID is already queued or allocated.
    pub fn add_task(&mut self, task: T) -> Result<()> {
        let id = task.id();
        if self.task_index.contains_key(&id) {
            return Err(Error::Resource(format!("Task {id} already queued")));
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        self.task_index.insert(id, seq);
        self.tasks.insert(
            seq,
            TaskRecord {
                id,
                resource_type: task.required_resource_type().to_string(),
                priority: task.priority(),
                workflow_instance_id: task.workflow_instance_id(),
                estimated_duration: task.estimated_duration(),
                allocation: None,
            },
        );
        Ok(())
    }

    /// Queue several pending tasks
    pub fn add_tasks(&mut self, tasks: impl IntoIterator<Item = T>) -> Result<()> {
        for task in tasks {
            self.add_task(task)?;
        }
        Ok(())
    }

    /// Remove a task, releasing its capacity if it was allocated
    ///
    /// Returns `Ok(false)` if the task is unknown.
    pub fn remove_task(&mut self, id: TaskId) -> Result<bool> {
        Ok(self.take_task(id)?.is_some())
    }

    /// Finish an allocated task now, releasing its capacity and removing it
    ///
    /// Use this when domain logic decides a task is done before its estimated
    /// duration has elapsed. Tasks that run their full duration are completed
    /// by [`get_completed_tasks`](Self::get_completed_tasks) instead.
    ///
    /// # Errors
    ///
    /// Returns an error if the task is unknown or still pending.
    pub fn complete_task(&mut self, task_id: TaskId) -> Result<CompletedTask> {
        let seq = *self
            .task_index
            .get(&task_id)
            .ok_or_else(|| Error::Resource(format!("Task {task_id} not found")))?;
        if self.tasks[&seq].allocation.is_none() {
            return Err(Error::Resource(format!(
                "Task {task_id} is pending and cannot be completed"
            )));
        }
        let record = self
            .take_task(task_id)?
            .expect("task presence checked above");
        let (resource_id, _) = record.allocation.expect("allocation checked above");
        Ok(CompletedTask {
            task_id,
            resource_id,
            workflow_instance_id: record.workflow_instance_id,
        })
    }

    /// Number of pending (unallocated) tasks
    pub fn pending_task_count(&self) -> usize {
        self.tasks
            .values()
            .filter(|t| t.allocation.is_none())
            .count()
    }

    /// Number of allocated (in-progress) tasks
    pub fn allocated_task_count(&self) -> usize {
        self.tasks.len() - self.pending_task_count()
    }

    /// Pending task IDs in insertion order
    pub fn pending_tasks(&self) -> impl Iterator<Item = TaskId> + '_ {
        self.tasks
            .values()
            .filter(|t| t.allocation.is_none())
            .map(|t| t.id)
    }

    /// Resource a task is allocated to (`None` if pending or unknown)
    pub fn get_assignment(&self, task_id: TaskId) -> Option<ResourceId> {
        self.record(task_id)?.allocation.map(|(r, _)| r)
    }

    /// All current allocations, in task insertion order
    pub fn assignments(&self) -> Vec<Assignment> {
        self.tasks
            .values()
            .filter_map(|t| {
                t.allocation.map(|(resource_id, _)| Assignment {
                    task_id: t.id,
                    resource_id,
                    workflow_instance_id: t.workflow_instance_id,
                })
            })
            .collect()
    }

    /// Workflow instance that owns a task
    pub fn get_task_workflow_instance_id(&self, task_id: TaskId) -> Option<WorkflowInstanceId> {
        self.record(task_id)?.workflow_instance_id
    }

    /// Estimated duration of a task
    pub fn get_task_estimated_duration(&self, task_id: TaskId) -> Option<Duration> {
        self.record(task_id).map(|t| t.estimated_duration)
    }

    // ----- allocation ------------------------------------------------------

    /// Allocate pending tasks to resources
    ///
    /// Tasks are considered highest priority first (ties in insertion order).
    /// For each, `strategy` picks among the resources of the required type
    /// that have free capacity; tasks with no candidate stay pending.
    ///
    /// # Errors
    ///
    /// Returns an error if the strategy picks a resource that was not offered.
    pub fn assign_tasks(
        &mut self,
        strategy: &dyn AllocationStrategy,
        current_time: DateTime<Utc>,
    ) -> Result<Vec<Assignment>> {
        let pending: Vec<u64> = self
            .tasks
            .iter()
            .filter(|(_, t)| t.allocation.is_none())
            .map(|(seq, _)| *seq)
            .collect();
        self.allocate_seqs(pending, strategy, current_time)
    }

    /// Like [`assign_tasks`](Self::assign_tasks), restricted to `task_ids`
    ///
    /// Unknown or already-allocated IDs are skipped.
    pub fn allocate_batch(
        &mut self,
        task_ids: impl IntoIterator<Item = TaskId>,
        strategy: &dyn AllocationStrategy,
        current_time: DateTime<Utc>,
    ) -> Result<Vec<Assignment>> {
        let mut seqs: Vec<u64> = task_ids
            .into_iter()
            .filter_map(|id| self.task_index.get(&id).copied())
            .filter(|seq| self.tasks[seq].allocation.is_none())
            .collect();
        seqs.sort_unstable();
        seqs.dedup();
        self.allocate_seqs(seqs, strategy, current_time)
    }

    /// Complete every allocated task whose estimated duration has elapsed
    ///
    /// Releases capacity, removes the tasks and returns them in insertion
    /// order. Tasks with and without a workflow instance are both completed.
    pub fn get_completed_tasks(
        &mut self,
        current_time: DateTime<Utc>,
    ) -> Result<Vec<CompletedTask>> {
        let due: Vec<TaskId> = self
            .tasks
            .values()
            .filter(|t| {
                t.allocation
                    .is_some_and(|(_, at)| at + t.estimated_duration <= current_time)
            })
            .map(|t| t.id)
            .collect();
        due.into_iter().map(|id| self.complete_task(id)).collect()
    }

    // ----- snapshot boundary ----------------------------------------------

    /// Export resources and tasks as Arrow batches
    ///
    /// The schemas are [`resource_schema`] and [`task_schema`]. Rows are in
    /// insertion order.
    pub fn export_batch(&self) -> Result<(RecordBatch, RecordBatch)> {
        let n = self.resources.len();
        let resources = RecordBatch::try_new(
            resource_schema(),
            vec![
                Arc::new(BinaryArray::from_iter_values(
                    self.resources.iter().map(|s| s.id.as_bytes().as_slice()),
                )) as ArrayRef,
                Arc::new(StringArray::from_iter_values(
                    self.resources.iter().map(|s| s.resource_type.as_str()),
                )),
                Arc::new(Int64Array::from_iter_values(
                    self.resources.iter().map(|s| s.capacity as i64),
                )),
                Arc::new(Int64Array::from_iter_values(
                    self.resources.iter().map(|s| s.available as i64),
                )),
                Arc::new(BinaryArray::from(vec![None::<&[u8]>; n])),
                Arc::new(StringArray::from(vec![None::<&str>; n])),
            ],
        )?;

        let tasks: Vec<&TaskRecord> = self.tasks.values().collect();
        let tasks = RecordBatch::try_new(
            task_schema(),
            vec![
                Arc::new(BinaryArray::from_iter_values(
                    tasks.iter().map(|t| t.id.as_bytes().as_slice()),
                )) as ArrayRef,
                Arc::new(StringArray::from_iter_values(
                    tasks.iter().map(|t| t.resource_type.as_str()),
                )),
                Arc::new(StringArray::from_iter_values(tasks.iter().map(|t| {
                    if t.allocation.is_some() {
                        task_status::ALLOCATED
                    } else {
                        task_status::PENDING
                    }
                }))),
                Arc::new(Int64Array::from_iter_values(tasks.iter().map(|_| 1))),
                Arc::new(BinaryArray::from_iter(tasks.iter().map(|t| {
                    t.allocation.as_ref().map(|(r, _)| r.as_bytes().as_slice())
                }))),
                Arc::new(BinaryArray::from_iter(tasks.iter().map(|t| {
                    t.workflow_instance_id
                        .as_ref()
                        .map(|w| w.as_bytes().as_slice())
                }))),
                Arc::new(Int32Array::from_iter_values(
                    tasks.iter().map(|t| t.priority),
                )),
                Arc::new(StringArray::from(vec![None::<&str>; tasks.len()])),
                Arc::new(Int64Array::from_iter(
                    tasks
                        .iter()
                        .map(|t| t.allocation.map(|(_, at)| at.timestamp_millis())),
                )),
                Arc::new(Int64Array::from_iter_values(
                    tasks
                        .iter()
                        .map(|t| t.estimated_duration.num_milliseconds()),
                )),
            ],
        )?;

        Ok((resources, tasks))
    }

    /// Replace the allocator state with previously exported batches
    ///
    /// The import is validated in full before anything is replaced: on error
    /// the allocator is unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error if a schema does not match, an ID is malformed or
    /// duplicated, a task references an unknown resource, or a resource's free
    /// capacity plus its allocated tasks exceeds its capacity.
    pub fn import_batch(&mut self, resources: RecordBatch, tasks: RecordBatch) -> Result<()> {
        check_schema(&resources, &resource_schema(), "resource")?;
        check_schema(&tasks, &task_schema(), "task")?;

        let mut staged = Self::new();

        let ids = column::<BinaryArray>(&resources, 0)?;
        let types = column::<StringArray>(&resources, 1)?;
        let capacities = column::<Int64Array>(&resources, 2)?;
        let available = column::<Int64Array>(&resources, 3)?;
        for i in 0..resources.num_rows() {
            let id = ResourceId::from_bytes(ids.value(i))
                .map_err(|e| Error::Resource(format!("Invalid resource ID at row {i}: {e}")))?;
            let capacity = non_negative(capacities.value(i), "capacity", i)?;
            let free = non_negative(available.value(i), "available", i)?;
            if free > capacity {
                return Err(Error::Resource(format!(
                    "Resource {id}: available ({free}) exceeds capacity ({capacity})"
                )));
            }
            if staged
                .resource_index
                .insert(id, staged.resources.len())
                .is_some()
            {
                return Err(Error::Resource(format!("Duplicate resource {id}")));
            }
            staged.resources.push(ResourceSlot {
                id,
                resource_type: types.value(i).to_string(),
                capacity,
                available: free,
            });
        }

        let ids = column::<BinaryArray>(&tasks, 0)?;
        let types = column::<StringArray>(&tasks, 1)?;
        let statuses = column::<StringArray>(&tasks, 2)?;
        let assigned = column::<BinaryArray>(&tasks, 4)?;
        let workflows = column::<BinaryArray>(&tasks, 5)?;
        let priorities = column::<Int32Array>(&tasks, 6)?;
        let allocated_at = column::<Int64Array>(&tasks, 8)?;
        let durations = column::<Int64Array>(&tasks, 9)?;
        let mut used: HashMap<ResourceId, usize> = HashMap::new();
        for i in 0..tasks.num_rows() {
            let id = TaskId::from_bytes(ids.value(i))
                .map_err(|e| Error::Resource(format!("Invalid task ID at row {i}: {e}")))?;
            let allocation = match statuses.value(i) {
                task_status::PENDING => None,
                task_status::ALLOCATED => {
                    if assigned.is_null(i) || allocated_at.is_null(i) {
                        return Err(Error::Resource(format!(
                            "Allocated task {id} has no resource or allocation time"
                        )));
                    }
                    let resource_id = ResourceId::from_bytes(assigned.value(i)).map_err(|e| {
                        Error::Resource(format!("Invalid resource ID for task {id}: {e}"))
                    })?;
                    if !staged.resource_index.contains_key(&resource_id) {
                        return Err(Error::Resource(format!(
                            "Task {id} is allocated to unknown resource {resource_id}"
                        )));
                    }
                    *used.entry(resource_id).or_default() += 1;
                    let at = DateTime::from_timestamp_millis(allocated_at.value(i)).ok_or_else(
                        || Error::Resource(format!("Invalid allocation time for task {id}")),
                    )?;
                    Some((resource_id, at))
                }
                other => {
                    return Err(Error::Resource(format!(
                        "Task {id} has unknown status '{other}'"
                    )));
                }
            };
            let workflow_instance_id = if workflows.is_null(i) {
                None
            } else {
                Some(
                    WorkflowInstanceId::from_bytes(workflows.value(i)).map_err(|e| {
                        Error::Resource(format!("Invalid workflow instance ID for task {id}: {e}"))
                    })?,
                )
            };
            let seq = staged.next_seq;
            staged.next_seq += 1;
            if staged.task_index.insert(id, seq).is_some() {
                return Err(Error::Resource(format!("Duplicate task {id}")));
            }
            staged.tasks.insert(
                seq,
                TaskRecord {
                    id,
                    resource_type: types.value(i).to_string(),
                    priority: priorities.value(i),
                    workflow_instance_id,
                    estimated_duration: Duration::milliseconds(durations.value(i)),
                    allocation,
                },
            );
        }

        // Resources may start partly loaded (`available_capacity() < capacity()`),
        // so free + allocated may be below capacity but never above it.
        for slot in &staged.resources {
            let in_use = used.get(&slot.id).copied().unwrap_or(0);
            if slot.available + in_use > slot.capacity {
                return Err(Error::Resource(format!(
                    "Resource {}: available {} + allocated {} exceeds capacity {}",
                    slot.id, slot.available, in_use, slot.capacity
                )));
            }
        }

        *self = staged;
        Ok(())
    }

    // ----- internals -------------------------------------------------------

    fn slot(&self, id: ResourceId) -> Option<&ResourceSlot> {
        self.resource_index
            .get(&id)
            .map(|&row| &self.resources[row])
    }

    fn record(&self, id: TaskId) -> Option<&TaskRecord> {
        self.task_index.get(&id).map(|seq| &self.tasks[seq])
    }

    fn rebuild_resource_index(&mut self) {
        self.resource_index = self
            .resources
            .iter()
            .enumerate()
            .map(|(row, s)| (s.id, row))
            .collect();
    }

    /// Remove a task and release its capacity if it was allocated
    fn take_task(&mut self, id: TaskId) -> Result<Option<TaskRecord>> {
        let Some(seq) = self.task_index.remove(&id) else {
            return Ok(None);
        };
        let record = self.tasks.remove(&seq).expect("task index out of sync");
        if let Some((resource_id, _)) = record.allocation {
            let row = *self.resource_index.get(&resource_id).ok_or_else(|| {
                Error::InvalidState(format!(
                    "Task {id} was allocated to unknown resource {resource_id}"
                ))
            })?;
            let slot = &mut self.resources[row];
            if slot.available >= slot.capacity {
                return Err(Error::InvalidState(format!(
                    "Resource {resource_id} released more capacity than it has"
                )));
            }
            slot.available += 1;
        }
        Ok(Some(record))
    }

    fn allocate_seqs(
        &mut self,
        mut seqs: Vec<u64>,
        strategy: &dyn AllocationStrategy,
        current_time: DateTime<Utc>,
    ) -> Result<Vec<Assignment>> {
        // Highest priority first; `seqs` is ascending so the stable sort keeps
        // insertion order among equal priorities.
        seqs.sort_by_key(|seq| std::cmp::Reverse(self.tasks[seq].priority));

        let mut assignments = Vec::new();
        for seq in seqs {
            let task = &self.tasks[&seq];
            let candidates: Vec<ResourceCandidate<'_>> = self
                .resources
                .iter()
                .filter(|s| s.resource_type == task.resource_type && s.available > 0)
                .map(|s| ResourceCandidate {
                    id: s.id,
                    resource_type: &s.resource_type,
                    capacity: s.capacity,
                    available: s.available,
                })
                .collect();
            if candidates.is_empty() {
                continue;
            }
            let view = TaskCandidate {
                id: task.id,
                required_resource_type: &task.resource_type,
                priority: task.priority,
                workflow_instance_id: task.workflow_instance_id,
            };
            let Some(resource_id) = strategy.assign(&view, &candidates) else {
                continue;
            };
            if !candidates.iter().any(|c| c.id == resource_id) {
                return Err(Error::Resource(format!(
                    "Allocation strategy chose resource {resource_id}, which was not a candidate for task {}",
                    task.id
                )));
            }
            let task_id = task.id;
            let workflow_instance_id = task.workflow_instance_id;
            let row = self.resource_index[&resource_id];
            self.resources[row].available -= 1;
            self.tasks
                .get_mut(&seq)
                .expect("seq from task map")
                .allocation = Some((resource_id, current_time));
            assignments.push(Assignment {
                task_id,
                resource_id,
                workflow_instance_id,
            });
        }
        Ok(assignments)
    }
}

fn check_schema(
    batch: &RecordBatch,
    expected: &arrow::datatypes::SchemaRef,
    what: &str,
) -> Result<()> {
    let actual = batch.schema();
    let same = actual.fields().len() == expected.fields().len()
        && actual
            .fields()
            .iter()
            .zip(expected.fields())
            .all(|(a, e)| a.name() == e.name() && a.data_type() == e.data_type());
    if same {
        Ok(())
    } else {
        Err(Error::Resource(format!(
            "{what} batch schema mismatch: expected {expected:?}, got {actual:?}"
        )))
    }
}

fn column<A: Array + 'static>(batch: &RecordBatch, idx: usize) -> Result<&A> {
    batch
        .column(idx)
        .as_any()
        .downcast_ref::<A>()
        .ok_or_else(|| Error::Resource(format!("Unexpected array type in column {idx}")))
}

fn non_negative(value: i64, field: &str, row: usize) -> Result<usize> {
    usize::try_from(value)
        .map_err(|_| Error::Resource(format!("Negative {field} ({value}) at row {row}")))
}

#[cfg(test)]
#[path = "allocator_tests.rs"]
mod tests;
