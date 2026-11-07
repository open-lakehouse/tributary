use super::*;
use crate::resources::{FirstFit, LeastLoaded, RoundRobin};
use chrono::TimeZone;

#[derive(Debug)]
struct TestResource {
    id: ResourceId,
    kind: &'static str,
    capacity: usize,
}

impl Resource for TestResource {
    fn id(&self) -> ResourceId {
        self.id
    }
    fn resource_type(&self) -> &str {
        self.kind
    }
    fn capacity(&self) -> usize {
        self.capacity
    }
    fn current_load(&self) -> usize {
        0
    }
}

#[derive(Debug)]
struct TestTask {
    id: TaskId,
    kind: &'static str,
    duration: Duration,
    workflow: Option<WorkflowInstanceId>,
    priority: i32,
}

impl Task for TestTask {
    fn id(&self) -> TaskId {
        self.id
    }
    fn required_resource_type(&self) -> &str {
        self.kind
    }
    fn estimated_duration(&self) -> Duration {
        self.duration
    }
    fn priority(&self) -> i32 {
        self.priority
    }
    fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
        self.workflow
    }
}

type Alloc = ResourceAllocator<TestResource, TestTask>;

fn t0() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap()
}

fn resource(kind: &'static str, capacity: usize) -> TestResource {
    TestResource {
        id: ResourceId::new(),
        kind,
        capacity,
    }
}

fn task(kind: &'static str, secs: i64) -> TestTask {
    TestTask {
        id: TaskId::new(),
        kind,
        duration: Duration::seconds(secs),
        workflow: None,
        priority: 0,
    }
}

fn add_resource(a: &mut Alloc, kind: &'static str, capacity: usize) -> ResourceId {
    let r = resource(kind, capacity);
    let id = r.id;
    a.add_resource(r).unwrap();
    id
}

fn add_task(a: &mut Alloc, t: TestTask) -> TaskId {
    let id = t.id;
    a.add_task(t).unwrap();
    id
}

#[test]
fn assign_matches_type_and_consumes_capacity() {
    let mut a = Alloc::new();
    let oven = add_resource(&mut a, "oven", 2);
    add_resource(&mut a, "grill", 5);
    let t1 = add_task(&mut a, task("oven", 10));
    let t2 = add_task(&mut a, task("oven", 10));
    let t3 = add_task(&mut a, task("oven", 10));

    let assigned = a.assign_tasks(&LeastLoaded, t0()).unwrap();
    assert_eq!(
        assigned,
        vec![
            Assignment {
                task_id: t1,
                resource_id: oven,
                workflow_instance_id: None,
            },
            Assignment {
                task_id: t2,
                resource_id: oven,
                workflow_instance_id: None,
            },
        ]
    );
    assert_eq!(a.resource_available(oven), Some(0));
    assert_eq!(a.pending_tasks().collect::<Vec<_>>(), vec![t3]);
    assert_eq!(a.allocated_task_count(), 2);
}

#[test]
fn tasks_without_workflow_complete_and_release_capacity() {
    // Regression: tasks without a workflow instance used to hold capacity forever.
    let mut a = Alloc::new();
    let r = add_resource(&mut a, "test", 1);
    let t = add_task(&mut a, task("test", 10));
    a.assign_tasks(&LeastLoaded, t0()).unwrap();
    assert_eq!(a.resource_available(r), Some(0));

    assert!(
        a.get_completed_tasks(t0() + Duration::seconds(9))
            .unwrap()
            .is_empty()
    );
    let done = a.get_completed_tasks(t0() + Duration::seconds(10)).unwrap();
    assert_eq!(
        done,
        vec![CompletedTask {
            task_id: t,
            resource_id: r,
            workflow_instance_id: None
        }]
    );
    assert_eq!(a.resource_available(r), Some(1));
    assert_eq!(a.get_assignment(t), None);
    // Completed tasks are gone, so completing again is an error, not a double release.
    assert!(a.complete_task(t).is_err());
    assert_eq!(a.resource_available(r), Some(1));
}

#[test]
fn completed_tasks_report_workflow_instance() {
    let mut a = Alloc::new();
    add_resource(&mut a, "test", 5);
    let wf = WorkflowInstanceId::new();
    let with_wf = add_task(
        &mut a,
        TestTask {
            workflow: Some(wf),
            ..task("test", 10)
        },
    );
    let without = add_task(&mut a, task("test", 10));
    a.assign_tasks(&LeastLoaded, t0()).unwrap();

    let done = a.get_completed_tasks(t0() + Duration::seconds(10)).unwrap();
    let ids: Vec<_> = done
        .iter()
        .map(|c| (c.task_id, c.workflow_instance_id))
        .collect();
    assert_eq!(ids, vec![(with_wf, Some(wf)), (without, None)]);
}

