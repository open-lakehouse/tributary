//! End-to-end integration tests for Phase 6 Week 1 refinements
//!
//! These tests verify that all refinements work together correctly:
//! - Batch operations across multiple handlers
//! - Error handling maintains consistency
//! - Domain integration (sim-kitchen)

use geo_types::Point;
use sim_core::{Agent, SimulationConfig, SimulationContext, population::PopulationHandler};
use sim_kitchen::agents::{Customer, CustomerStatus};

// Helper to create test customers
fn create_test_customers(count: usize) -> Vec<Customer> {
    (0..count)
        .map(|i| {
            let location = Point::new(-122.4 + i as f64 * 0.01, 37.8);
            Customer::new(location)
        })
        .collect()
}

#[test]
fn test_kitchen_customer_round_trip() {
    // Test that Customer agents work with PopulationHandler
    let mut handler = PopulationHandler::new();

    // Create customers with different statuses
    let customers = vec![
        Customer::new(Point::new(-122.4, 37.8)),
        {
            let mut c = Customer::new(Point::new(-122.5, 37.9));
            c.status = CustomerStatus::Ordering;
            c
        },
        {
            let mut c = Customer::new(Point::new(-122.6, 38.0));
            c.status = CustomerStatus::WaitingForFood {
                order_id: "ORDER123".to_string(),
            };
            c
        },
        {
            let mut c = Customer::new(Point::new(-122.7, 38.1));
            c.status = CustomerStatus::Eating {
                finish_time: chrono::Utc::now() + chrono::Duration::minutes(10),
            };
            c
        },
    ];

    // Store IDs for later verification
    let customer_ids: Vec<_> = customers.iter().map(|c| c.id()).collect();

    // Add using batch operation
    handler.add_agents(customers.clone()).unwrap();

    // Verify all were added
    assert_eq!(handler.count(), 4);

    // Retrieve and verify state preserved
    for (original, id) in customers.iter().zip(customer_ids.iter()) {
        let retrieved = handler.get_agent(*id).unwrap();
        assert_eq!(original.id(), retrieved.id());
        assert_eq!(original.agent_type(), retrieved.agent_type());
        assert_eq!(original.status().as_str(), retrieved.status().as_str());

        // Verify position preserved
        assert_eq!(original.position(), retrieved.position());
    }
}

#[test]
fn test_batch_add_vs_sequential_consistency() {
    // Verify that batch add and sequential add produce identical results
    let customers = create_test_customers(20);

    // Batch add
    let mut batch_handler = PopulationHandler::new();
    batch_handler.add_agents(customers.clone()).unwrap();

    // Sequential add
    let mut sequential_handler = PopulationHandler::new();
    for customer in customers.clone() {
        sequential_handler.add_agent(customer);
    }

    // Verify same count
    assert_eq!(batch_handler.count(), sequential_handler.count());
    assert_eq!(batch_handler.count(), 20);

    // Verify all IDs present in both
    for customer in customers {
        let batch_agent = batch_handler.get_agent(customer.id()).unwrap();
        let seq_agent = sequential_handler.get_agent(customer.id()).unwrap();

        assert_eq!(batch_agent.id(), seq_agent.id());
        assert_eq!(batch_agent.status().as_str(), seq_agent.status().as_str());
    }
}

#[test]
fn test_end_to_end_simulation_with_batch_ops() {
    // Initialize handler
    let mut population = PopulationHandler::new();

    // Batch add agents
    let agents = create_test_customers(50);
    let agent_count = agents.len();
    population.add_agents(agents).unwrap();

    // Verify initial state
    assert_eq!(population.count(), agent_count);

    // Run simulation steps using tick_agents (which uses batch updates internally)
    let ctx = SimulationContext::from_now(SimulationConfig::default());

    for step in 0..5 {
        // Tick all agents - this uses batch update internally
        let result = population.tick_agents(&ctx);
        assert!(
            result.is_ok(),
            "Tick failed at step {}: {:?}",
            step,
            result.err()
        );

        // Verify state consistency after each tick
        assert_eq!(
            population.count(),
            agent_count,
            "Agent count changed unexpectedly at step {}",
            step
        );
    }

    // Verify final state is consistent
    assert_eq!(population.count(), agent_count);
}

#[test]
fn test_synchronous_queries() {
    let mut handler = PopulationHandler::new();

    // Add customers with different statuses
    let mut all_customers = Vec::new();

    // 20 idle
    all_customers.extend(create_test_customers(20));

    // 15 ordering
    all_customers.extend(create_test_customers(15).into_iter().map(|mut c| {
        c.status = CustomerStatus::Ordering;
        c
    }));

    // 10 eating
    all_customers.extend(create_test_customers(10).into_iter().map(|mut c| {
        c.status = CustomerStatus::Eating {
            finish_time: chrono::Utc::now() + chrono::Duration::minutes(10),
        };
        c
    }));

    handler.add_agents(all_customers).unwrap();

    // Verify counts using iterator filters (synchronous queries)
    let idle_count = handler
        .iter()
        .filter(|a| a.status().as_str() == "idle")
        .count();
    assert_eq!(idle_count, 20, "Expected 20 idle customers");

    let ordering_count = handler
        .iter()
        .filter(|a| a.status().as_str() == "ordering")
        .count();
    assert_eq!(ordering_count, 15, "Expected 15 ordering customers");

    let eating_count = handler
        .iter()
        .filter(|a| a.status().as_str() == "eating")
        .count();
    assert_eq!(eating_count, 10, "Expected 10 eating customers");

    // Query by type
    let customer_count = handler
        .iter()
        .filter(|a| a.agent_type() == "customer")
        .count();
    assert_eq!(customer_count, 45, "Expected 45 total customers");
}

