//! Performance benchmarks for workflow batch operations
//!
//! These benchmarks validate that batch update operations provide significant
//! performance improvements over sequential updates. The batch API should be
//! 10x+ faster for updating multiple workflow instances.
//!
//! Run with:
//!
//! ```bash
//! cargo bench --bench workflow_performance
//! ```
//!
//! Expected results:
//! - Batch updates should be 10x+ faster than sequential updates
//! - Performance advantage should scale with the number of updates
//! - Overhead should remain O(N) regardless of update count

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use sim_core::{
    utils::idents::{WorkflowId, WorkflowInstanceId},
    workflows::{
        BatchUpdateList, InstanceStatus, WorkflowInstance, WorkflowInstanceRowUpdate,
        WorkflowInstanceTable,
    },
};
use std::hint::black_box;

/// Create a test workflow instance
fn create_test_workflow(workflow_id: WorkflowId) -> WorkflowInstance {
    WorkflowInstance::new(workflow_id, 5)
}

/// Populate a table with workflow instances
fn populate_table(count: usize) -> (WorkflowInstanceTable, Vec<WorkflowInstanceId>) {
    let mut table = WorkflowInstanceTable::new();
    let workflow_id = WorkflowId::new();
    let mut instance_ids = Vec::with_capacity(count);

    for _ in 0..count {
        let instance = create_test_workflow(workflow_id);
        instance_ids.push(instance.id);
        table.append_instance(&instance).unwrap();
    }

    (table, instance_ids)
}

