use serde::*;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DathostServerDuplicateResponse {
    pub game: Option<String>,
    pub id: String,
    pub ip: String,
    pub ports: Ports,
    pub location: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ports {
    pub game: i64,
    pub gotv: i64,
}