#[test]
fn complete_task_early_releases_once() {
    let mut a = Alloc::new();
    let r = add_resource(&mut a, "test", 1);
    let t = add_task(&mut a, task("test", 600));
    a.assign_tasks(&LeastLoaded, t0()).unwrap();

    let done = a.complete_task(t).unwrap();
    assert_eq!(done.resource_id, r);
    assert_eq!(a.resource_available(r), Some(1));
    assert!(
        a.get_completed_tasks(t0() + Duration::hours(1))
            .unwrap()
            .is_empty()
    );
    assert_eq!(a.resource_available(r), Some(1));
}

#[test]
fn complete_pending_task_is_error() {
    let mut a = Alloc::new();
    let t = add_task(&mut a, task("test", 1));
    assert!(a.complete_task(t).is_err());
    assert_eq!(a.pending_task_count(), 1);
}

#[test]
fn remove_allocated_task_releases_capacity() {
    let mut a = Alloc::new();
    let r = add_resource(&mut a, "test", 1);
    let t = add_task(&mut a, task("test", 60));
    a.assign_tasks(&LeastLoaded, t0()).unwrap();
    assert!(a.remove_task(t).unwrap());
    assert_eq!(a.resource_available(r), Some(1));
    assert!(!a.remove_task(t).unwrap());
}

#[test]
fn remove_resource_with_allocations_is_rejected() {
    let mut a = Alloc::new();
    let r = add_resource(&mut a, "test", 1);
    let other = add_resource(&mut a, "test", 1);
    add_task(&mut a, task("test", 60));
    a.assign_tasks(&FirstFit, t0()).unwrap();
    assert!(a.remove_resource(r).is_err());
    assert!(a.remove_resource(other).unwrap());
    assert_eq!(a.resource_count(), 1);
    assert_eq!(a.resource_available(r), Some(0));
}

#[test]
fn duplicate_ids_are_rejected() {
    let mut a = Alloc::new();
    let r = resource("test", 1);
    let rid = r.id;
    a.add_resource(r).unwrap();
    assert!(
        a.add_resource(TestResource {
            id: rid,
            kind: "test",
            capacity: 1
        })
        .is_err()
    );

    let t = task("test", 1);
    let tid = t.id;
    a.add_task(t).unwrap();
    assert!(
        a.add_task(TestTask {
            id: tid,
            ..task("test", 1)
        })
        .is_err()
    );
}

#[test]
fn strategy_is_honoured() {
    let mut a = Alloc::new();
    let r1 = add_resource(&mut a, "test", 10);
    let r2 = add_resource(&mut a, "test", 10);
    for _ in 0..4 {
        add_task(&mut a, task("test", 60));
    }
    let picks: Vec<_> = a
        .assign_tasks(&RoundRobin::new(), t0())
        .unwrap()
        .into_iter()
        .map(|x| x.resource_id)
        .collect();
    assert_eq!(picks, vec![r1, r2, r1, r2]);

    let mut b = Alloc::new();
    let b1 = add_resource(&mut b, "test", 10);
    let b2 = add_resource(&mut b, "test", 10);
    for _ in 0..4 {
        add_task(&mut b, task("test", 60));
    }
    let picks: Vec<_> = b
        .assign_tasks(&LeastLoaded, t0())
        .unwrap()
        .into_iter()
        .map(|x| x.resource_id)
        .collect();
    assert_eq!(picks, vec![b1, b2, b1, b2]);

    let mut c = Alloc::new();
    let c1 = add_resource(&mut c, "test", 10);
    add_resource(&mut c, "test", 10);
    for _ in 0..4 {
        add_task(&mut c, task("test", 60));
    }
    assert!(
        c.assign_tasks(&FirstFit, t0())
            .unwrap()
            .iter()
            .all(|x| x.resource_id == c1)
    );
}

#[test]
fn strategy_returning_non_candidate_is_error() {
    struct Rogue;
    impl AllocationStrategy for Rogue {
        fn assign(&self, _: &TaskCandidate<'_>, _: &[ResourceCandidate<'_>]) -> Option<ResourceId> {
            Some(ResourceId::new())
        }
    }
    let mut a = Alloc::new();
    add_resource(&mut a, "test", 1);
    add_task(&mut a, task("test", 1));
    assert!(a.assign_tasks(&Rogue, t0()).is_err());
}

