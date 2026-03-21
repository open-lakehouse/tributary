//! Workflow Registry for Multi-Workflow Support
//!
//! This module provides the `WorkflowRegistry` pattern for managing multiple
//! workflow engines concurrently. Each engine can process a different workflow
//! type (e.g., food orders, courier delivery, supply chain) simultaneously.
//!
//! # Architecture
//!
//! The registry maintains a HashMap of workflow engines indexed by name:
//! - Fast O(1) lookup by workflow name
//! - Support for heterogeneous workflow types
//! - Parallel execution of all registered workflows
//!
//! # Example
//!
//! ```rust,ignore
//! use sim_core::workflows::{WorkflowRegistry, WorkflowEngine};
//!
//! let mut registry = WorkflowRegistry::new();
//!
//! // Register multiple workflow types
//! let food_engine = WorkflowEngine::new();
//! let delivery_engine = WorkflowEngine::new();
//! let supply_engine = WorkflowEngine::new();
//!
//! registry.register("food_orders".to_string(), food_engine)?;
//! registry.register("courier_delivery".to_string(), delivery_engine)?;
//! registry.register("supply_chain".to_string(), supply_engine)?;
//!
//! // Access specific workflow engine
//! if let Some(engine) = registry.get("food_orders") {
//!     // Work with food orders workflow
//! }
//!
//! // Process all workflows in parallel
//! let events = registry.tick_all(current_time)?;
//! ```

use super::engine::{WorkflowEngine, WorkflowStepEvent};
use super::traits::Workflow;
use crate::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Registry for managing multiple workflow engines
///
/// Enables running multiple workflow types concurrently in the same simulation.
/// Each workflow engine is identified by a unique string name.
///
/// # Type Parameters
///
/// * `W` - Workflow type implementing the Workflow trait
///
/// # Thread Safety
///
/// WorkflowRegistry is not thread-safe by default. For concurrent access,
/// wrap it in `Arc<Mutex<WorkflowRegistry<W>>>`.
pub struct WorkflowRegistry<W: Workflow> {
    /// Workflow engines indexed by name
    engines: HashMap<String, WorkflowEngine<W>>,
}

impl<W: Workflow> WorkflowRegistry<W> {
    /// Create a new empty workflow registry
    ///
    /// # Returns
    ///
    /// A new WorkflowRegistry instance with no registered engines
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let registry = WorkflowRegistry::<MyWorkflow>::new();
    /// ```
    pub fn new() -> Self {
        Self {
            engines: HashMap::new(),
        }
    }

    /// Register a workflow engine with a unique name
    ///
    /// # Arguments
    ///
    /// * `name` - Unique identifier for this workflow engine
    /// * `engine` - WorkflowEngine instance to register
    ///
    /// # Returns
    ///
    /// Ok(()) on success, Error if name already exists
    ///
    /// # Errors
    ///
    /// Returns Error::Workflow if the name is already registered
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut registry = WorkflowRegistry::new();
    /// let engine = WorkflowEngine::new();
    ///
    /// registry.register("food_orders".to_string(), engine)?;
    /// ```
    pub fn register(&mut self, name: String, engine: WorkflowEngine<W>) -> Result<()> {
        if self.engines.contains_key(&name) {
            return Err(crate::Error::Workflow(format!(
                "Workflow engine '{}' is already registered",
                name
            )));
        }

        self.engines.insert(name, engine);
        Ok(())
    }

