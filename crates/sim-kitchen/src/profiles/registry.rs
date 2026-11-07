//! ProfileRegistry — deterministic profile storage keyed by AgentId.

use sim_core::AgentId;
use std::collections::HashMap;

use super::CustomerProfile;
use super::customer::{CardType, LoyaltyTier, PaymentProfile, PostalAddress};
use chrono::{DateTime, NaiveDate};

/// Stores and lazily generates deterministic profiles for simulation agents.
pub struct ProfileRegistry {
    customers: HashMap<AgentId, CustomerProfile>,
}

impl ProfileRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            customers: HashMap::new(),
        }
    }

    /// Get or create a customer profile for the given agent ID.
    /// Profile is deterministically generated from the agent ID.
    pub fn customer_profile(&mut self, id: AgentId) -> &CustomerProfile {
        self.customers
            .entry(id)
            .or_insert_with(|| generate_customer(id))
    }

    /// Deterministically derive a 32-byte seed from an AgentId.
    pub fn derive_seed(id: AgentId) -> [u8; 32] {
        let bytes = id.as_bytes();
        let mut seed = [0u8; 32];
        seed[..16].copy_from_slice(bytes);
        seed[16..].copy_from_slice(bytes);
        seed
    }
}

impl Default for ProfileRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---- Deterministic generation helpers ----

fn seed_u8(seed: &[u8; 32], index: usize) -> u8 {
    seed[index % 32]
}

fn pick<'a, T>(seed: &[u8; 32], index: usize, choices: &'a [T]) -> &'a T {
    &choices[seed_u8(seed, index) as usize % choices.len()]
}

fn generate_customer(id: AgentId) -> CustomerProfile {
    let seed = ProfileRegistry::derive_seed(id);

    let first_names = [
        "Alice", "Bob", "Carol", "David", "Emma", "Frank", "Grace", "Henry",
    ];
    let last_names = [
        "Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller",
    ];
    let cities = [
        "San Francisco",
        "Oakland",
        "San Jose",
        "Berkeley",
        "Fremont",
    ];
    let card_types = [
        CardType::Visa,
        CardType::Mastercard,
        CardType::Amex,
        CardType::Discover,
    ];
    let tiers = [
        LoyaltyTier::Bronze,
        LoyaltyTier::Silver,
        LoyaltyTier::Gold,
        LoyaltyTier::Platinum,
    ];

    let first = pick(&seed, 0, &first_names).to_string();
    let last = pick(&seed, 1, &last_names).to_string();
    let city = pick(&seed, 2, &cities).to_string();

    let address = PostalAddress {
        street: format!("{} Main St", seed_u8(&seed, 3) as u32 * 3 + 100),
        city: city.clone(),
        state: "CA".to_string(),
        postal_code: format!("9{:04}", seed_u8(&seed, 4) as u32 % 10000),
        country_code: "US".to_string(),
        lat: 37.0 + (seed_u8(&seed, 5) as f64 / 256.0),
        lon: -122.0 - (seed_u8(&seed, 6) as f64 / 256.0),
    };

    let payment = PaymentProfile {
        card_last_four: format!("{:04}", seed_u8(&seed, 7) as u32 * 100 % 10000),
        card_type: pick(&seed, 8, &card_types).clone(),
        expiry_month: seed_u8(&seed, 9) % 12 + 1,
        expiry_year: 2026 + (seed_u8(&seed, 10) % 5) as u16,
        billing_address: address.clone(),
    };

    CustomerProfile {
        first_name: first.clone(),
        last_name: last.clone(),
        email: format!(
            "{}.{}@example.com",
            pick(&seed, 0, &first_names).to_lowercase(),
            pick(&seed, 1, &last_names).to_lowercase()
        ),
        phone: format!("+1415{:07}", seed_u8(&seed, 11) as u32 * 100000 % 10000000),
        date_of_birth: NaiveDate::from_ymd_opt(
            1970 + (seed_u8(&seed, 12) % 40) as i32,
            (seed_u8(&seed, 13) % 12 + 1) as u32,
            (seed_u8(&seed, 14) % 28 + 1) as u32,
        )
        .unwrap_or(NaiveDate::from_ymd_opt(1990, 1, 1).unwrap()),
        address,
        payment,
        dietary_restrictions: vec![],
        favorite_items: vec![],
        order_history_count: seed_u8(&seed, 15) as u32,
        // Derived from the profile seed (not the wall clock) so profiles are
        // reproducible: some time in the ~3 years before 2024-01-01.
        account_created_at: DateTime::from_timestamp(
            1_704_067_200
                - i64::from(u16::from_le_bytes([seed_u8(&seed, 18), seed_u8(&seed, 19)])) * 1_440,
            0,
        )
        .expect("timestamp in range"),
        loyalty_points: seed_u8(&seed, 16) as u32 * 100,
        loyalty_tier: pick(&seed, 17, &tiers).clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new() {
        let _registry = ProfileRegistry::new();
    }

    #[test]
    fn test_customer_profile_deterministic() {
        let id = AgentId::new();
        let mut registry = ProfileRegistry::new();
        let profile1 = registry.customer_profile(id).clone();
        let profile2 = registry.customer_profile(id).clone();
        assert_eq!(profile1.first_name, profile2.first_name);
        assert_eq!(profile1.last_name, profile2.last_name);
        assert_eq!(profile1.email, profile2.email);
    }

    #[test]
    fn test_different_ids_different_seeds() {
        let id1 = AgentId::new();
        let id2 = AgentId::new();
        // Different AgentIds must produce different seeds (UUID bytes are unique)
        let seed1 = ProfileRegistry::derive_seed(id1);
        let seed2 = ProfileRegistry::derive_seed(id2);
        assert_ne!(seed1, seed2);
    }

    #[test]
    fn test_derive_seed() {
        let id = AgentId::new();
        let seed = ProfileRegistry::derive_seed(id);
        assert_eq!(seed.len(), 32);
        // First 16 bytes == last 16 bytes
        assert_eq!(&seed[..16], &seed[16..]);
    }
}
