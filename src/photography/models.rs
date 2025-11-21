use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkaterRow {
    pub first_name: String,
    pub last_name: String,
    pub comp_name: Option<String>,
    pub event_num: Option<i32>,
    pub split_ice: Option<String>,
    pub time_slot: Option<String>,
    pub req_status: Option<String>,
    pub gal_status: Option<String>,
    pub sent_date: Option<String>,
    pub purchase_amount: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusRow {
    pub family_name: String,
    pub email: Option<String>,
    pub request_status: Option<String>,
    pub gallery_status: Option<String>,
    pub sent_date: Option<String>,
    pub ty_requested: Option<bool>,
    pub ty_sent: Option<bool>,
    pub ty_sent_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Family {
    pub id: Thing,
    pub last_name: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RosterRow {
    #[serde(rename = "Time")]
    pub time: Option<String>,
    #[serde(rename = "Event")]
    pub event: u32,
    #[serde(rename = "Split Ice")]
    pub split_ice: Option<String>,
    #[serde(rename = "Skate Order")]
    pub skate_order: Option<u32>,
    #[serde(rename = "Skater Name")]
    pub skater_name: String,
    #[serde(rename = "SignUp")]
    pub signup: Option<String>,
    #[serde(rename = "Email")]
    pub email: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedSkater {
    pub first_name: String,
    pub last_name: String,
    pub _family_email: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedName {
    pub skaters: Vec<ParsedSkater>,
    pub is_family: bool,
    pub _is_synchro: bool,
}
