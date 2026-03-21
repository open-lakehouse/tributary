//! Courier profile types.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CourierProfile {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: String,
    pub vehicle_type: String,
    pub license_plate: String,
    pub driver_rating: f32,
    pub certifications: Vec<String>,
    pub total_deliveries: u32,
}
