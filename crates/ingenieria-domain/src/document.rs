#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DocumentSummary {
    pub uri: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub factory: String,
    pub name: String,
    pub description: String,
    pub last_modified: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DocumentDetail {
    pub uri: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub factory: String,
    pub name: String,
    pub description: String,
    pub content: String,
    pub last_modified: String,
}
