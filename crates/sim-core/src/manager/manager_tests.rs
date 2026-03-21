use super::*;
use crate::snapshot::SnapshotMetadata;

fn make_metadata(simulation_id: Uuid, label: &str) -> SnapshotMetadata {
    SnapshotMetadata::with_ids(
        Uuid::now_v7(),
        simulation_id,
        Some(label.to_string()),
        Utc::now(),
        "1.0".to_string(),
        10,
        5,
        20,
        3,
        0,
    )
}

#[tokio::test]
async fn test_create_simulation_returns_uuid() {
    let mut mgr = SimulationManager::new(SimulationManagerBackend::Memory)
        .await
        .unwrap();

    let id = mgr.create_simulation(Some("test sim"), None).await.unwrap();

    // Must be a valid non-nil UUID
    assert_ne!(id, Uuid::nil());
}

#[tokio::test]
async fn test_list_simulations_empty() {
    let mgr = SimulationManager::new(SimulationManagerBackend::Memory)
        .await
        .unwrap();

    let sims = mgr.list_simulations().await.unwrap();
    assert!(sims.is_empty());
}

#[tokio::test]
async fn test_list_simulations_after_create() {
    let mut mgr = SimulationManager::new(SimulationManagerBackend::Memory)
        .await
        .unwrap();

    let id = mgr
        .create_simulation(
            Some("kitchen sim"),
            Some(serde_json::json!({"kitchens": 3})),
        )
        .await
        .unwrap();

    let sims = mgr.list_simulations().await.unwrap();
    assert_eq!(sims.len(), 1);
    assert_eq!(sims[0].id, id);
    assert_eq!(sims[0].label.as_deref(), Some("kitchen sim"));
    assert!(sims[0].properties.is_some());
}

#[tokio::test]
async fn test_multiple_simulations() {
    let mut mgr = SimulationManager::new(SimulationManagerBackend::Memory)
        .await
        .unwrap();

    let id1 = mgr.create_simulation(Some("sim-1"), None).await.unwrap();
    let id2 = mgr.create_simulation(Some("sim-2"), None).await.unwrap();

    let sims = mgr.list_simulations().await.unwrap();
    assert_eq!(sims.len(), 2);

    let ids: Vec<Uuid> = sims.iter().map(|s| s.id).collect();
    assert!(ids.contains(&id1));
    assert!(ids.contains(&id2));
}

#[tokio::test]
async fn test_record_and_list_snapshots() {
    let mut mgr = SimulationManager::new(SimulationManagerBackend::Memory)
        .await
        .unwrap();

    let sim_id = mgr.create_simulation(None, None).await.unwrap();
    let meta = make_metadata(sim_id, "checkpoint_1");

    mgr.record_snapshot(&meta).await.unwrap();

    let snaps = mgr.list_snapshots(sim_id).await.unwrap();
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].id, meta.snapshot_id);
    assert_eq!(snaps[0].simulation_id, sim_id);
    assert_eq!(snaps[0].label.as_deref(), Some("checkpoint_1"));
    assert_eq!(snaps[0].agent_count, 10);
}

#[tokio::test]
async fn test_list_snapshots_filters_by_simulation() {
    let mut mgr = SimulationManager::new(SimulationManagerBackend::Memory)
        .await
        .unwrap();

    let sim1 = mgr.create_simulation(None, None).await.unwrap();
    let sim2 = mgr.create_simulation(None, None).await.unwrap();

    mgr.record_snapshot(&make_metadata(sim1, "snap-a"))
        .await
        .unwrap();
    mgr.record_snapshot(&make_metadata(sim1, "snap-b"))
        .await
        .unwrap();
    mgr.record_snapshot(&make_metadata(sim2, "snap-c"))
        .await
        .unwrap();

    let snaps1 = mgr.list_snapshots(sim1).await.unwrap();
    assert_eq!(snaps1.len(), 2);

    let snaps2 = mgr.list_snapshots(sim2).await.unwrap();
    assert_eq!(snaps2.len(), 1);
}

#[tokio::test]
async fn test_list_snapshots_empty_for_unknown_simulation() {
    let mgr = SimulationManager::new(SimulationManagerBackend::Memory)
        .await
        .unwrap();

    let unknown = Uuid::now_v7();
    let snaps = mgr.list_snapshots(unknown).await.unwrap();
    assert!(snaps.is_empty());
}
