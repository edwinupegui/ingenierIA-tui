#![allow(dead_code)]
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub total: u32,
    pub results: Vec<SearchResultItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResultItem {
    pub uri: String,
    pub name: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub factory: String,
    pub description: String,
    pub matches: Vec<MatchLine>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MatchLine {
    pub line: u32,
    pub text: String,
}