#[test]
fn higher_priority_allocated_first() {
    let mut a = Alloc::new();
    add_resource(&mut a, "test", 1);
    let low = add_task(&mut a, task("test", 60));
    let high = add_task(
        &mut a,
        TestTask {
            priority: 5,
            ..task("test", 60)
        },
    );
    let assigned = a.assign_tasks(&LeastLoaded, t0()).unwrap();
    assert_eq!(assigned.len(), 1);
    assert_eq!(assigned[0].task_id, high);
    assert_eq!(a.pending_tasks().collect::<Vec<_>>(), vec![low]);
}

#[test]
fn allocate_batch_only_touches_given_tasks() {
    let mut a = Alloc::new();
    add_resource(&mut a, "test", 10);
    let t1 = add_task(&mut a, task("test", 60));
    let t2 = add_task(&mut a, task("test", 60));
    let assigned = a
        .allocate_batch([t2, t2, TaskId::new()], &LeastLoaded, t0())
        .unwrap();
    assert_eq!(assigned.len(), 1);
    assert_eq!(assigned[0].task_id, t2);
    assert_eq!(a.pending_tasks().collect::<Vec<_>>(), vec![t1]);
}

#[test]
fn no_matching_resource_leaves_task_pending() {
    let mut a = Alloc::new();
    add_resource(&mut a, "grill", 3);
    add_task(&mut a, task("oven", 1));
    assert!(a.assign_tasks(&LeastLoaded, t0()).unwrap().is_empty());
    assert_eq!(a.pending_task_count(), 1);
}

#[test]
fn export_import_round_trip() {
    let mut a = Alloc::new();
    let r = add_resource(&mut a, "test", 3);
    let wf = WorkflowInstanceId::new();
    let running = add_task(
        &mut a,
        TestTask {
            workflow: Some(wf),
            priority: 2,
            duration: Duration::milliseconds(1500),
            ..task("test", 0)
        },
    );
    a.assign_tasks(&LeastLoaded, t0()).unwrap();
    let pending = add_task(&mut a, task("other", 30));

    let (resources, tasks) = a.export_batch().unwrap();
    assert_eq!(resources.num_rows(), 1);
    assert_eq!(tasks.num_rows(), 2);

    let mut b = Alloc::new();
    b.import_batch(resources, tasks).unwrap();
    assert_eq!(b.resource_available(r), Some(2));
    assert_eq!(b.get_assignment(running), Some(r));
    assert_eq!(b.get_task_workflow_instance_id(running), Some(wf));
    assert_eq!(
        b.get_task_estimated_duration(running),
        Some(Duration::milliseconds(1500))
    );
    assert_eq!(b.pending_tasks().collect::<Vec<_>>(), vec![pending]);

    // Timing survives the round trip.
    assert!(
        b.get_completed_tasks(t0() + Duration::milliseconds(1499))
            .unwrap()
            .is_empty()
    );
    let done = b
        .get_completed_tasks(t0() + Duration::milliseconds(1500))
        .unwrap();
    assert_eq!(done.len(), 1);
    assert_eq!(b.resource_available(r), Some(3));
}

#[test]
fn import_rejects_inconsistent_capacity_and_leaves_state() {
    let mut a = Alloc::new();
    add_resource(&mut a, "test", 2);
    add_task(&mut a, task("test", 30));
    a.assign_tasks(&LeastLoaded, t0()).unwrap();
    let (_, tasks) = a.export_batch().unwrap();
    // Re-export with the resource back at full capacity while its task is still
    // allocated: free + allocated would exceed capacity.
    a.complete_task(a.assignments()[0].task_id).unwrap();
    let (resources, _) = a.export_batch().unwrap();

    let mut b = Alloc::new();
    let keep = add_resource(&mut b, "keep", 1);
    assert!(b.import_batch(resources, tasks).is_err());
    assert_eq!(b.resource_count(), 1);
    assert_eq!(b.resource_available(keep), Some(1));
}

#[test]
fn import_rejects_wrong_schema() {
    let mut a = Alloc::new();
    let bad = RecordBatch::new_empty(task_schema());
    assert!(
        a.import_batch(bad, RecordBatch::new_empty(task_schema()))
            .is_err()
    );
}

#[test]
fn many_tasks_scale_linearly() {
    // Previously every add_task rebuilt the whole batch (O(n^2)).
    let mut a = Alloc::new();
    for _ in 0..100 {
        add_resource(&mut a, "test", 10);
    }
    for _ in 0..10_000 {
        add_task(&mut a, task("test", 30));
    }
    assert_eq!(a.assign_tasks(&LeastLoaded, t0()).unwrap().len(), 1000);
    assert_eq!(
        a.get_completed_tasks(t0() + Duration::seconds(30))
            .unwrap()
            .len(),
        1000
    );
    assert_eq!(a.pending_task_count(), 9000);
}
