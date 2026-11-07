//! Food preparation workflow

use crate::resources::Kitchen;
use crate::resources::kitchen_order::KITCHEN_NAMESPACE;
use chrono::Duration;
use sim_core::workflows::WorkflowStep;
use sim_core::{Resource, Workflow, WorkflowId};
use std::collections::HashMap;

/// Preparing one order in one kitchen
///
/// 1. **Prep** (2 min): gather ingredients and measure portions
/// 2. **Cook** (5 min)
/// 3. **Package** (1 min)
///
/// Every step runs in the same kitchen (they all require that kitchen's
/// resource type), so there is one workflow definition per kitchen. Its ID is
/// derived from the kitchen name, so snapshots taken in one process restore
/// in another.
#[derive(Debug, Clone)]
pub struct FoodPrepWorkflow {
    id: WorkflowId,
    steps: Vec<WorkflowStep>,
}

impl FoodPrepWorkflow {
    /// Preparation steps as `(name, duration)`
    pub const STEPS: [(&'static str, i64); 3] = [("prep", 120), ("cook", 300), ("package", 60)];

    /// The preparation workflow for `kitchen`
    pub fn for_kitchen(kitchen: &Kitchen) -> Self {
        let steps = Self::STEPS
            .iter()
            .enumerate()
            .map(|(step_index, (name, secs))| WorkflowStep {
                step_index,
                required_resource_type: kitchen.resource_type().to_string(),
                expected_duration: Duration::seconds(*secs),
                metadata: HashMap::from([("name".to_string(), (*name).to_string())]),
            })
            .collect();
        Self {
            id: WorkflowId::from_name(KITCHEN_NAMESPACE, &format!("food_prep/{}", kitchen.name)),
            steps,
        }
    }

    /// Total time an order spends being prepared, excluding queueing
    pub fn total_duration() -> Duration {
        Duration::seconds(Self::STEPS.iter().map(|(_, s)| s).sum())
    }
}

impl Workflow for FoodPrepWorkflow {
    fn id(&self) -> WorkflowId {
        self.id
    }

    fn workflow_type(&self) -> &str {
        "food_preparation"
    }

    fn steps(&self) -> &[WorkflowStep] {
        &self.steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_run_in_the_kitchen() {
        let kitchen = Kitchen::new("K", 3);
        let wf = FoodPrepWorkflow::for_kitchen(&kitchen);
        assert_eq!(wf.steps().len(), 3);
        assert!(
            wf.steps()
                .iter()
                .all(|s| s.required_resource_type == kitchen.resource_type())
        );
        assert_eq!(FoodPrepWorkflow::total_duration(), Duration::minutes(8));
        assert_eq!(wf.id(), FoodPrepWorkflow::for_kitchen(&kitchen).id());
    }
}
