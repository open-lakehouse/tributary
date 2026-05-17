#[cfg(test)]
mod tests {
    use crate::LogisticsSimulation;

    #[test]
    fn packages_delivered_never_exceeds_created() {
        let mut sim = LogisticsSimulation::new();
        sim.run_for_hours(2).unwrap();
        let m = sim.get_metrics();
        assert!(
            m.packages_delivered <= m.packages_created,
            "delivered {} > created {}",
            m.packages_delivered,
            m.packages_created
        );
    }

    #[test]
    fn demand_generator_creates_packages_autonomously() {
        let mut sim = LogisticsSimulation::new();
        for _ in 0..120 {
            sim.step().unwrap();
        }
        let m = sim.get_metrics();
        assert!(
            m.packages_created > 0,
            "demand generator should create packages autonomously"
        );
    }

    #[test]
    fn driver_utilization_in_valid_range() {
        let mut sim = LogisticsSimulation::new();
        sim.run_for_hours(1).unwrap();
        let m = sim.get_metrics();
        assert!(
            m.driver_utilization_percent >= 0.0 && m.driver_utilization_percent <= 100.0,
            "utilization {} out of [0, 100]",
            m.driver_utilization_percent
        );
    }

    #[test]
    fn delivery_time_positive_after_deliveries() {
        let mut sim = LogisticsSimulation::new();
        sim.run_for_hours(6).unwrap();
        let m = sim.get_metrics();
        if m.packages_delivered > 0 {
            assert!(
                m.average_delivery_time_secs > 0.0,
                "delivery time should be positive when packages delivered"
            );
        }
    }
}