    /// Get immutable reference to a workflow engine by name
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the workflow engine to retrieve
    ///
    /// # Returns
    ///
    /// Some(&WorkflowEngine) if found, None otherwise
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if let Some(engine) = registry.get("food_orders") {
    ///     let count = engine.instance_table.instance_count();
    ///     println!("Food orders: {} instances", count);
    /// }
    /// ```
    pub fn get(&self, name: &str) -> Option<&WorkflowEngine<W>> {
        self.engines.get(name)
    }

    /// Get mutable reference to a workflow engine by name
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the workflow engine to retrieve
    ///
    /// # Returns
    ///
    /// Some(&mut WorkflowEngine) if found, None otherwise
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if let Some(engine) = registry.get_mut("food_orders") {
    ///     let workflow = FoodOrderWorkflow::new();
    ///     engine.register_workflow(workflow);
    /// }
    /// ```
    pub fn get_mut(&mut self, name: &str) -> Option<&mut WorkflowEngine<W>> {
        self.engines.get_mut(name)
    }

    /// List all registered workflow engine names
    ///
    /// # Returns
    ///
    /// Vector of workflow engine names (sorted alphabetically)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let names = registry.list();
    /// for name in names {
    ///     println!("Registered workflow: {}", name);
    /// }
    /// ```
    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.engines.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Remove a workflow engine from the registry
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the workflow engine to remove
    ///
    /// # Returns
    ///
    /// Some(WorkflowEngine) if removed, None if not found
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if let Some(engine) = registry.remove("old_workflow") {
    ///     println!("Removed workflow with {} instances",
    ///              engine.instance_table.instance_count());
    /// }
    /// ```
    pub fn remove(&mut self, name: &str) -> Option<WorkflowEngine<W>> {
        self.engines.remove(name)
    }

    /// Get the number of registered workflow engines
    ///
    /// # Returns
    ///
    /// Count of registered engines
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// println!("Registry has {} workflow types", registry.count());
    /// ```
    pub fn count(&self) -> usize {
        self.engines.len()
    }

    /// Check if a workflow engine is registered
    ///
    /// # Arguments
    ///
    /// * `name` - Name to check
    ///
    /// # Returns
    ///
    /// true if registered, false otherwise
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if registry.contains("food_orders") {
    ///     // Process food orders
    /// }
    /// ```
    pub fn contains(&self, name: &str) -> bool {
        self.engines.contains_key(name)
    }

    /// Process tick for all registered workflow engines
    ///
    /// This method iterates through all registered engines and calls their
    /// tick() method, collecting all emitted workflow events.
    ///
    /// # Arguments
    ///
    /// * `current_time` - Current simulation time
    ///
    /// # Returns
    ///
    /// Vector of all workflow step events from all engines
    ///
    /// # Errors
    ///
    /// Returns error if any engine's tick() fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let events = registry.tick_all(current_time)?;
    /// for event in events {
    ///     match event {
    ///         WorkflowStepEvent::StepCompleted { .. } => {
    ///             // Handle step completion
    ///         }
    ///         WorkflowStepEvent::WorkflowCompleted { .. } => {
    ///             // Handle workflow completion
    ///         }
    ///     }
    /// }
    /// ```
    pub fn tick_all(&mut self, current_time: DateTime<Utc>) -> Result<Vec<WorkflowStepEvent>> {
        let mut all_events = Vec::new();

        for (_name, engine) in self.engines.iter_mut() {
            let events = engine.tick(current_time)?;
            all_events.extend(events);
        }

        Ok(all_events)
    }

    /// Get total count of workflow instances across all engines
    ///
    /// # Returns
    ///
    /// Total number of workflow instances (all engines combined)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let total = registry.total_instance_count();
    /// println!("Total workflow instances: {}", total);
    /// ```
    pub fn total_instance_count(&self) -> usize {
        self.engines
            .values()
            .map(|engine| engine.instance_table.instance_count())
            .sum()
    }

    /// Get count of workflow instances for a specific engine
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the workflow engine
    ///
    /// # Returns
    ///
    /// Some(count) if engine exists, None otherwise
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if let Some(count) = registry.instance_count("food_orders") {
    ///     println!("Food order instances: {}", count);
    /// }
    /// ```
    pub fn instance_count(&self, name: &str) -> Option<usize> {
        self.engines
            .get(name)
            .map(|engine| engine.instance_table.instance_count())
    }

    /// Clear all registered workflow engines
    ///
    /// Removes all engines from the registry. Use with caution as this
    /// will lose all workflow state.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// registry.clear();
    /// assert_eq!(registry.count(), 0);
    /// ```
    pub fn clear(&mut self) {
        self.engines.clear();
    }

    /// Iterate over all workflow engines (immutable)
    pub fn engines(&self) -> impl Iterator<Item = &WorkflowEngine<W>> {
        self.engines.values()
    }

    /// Iterate over all workflow engines (mutable)
    pub fn engines_mut(&mut self) -> impl Iterator<Item = &mut WorkflowEngine<W>> {
        self.engines.values_mut()
    }
}

impl<W: Workflow> Default for WorkflowRegistry<W> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::idents::WorkflowId;
    use crate::workflows::WorkflowStep;
    use chrono::Duration;

    #[derive(Debug, Clone)]
    struct TestWorkflow {
        id: WorkflowId,
        workflow_type: String,
        steps: Vec<WorkflowStep>,
    }

    impl Workflow for TestWorkflow {
        fn id(&self) -> WorkflowId {
            self.id
        }

        fn workflow_type(&self) -> &str {
            &self.workflow_type
        }

        fn steps(&self) -> &[WorkflowStep] {
            &self.steps
        }
    }

    fn create_test_workflow(workflow_type: &str) -> TestWorkflow {
        TestWorkflow {
            id: WorkflowId::new(),
            workflow_type: workflow_type.to_string(),
            steps: vec![
                WorkflowStep {
                    step_index: 0,
                    required_resource_type: "step1".to_string(),
                    expected_duration: Duration::seconds(10),
                    metadata: std::collections::HashMap::new(),
                },
                WorkflowStep {
                    step_index: 1,
                    required_resource_type: "step2".to_string(),
                    expected_duration: Duration::seconds(20),
                    metadata: std::collections::HashMap::new(),
                },
            ],
        }
    }

    #[test]
    fn test_registry_creation() {
        let registry = WorkflowRegistry::<TestWorkflow>::new();
        assert_eq!(registry.count(), 0);
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_register_single_engine() {
        let mut registry = WorkflowRegistry::new();
        let engine = WorkflowEngine::<TestWorkflow>::new();

        let result = registry.register("food_orders".to_string(), engine);
        assert!(result.is_ok());
        assert_eq!(registry.count(), 1);
        assert!(registry.contains("food_orders"));
    }

    #[test]
    fn test_register_multiple_engines() {
        let mut registry = WorkflowRegistry::<TestWorkflow>::new();

        registry
            .register("food_orders".to_string(), WorkflowEngine::new())
            .unwrap();
        registry
            .register("courier_delivery".to_string(), WorkflowEngine::new())
            .unwrap();
        registry
            .register("supply_chain".to_string(), WorkflowEngine::new())
            .unwrap();

        assert_eq!(registry.count(), 3);
        assert!(registry.contains("food_orders"));
        assert!(registry.contains("courier_delivery"));
        assert!(registry.contains("supply_chain"));
    }

    #[test]
    fn test_register_duplicate_name_fails() {
        let mut registry = WorkflowRegistry::<TestWorkflow>::new();

        registry
            .register("food_orders".to_string(), WorkflowEngine::new())
            .unwrap();

        let result = registry.register("food_orders".to_string(), WorkflowEngine::new());
        assert!(result.is_err());
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn test_get_engine_by_name() {
        let mut registry = WorkflowRegistry::new();
        let mut engine = WorkflowEngine::new();

        let workflow = create_test_workflow("food");
        let workflow_id = workflow.id();
        engine.register_workflow(workflow);

        registry
            .register("food_orders".to_string(), engine)
            .unwrap();

        let retrieved = registry.get("food_orders");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().workflow_count(), 1);

        // Verify we can access the workflow
        let engine_ref = retrieved.unwrap();
        assert!(engine_ref.get_workflow(workflow_id).is_some());
    }

    #[test]
    fn test_get_engine_not_found() {
        let registry = WorkflowRegistry::<TestWorkflow>::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_get_mut_engine() {
        let mut registry = WorkflowRegistry::new();
        registry
            .register("food_orders".to_string(), WorkflowEngine::new())
            .unwrap();

        // Get mutable reference and modify
        if let Some(engine) = registry.get_mut("food_orders") {
            let workflow = create_test_workflow("food");
            engine.register_workflow(workflow);
        }

        // Verify modification
        let engine = registry.get("food_orders").unwrap();
        assert_eq!(engine.workflow_count(), 1);
    }

    #[test]
    fn test_list_engines() {
        let mut registry = WorkflowRegistry::<TestWorkflow>::new();

        registry
            .register("courier_delivery".to_string(), WorkflowEngine::new())
            .unwrap();
        registry
            .register("food_orders".to_string(), WorkflowEngine::new())
            .unwrap();
        registry
            .register("supply_chain".to_string(), WorkflowEngine::new())
            .unwrap();

        let names = registry.list();
        assert_eq!(names.len(), 3);

        // Should be sorted alphabetically
        assert_eq!(names[0], "courier_delivery");
        assert_eq!(names[1], "food_orders");
        assert_eq!(names[2], "supply_chain");
    }

    #[test]
    fn test_remove_engine() {
        let mut registry = WorkflowRegistry::<TestWorkflow>::new();

        registry
            .register("food_orders".to_string(), WorkflowEngine::new())
            .unwrap();
        registry
            .register("courier_delivery".to_string(), WorkflowEngine::new())
            .unwrap();

        assert_eq!(registry.count(), 2);

        let removed = registry.remove("food_orders");
        assert!(removed.is_some());
        assert_eq!(registry.count(), 1);
        assert!(!registry.contains("food_orders"));
        assert!(registry.contains("courier_delivery"));
    }

    #[test]
    fn test_remove_nonexistent_engine() {
        let mut registry = WorkflowRegistry::<TestWorkflow>::new();
        let removed = registry.remove("nonexistent");
        assert!(removed.is_none());
    }

    #[test]
    fn test_tick_all_empty_registry() {
        let mut registry = WorkflowRegistry::<TestWorkflow>::new();
        let current_time = chrono::Utc::now();

        let events = registry.tick_all(current_time).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_tick_all_multiple_engines() {
        let mut registry = WorkflowRegistry::new();

        // Create three engines with workflows
        let mut food_engine = WorkflowEngine::new();
        let food_workflow = create_test_workflow("food");
        food_engine.register_workflow(food_workflow);

        let mut delivery_engine = WorkflowEngine::new();
        let delivery_workflow = create_test_workflow("delivery");
        delivery_engine.register_workflow(delivery_workflow);

        let mut supply_engine = WorkflowEngine::new();
        let supply_workflow = create_test_workflow("supply");
        supply_engine.register_workflow(supply_workflow);

        registry
            .register("food_orders".to_string(), food_engine)
            .unwrap();
        registry
            .register("courier_delivery".to_string(), delivery_engine)
            .unwrap();
        registry
            .register("supply_chain".to_string(), supply_engine)
            .unwrap();

        let current_time = chrono::Utc::now();
        let events = registry.tick_all(current_time).unwrap();

        // tick() currently returns empty events, but we're testing the mechanism works
        assert!(events.is_empty());
    }

    #[test]
    fn test_total_instance_count() {
        let mut registry = WorkflowRegistry::new();

        // Create engines with instances
        let mut food_engine = WorkflowEngine::new();
        let food_workflow = create_test_workflow("food");
        let food_id = food_workflow.id();
        food_engine.register_workflow(food_workflow);
        food_engine.start_instance(food_id).unwrap();
        food_engine.start_instance(food_id).unwrap();

        let mut delivery_engine = WorkflowEngine::new();
        let delivery_workflow = create_test_workflow("delivery");
        let delivery_id = delivery_workflow.id();
        delivery_engine.register_workflow(delivery_workflow);
        delivery_engine.start_instance(delivery_id).unwrap();
        delivery_engine.start_instance(delivery_id).unwrap();
        delivery_engine.start_instance(delivery_id).unwrap();

        registry
            .register("food_orders".to_string(), food_engine)
            .unwrap();
        registry
            .register("courier_delivery".to_string(), delivery_engine)
            .unwrap();

        assert_eq!(registry.total_instance_count(), 5);
    }

    #[test]
    fn test_instance_count_by_name() {
        let mut registry = WorkflowRegistry::new();

        let mut food_engine = WorkflowEngine::new();
        let food_workflow = create_test_workflow("food");
        let food_id = food_workflow.id();
        food_engine.register_workflow(food_workflow);
        food_engine.start_instance(food_id).unwrap();
        food_engine.start_instance(food_id).unwrap();

        registry
            .register("food_orders".to_string(), food_engine)
            .unwrap();

        assert_eq!(registry.instance_count("food_orders"), Some(2));
        assert_eq!(registry.instance_count("nonexistent"), None);
    }

    #[test]
    fn test_clear_registry() {
        let mut registry = WorkflowRegistry::<TestWorkflow>::new();

        registry
            .register("food_orders".to_string(), WorkflowEngine::new())
            .unwrap();
        registry
            .register("courier_delivery".to_string(), WorkflowEngine::new())
            .unwrap();

        assert_eq!(registry.count(), 2);

        registry.clear();

        assert_eq!(registry.count(), 0);
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_multiple_workflows_parallel_execution() {
        let mut registry = WorkflowRegistry::new();

        // Create food orders engine with 2 instances
        let mut food_engine = WorkflowEngine::new();
        let food_workflow = create_test_workflow("food");
        let food_id = food_workflow.id();
        food_engine.register_workflow(food_workflow);
        let food_instance1 = food_engine.start_instance(food_id).unwrap();
        let food_instance2 = food_engine.start_instance(food_id).unwrap();

        // Create delivery engine with 3 instances
        let mut delivery_engine = WorkflowEngine::new();
        let delivery_workflow = create_test_workflow("delivery");
        let delivery_id = delivery_workflow.id();
        delivery_engine.register_workflow(delivery_workflow);
        let delivery_instance1 = delivery_engine.start_instance(delivery_id).unwrap();
        let delivery_instance2 = delivery_engine.start_instance(delivery_id).unwrap();
        let delivery_instance3 = delivery_engine.start_instance(delivery_id).unwrap();

        registry
            .register("food_orders".to_string(), food_engine)
            .unwrap();
        registry
            .register("courier_delivery".to_string(), delivery_engine)
            .unwrap();

        // Verify both workflows are running in parallel
        assert_eq!(registry.instance_count("food_orders"), Some(2));
        assert_eq!(registry.instance_count("courier_delivery"), Some(3));
        assert_eq!(registry.total_instance_count(), 5);

        // Advance food order workflow
        let current_time = chrono::Utc::now();
        let food_engine = registry.get_mut("food_orders").unwrap();
        food_engine
            .advance_instance_step(food_instance1.id, current_time)
            .unwrap();

        // Advance delivery workflow
        let delivery_engine = registry.get_mut("courier_delivery").unwrap();
        delivery_engine
            .advance_instance_step(delivery_instance1.id, current_time)
            .unwrap();

        // Verify both workflows progressed independently
        let food_engine = registry.get("food_orders").unwrap();
        let food_inst = food_engine
            .instance_table
            .get_instance(food_instance1.id)
            .unwrap();
        assert_eq!(food_inst.current_step, 1);

        let delivery_engine = registry.get("courier_delivery").unwrap();
        let delivery_inst = delivery_engine
            .instance_table
            .get_instance(delivery_instance1.id)
            .unwrap();
        assert_eq!(delivery_inst.current_step, 1);

        // Other instances should remain at step 0
        let food_inst2 = food_engine
            .instance_table
            .get_instance(food_instance2.id)
            .unwrap();
        assert_eq!(food_inst2.current_step, 0);

        let delivery_inst2 = delivery_engine
            .instance_table
            .get_instance(delivery_instance2.id)
            .unwrap();
        assert_eq!(delivery_inst2.current_step, 0);

        let delivery_inst3 = delivery_engine
            .instance_table
            .get_instance(delivery_instance3.id)
            .unwrap();
        assert_eq!(delivery_inst3.current_step, 0);
    }

    #[test]
    fn test_default_implementation() {
        let registry = WorkflowRegistry::<TestWorkflow>::default();
        assert_eq!(registry.count(), 0);
    }
}
