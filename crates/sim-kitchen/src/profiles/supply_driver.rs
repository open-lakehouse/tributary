//! Supply driver profile types.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SupplyDriverProfile {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: String,
    pub company_name: String,
    pub cdl_license_class: String,
    pub route_ids: Vec<String>,
    pub years_experience: u8,
}
