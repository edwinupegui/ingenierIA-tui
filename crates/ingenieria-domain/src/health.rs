#![allow(dead_code)]
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct DocsStats {
    pub total: u32,
    pub by_factory: HashMap<String, u32>,
    pub by_type: HashMap<String, u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub version: String,
    pub sessions: u32,
    pub uptime_seconds: u64,
    pub docs: DocsStats,
}
