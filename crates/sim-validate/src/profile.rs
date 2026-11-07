//! Domain profiles: the event types, lifecycle stages, and default rules for a
//! particular simulation domain.

use crate::rules::{
    CapacityRule, CausalOrderingRule, CountConsistencyRule, InvariantRule, LifecyclePairRule,
    NoDuplicateTerminalRule, Stage,
};

/// A pair of event types bounding the active lifetime of a resource, used to
/// build a [`CapacityRule`].
#[derive(Debug, Clone)]
pub struct LifecyclePair {
    /// Event type that begins an active interval.
    pub acquire: String,
    /// Event type that ends an active interval.
    pub release: String,
}

/// The validation profile for one domain: which lifecycle stages exist and what
/// default rule set to run.
pub struct DomainProfile {
    /// Domain name (for reporting).
    pub name: String,
    /// Ordered lifecycle stages keyed by entity id.
    pub stages: Vec<Stage>,
    /// Optional capacity bound (acquire/release pair + limit).
    pub capacity: Option<(LifecyclePair, usize)>,
    /// Which stages are "terminal" (must not repeat per entity).
    pub terminal_stages: Vec<Stage>,
}

impl DomainProfile {
    /// Ghost-kitchen profile.
    ///
    /// Order lifecycle: created → assigned → completed (all keyed on `order_id`),
    /// followed by customer.eating (also keyed on `order_id`). Capacity is bounded
    /// by orders assigned-but-not-completed.
    pub fn kitchen() -> Self {
        let order_stages = vec![
            Stage::new("io.kitchen.order.created", "order_id"),
            Stage::new("io.kitchen.order.assigned", "order_id"),
            Stage::new("io.kitchen.order.completed", "order_id"),
            Stage::new("io.kitchen.customer.eating", "order_id"),
        ];
        Self {
            name: "kitchen".to_string(),
            stages: order_stages.clone(),
            capacity: None, // set via `with_capacity`
            terminal_stages: vec![
                Stage::new("io.kitchen.order.created", "order_id"),
                Stage::new("io.kitchen.order.assigned", "order_id"),
                Stage::new("io.kitchen.order.completed", "order_id"),
            ],
        }
    }

    /// Logistics profile.
    ///
    /// Package lifecycle: received → sorted → loaded → in_transit → delivered,
    /// all keyed on `package_id`. Every stage is terminal (at most once per
    /// package).
    ///
    /// There is no capacity bound: `delivered` follows the confirmation step,
    /// after the driver is released, so loaded-but-undelivered packages are not
    /// bounded by the fleet size.
    pub fn logistics() -> Self {
        let stages = vec![
            Stage::new("io.logistics.package.received", "package_id"),
            Stage::new("io.logistics.package.sorted", "package_id"),
            Stage::new("io.logistics.package.loaded", "package_id"),
            Stage::new("io.logistics.package.in_transit", "package_id"),
            Stage::new("io.logistics.package.delivered", "package_id"),
        ];
        Self {
            name: "logistics".to_string(),
            terminal_stages: stages.clone(),
            stages,
            capacity: None,
        }
    }

    /// Add a capacity bound to this profile (e.g. kitchen capacity = assigned
    /// minus completed orders must never exceed `limit`).
    pub fn with_capacity(mut self, acquire: &str, release: &str, limit: usize) -> Self {
        self.capacity = Some((
            LifecyclePair {
                acquire: acquire.to_string(),
                release: release.to_string(),
            },
            limit,
        ));
        self
    }

    /// The default rule set for this profile.
    pub fn rules(&self) -> Vec<Box<dyn InvariantRule>> {
        let mut rules: Vec<Box<dyn InvariantRule>> = vec![
            Box::new(CountConsistencyRule {
                stages: self.stages.iter().map(|s| s.event_type.clone()).collect(),
            }),
            Box::new(LifecyclePairRule {
                stages: self.stages.clone(),
            }),
            Box::new(CausalOrderingRule {
                stages: self.stages.clone(),
            }),
            Box::new(NoDuplicateTerminalRule {
                stages: self.terminal_stages.clone(),
            }),
        ];
        if let Some((pair, limit)) = &self.capacity {
            rules.push(Box::new(CapacityRule {
                acquire: pair.acquire.clone(),
                release: pair.release.clone(),
                capacity: *limit,
            }));
        }
        rules
    }
}