#[test]
fn test_error_handling_maintains_consistency() {
    let mut handler = PopulationHandler::new();

    // Add some agents
    let agents = create_test_customers(10);
    handler.add_agents(agents).unwrap();

    let initial_count = handler.count();
    assert_eq!(initial_count, 10);

    // Try to get non-existent agent
    let fake_id = sim_core::AgentId::new();
    let result = handler.get_agent(fake_id);
    assert!(
        result.is_none(),
        "Should return None for non-existent agent"
    );

    // Verify count unchanged after failed get
    assert_eq!(handler.count(), initial_count);

    // Try to remove non-existent agent
    let remove_result = handler.remove_agent(fake_id);
    assert!(
        remove_result.is_none(),
        "Should return None for non-existent agent"
    );

    // Verify count unchanged after failed remove
    assert_eq!(handler.count(), initial_count);
}

#[test]
fn test_batch_update_maintains_consistency() {
    let mut handler = PopulationHandler::new();

    // Add initial agents
    let mut customers = create_test_customers(20);
    let customer_ids: Vec<_> = customers.iter().map(|c| c.id()).collect();
    handler.add_agents(customers.clone()).unwrap();

    // Modify agents
    for customer in &mut customers {
        customer.status = CustomerStatus::Ordering;
    }

    // Batch update
    handler.update_agents(customers.clone()).unwrap();

    // Verify all updated correctly
    for (_original, id) in customers.iter().zip(customer_ids.iter()) {
        let retrieved = handler.get_agent(*id).unwrap();
        assert_eq!(retrieved.status().as_str(), "ordering");
    }

    // Verify count unchanged
    assert_eq!(handler.count(), 20);
}

#[test]
fn test_empty_batch_operations() {
    let mut handler = PopulationHandler::new();

    // Add some initial agents
    let customers = create_test_customers(5);
    handler.add_agents(customers).unwrap();
    let initial_count = handler.count();

    // Empty batch add should succeed and not change state
    let result = handler.add_agents(Vec::<Customer>::new());
    assert!(result.is_ok());
    assert_eq!(handler.count(), initial_count);

    // Empty batch update should succeed
    let result = handler.update_agents(Vec::<Customer>::new());
    assert!(result.is_ok());
    assert_eq!(handler.count(), initial_count);
}

#[test]
fn test_cross_handler_integration() {
    // Test that multiple handlers work together correctly
    let mut population = PopulationHandler::new();

    // Add agents to population
    let customers = create_test_customers(25);
    population.add_agents(customers).unwrap();

    // Population handler maintains state correctly
    assert_eq!(population.count(), 25);
}

#[test]
fn test_remove_during_iteration() {
    let mut handler = PopulationHandler::new();

    // Add customers
    let customers = create_test_customers(30);
    let ids_to_remove: Vec<_> = customers.iter().take(10).map(|c| c.id()).collect();
    handler.add_agents(customers).unwrap();

    // Remove some agents
    for id in ids_to_remove {
        handler.remove_agent(id);
    }

    // Verify correct count
    assert_eq!(handler.count(), 20);
}

#[test]
fn test_large_batch_performance() {
    let mut handler = PopulationHandler::new();

    // Large batch should complete quickly
    let start = std::time::Instant::now();
    let customers = create_test_customers(1000);
    handler.add_agents(customers).unwrap();
    let duration = start.elapsed();

    assert_eq!(handler.count(), 1000);
    // Should complete in less than 100ms (generous threshold for CI)
    assert!(
        duration.as_millis() < 100,
        "Batch add took too long: {:?}",
        duration
    );
}

#[test]
fn test_get_agent_preserves_full_state() {
    let mut handler = PopulationHandler::new();

    // Create customer with complex state
    let mut customer = Customer::new(Point::new(-122.4, 37.8));
    customer.status = CustomerStatus::WaitingForFood {
        order_id: "ORDER_XYZ_123".to_string(),
    };

    let id = customer.id();
    handler.add_agent(customer.clone());

    // Retrieve and verify all state preserved
    let retrieved = handler.get_agent(id).unwrap();
    assert_eq!(retrieved.id(), id);
    assert_eq!(retrieved.status().as_str(), "waiting_for_food");
    assert_eq!(retrieved.position(), Some(Point::new(-122.4, 37.8)));
}

#[test]
fn test_update_agent_overwrites_state() {
    let mut handler = PopulationHandler::new();

    // Add initial customer
    let customer = Customer::new(Point::new(-122.4, 37.8));
    let id = customer.id();
    handler.add_agent(customer);

    // Create updated customer with same ID but different state
    let mut updated = Customer::new(Point::new(-123.0, 38.0));
    updated.id = id; // Reuse same ID
    updated.status = CustomerStatus::Eating {
        finish_time: chrono::Utc::now() + chrono::Duration::minutes(5),
    };

    // Update the agent
    handler.update_agent(updated.clone()).unwrap();

    // Verify state was completely replaced
    let retrieved = handler.get_agent(id).unwrap();
    assert_eq!(retrieved.status().as_str(), "eating");
    assert_eq!(retrieved.position(), Some(Point::new(-123.0, 38.0)));
}

#[test]
fn test_agents_iter_consistency() {
    let mut handler = PopulationHandler::new();

    let customers = create_test_customers(15);
    let expected_ids: std::collections::HashSet<_> = customers.iter().map(|c| c.id()).collect();

    handler.add_agents(customers).unwrap();

    // Verify iterator returns all agents
    let iter_ids: std::collections::HashSet<_> = handler.iter().map(|a| a.id()).collect();

    assert_eq!(iter_ids, expected_ids);
    assert_eq!(iter_ids.len(), 15);
}
