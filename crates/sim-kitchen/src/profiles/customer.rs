//! Customer profile types.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PostalAddress {
    pub street: String,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub country_code: String,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaymentProfile {
    pub card_last_four: String,
    pub card_type: CardType,
    pub expiry_month: u8,
    pub expiry_year: u16,
    pub billing_address: PostalAddress,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CardType {
    Visa,
    Mastercard,
    Amex,
    Discover,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LoyaltyTier {
    Bronze,
    Silver,
    Gold,
    Platinum,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CustomerProfile {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: String, // E.164 format
    pub date_of_birth: chrono::NaiveDate,
    pub address: PostalAddress,
    pub payment: PaymentProfile,
    pub dietary_restrictions: Vec<String>,
    pub favorite_items: Vec<String>,
    pub order_history_count: u32,
    pub account_created_at: chrono::DateTime<chrono::Utc>,
    pub loyalty_points: u32,
    pub loyalty_tier: LoyaltyTier,
}