/// Benchmark: Single workflow update (baseline)
fn bench_single_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_update");

    for total_instances in [100, 1_000, 10_000].iter() {
        group.bench_with_input(
            BenchmarkId::new("update_one", total_instances),
            total_instances,
            |b, &total_instances| {
                let (mut table, instance_ids) = populate_table(total_instances);
                let target_id = instance_ids[0];

                b.iter(|| {
                    let row_idx = table.get_row_index(target_id).unwrap();
                    table
                        .update_instance_at_row(row_idx, |u| {
                            u.current_step_index = Some(1);
                        })
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Sequential updates vs batch updates
fn bench_sequential_vs_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequential_vs_batch");

    // Test different update counts on fixed table sizes
    let test_cases = vec![
        (1_000, 10),     // 10 updates on 1,000 instances
        (1_000, 100),    // 100 updates on 1,000 instances
        (1_000, 500),    // 500 updates on 1,000 instances
        (10_000, 100),   // 100 updates on 10,000 instances
        (10_000, 1_000), // 1,000 updates on 10,000 instances
    ];

    for (total_instances, update_count) in test_cases {
        let label = format!("{}_of_{}", update_count, total_instances);

        // Sequential update benchmark
        group.bench_with_input(
            BenchmarkId::new("sequential", &label),
            &(total_instances, update_count),
            |b, &(total_instances, update_count)| {
                b.iter_batched(
                    || populate_table(total_instances),
                    |(mut table, instance_ids): (
                        WorkflowInstanceTable,
                        Vec<WorkflowInstanceId>,
                    )| {
                        for &instance_id in instance_ids.iter().take(update_count) {
                            let row_idx = table.get_row_index(instance_id).unwrap();
                            table
                                .update_instance_at_row(row_idx, |u| {
                                    u.current_step_index = Some(1);
                                })
                                .unwrap();
                        }
                    },
                    criterion::BatchSize::LargeInput,
                );
            },
        );

        // Batch update benchmark
        group.bench_with_input(
            BenchmarkId::new("batch", &label),
            &(total_instances, update_count),
            |b, &(total_instances, update_count)| {
                b.iter_batched(
                    || populate_table(total_instances),
                    |(mut table, instance_ids): (
                        WorkflowInstanceTable,
                        Vec<WorkflowInstanceId>,
                    )| {
                        let updates: BatchUpdateList = instance_ids
                            .iter()
                            .take(update_count)
                            .map(|&id| {
                                (
                                    id,
                                    Box::new(|u: &mut WorkflowInstanceRowUpdate| {
                                        u.current_step_index = Some(1);
                                    })
                                        as Box<dyn FnOnce(&mut WorkflowInstanceRowUpdate)>,
                                )
                            })
                            .collect();

                        table.batch_update_instances(black_box(updates)).unwrap();
                    },
                    criterion::BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark: Batch status updates (common operation)
fn bench_batch_status_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_status_updates");

    for (total_instances, update_count) in [(1_000, 100), (10_000, 1_000)] {
        let label = format!("complete_{}_of_{}", update_count, total_instances);

        group.bench_with_input(
            BenchmarkId::from_parameter(&label),
            &(total_instances, update_count),
            |b, &(total_instances, update_count)| {
                b.iter_batched(
                    || {
                        let (mut table, instance_ids) = populate_table(total_instances);
                        table.set_current_tick(100);
                        (table, instance_ids)
                    },
                    |(mut table, instance_ids): (
                        WorkflowInstanceTable,
                        Vec<WorkflowInstanceId>,
                    )| {
                        let current_tick = table.current_tick();
                        let updates: BatchUpdateList = instance_ids
                            .iter()
                            .take(update_count)
                            .map(|&id| {
                                (
                                    id,
                                    Box::new(move |u: &mut WorkflowInstanceRowUpdate| {
                                        u.status = Some(InstanceStatus::Complete);
                                        u.completed_at = Some(current_tick);
                                    })
                                        as Box<dyn FnOnce(&mut WorkflowInstanceRowUpdate)>,
                                )
                            })
                            .collect();

                        table.batch_update_instances(black_box(updates)).unwrap();
                    },
                    criterion::BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark: Batch progress updates (advancing workflow steps)
fn bench_batch_progress_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_progress_updates");

    for (total_instances, update_count) in [(1_000, 100), (10_000, 1_000)] {
        let label = format!("advance_{}_of_{}", update_count, total_instances);

        group.bench_with_input(
            BenchmarkId::from_parameter(&label),
            &(total_instances, update_count),
            |b, &(total_instances, update_count)| {
                b.iter_batched(
                    || populate_table(total_instances),
                    |(mut table, instance_ids): (
                        WorkflowInstanceTable,
                        Vec<WorkflowInstanceId>,
                    )| {
                        let updates: BatchUpdateList = instance_ids
                            .iter()
                            .take(update_count)
                            .enumerate()
                            .map(|(i, &id)| {
                                let step = (i % 5) + 1; // Cycle through steps 1-5
                                (
                                    id,
                                    Box::new(move |u: &mut WorkflowInstanceRowUpdate| {
                                        u.current_step_index = Some(step);
                                        if step == 5 {
                                            u.status = Some(InstanceStatus::Complete);
                                        }
                                    })
                                        as Box<dyn FnOnce(&mut WorkflowInstanceRowUpdate)>,
                                )
                            })
                            .collect();

                        table.batch_update_instances(black_box(updates)).unwrap();
                    },
                    criterion::BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark: Mixed operations (status, progress, pause)
fn bench_batch_mixed_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_mixed_updates");

    for (total_instances, update_count) in [(1_000, 100), (10_000, 1_000)] {
        let label = format!("mixed_{}_of_{}", update_count, total_instances);

        group.bench_with_input(
            BenchmarkId::from_parameter(&label),
            &(total_instances, update_count),
            |b, &(total_instances, update_count)| {
                b.iter_batched(
                    || {
                        let (mut table, instance_ids) = populate_table(total_instances);
                        table.set_current_tick(100);
                        (table, instance_ids)
                    },
                    |(mut table, instance_ids): (
                        WorkflowInstanceTable,
                        Vec<WorkflowInstanceId>,
                    )| {
                        let current_tick = table.current_tick();
                        let updates: BatchUpdateList = instance_ids
                            .iter()
                            .take(update_count)
                            .enumerate()
                            .map(|(i, &id)| {
                                match i % 3 {
                                    0 => (
                                        // Complete workflows
                                        id,
                                        Box::new(move |u: &mut WorkflowInstanceRowUpdate| {
                                            u.status = Some(InstanceStatus::Complete);
                                            u.current_step_index = Some(5);
                                            u.completed_at = Some(current_tick);
                                        })
                                            as Box<dyn FnOnce(&mut WorkflowInstanceRowUpdate)>,
                                    ),
                                    1 => (
                                        // Pause workflows
                                        id,
                                        Box::new(|u: &mut WorkflowInstanceRowUpdate| {
                                            u.status = Some(InstanceStatus::Paused);
                                        })
                                            as Box<dyn FnOnce(&mut WorkflowInstanceRowUpdate)>,
                                    ),
                                    _ => (
                                        // Advance workflows
                                        id,
                                        Box::new(|u: &mut WorkflowInstanceRowUpdate| {
                                            u.current_step_index = Some(2);
                                            u.status = Some(InstanceStatus::Running);
                                        })
                                            as Box<dyn FnOnce(&mut WorkflowInstanceRowUpdate)>,
                                    ),
                                }
                            })
                            .collect();

                        table.batch_update_instances(black_box(updates)).unwrap();
                    },
                    criterion::BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark: Append operations (for comparison baseline)
fn bench_append_instances(c: &mut Criterion) {
    let mut group = c.benchmark_group("append_instances");

    for count in [10, 100, 1_000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            let workflow_id = WorkflowId::new();

            b.iter(|| {
                let mut table = WorkflowInstanceTable::new();
                for _ in 0..count {
                    let instance = create_test_workflow(workflow_id);
                    table.append_instance(black_box(&instance)).unwrap();
                }
            });
        });
    }

    group.finish();
}

/// Benchmark: Read operations (get_instance)
fn bench_get_instance(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_instance");

    for total_instances in [100, 1_000, 10_000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(total_instances),
            total_instances,
            |b, &total_instances| {
                let (table, instance_ids) = populate_table(total_instances);
                let target_id = instance_ids[total_instances / 2]; // Middle instance

                b.iter(|| {
                    table.get_instance(black_box(target_id)).unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Query operations (get_instances_by_status)
fn bench_get_by_status(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_by_status");

    for total_instances in [100, 1_000, 10_000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(total_instances),
            total_instances,
            |b, &total_instances| {
                let (mut table, instance_ids) = populate_table(total_instances);

                // Mark 10% as complete
                let complete_count = total_instances / 10;
                let updates: BatchUpdateList = instance_ids
                    .iter()
                    .take(complete_count)
                    .map(|&id| {
                        (
                            id,
                            Box::new(|u: &mut WorkflowInstanceRowUpdate| {
                                u.status = Some(InstanceStatus::Complete);
                            })
                                as Box<dyn FnOnce(&mut WorkflowInstanceRowUpdate)>,
                        )
                    })
                    .collect();
                table.batch_update_instances(updates).unwrap();

                b.iter(|| {
                    table
                        .get_instances_by_status(black_box(InstanceStatus::Complete))
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_single_update,
    bench_sequential_vs_batch,
    bench_batch_status_updates,
    bench_batch_progress_updates,
    bench_batch_mixed_updates,
    bench_append_instances,
    bench_get_instance,
    bench_get_by_status,
);
criterion_main!(benches);
