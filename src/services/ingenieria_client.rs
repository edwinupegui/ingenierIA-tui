use std::sync::RwLock;

use crate::domain::{
    document::{DocumentDetail, DocumentSummary},
    health::HealthStatus,
    search::SearchResponse,
};
use reqwest::Client;

const HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Error de red: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Documento no encontrado: {uri}")]
    NotFound { uri: String },

    #[expect(dead_code, reason = "error variant reserved for future HTTP status handling")]
    #[error("Servidor respondió HTTP {status}")]
    Server { status: u16 },
}

type Result<T> = std::result::Result<T, ApiError>;

pub struct IngenieriaClient {
    base_url: RwLock<String>,
    client: Client,
}

impl Clone for IngenieriaClient {
    fn clone(&self) -> Self {
        Self { base_url: RwLock::new(self.url()), client: self.client.clone() }
    }
}

impl IngenieriaClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let client = Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .expect("default TLS config cannot fail");
        Self { base_url: RwLock::new(base_url), client }
    }

    fn url(&self) -> String {
        self.base_url.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn base_url(&self) -> String {
        self.url()
    }

    /// Update the base URL at runtime (e.g., after wizard completion).
    pub fn set_base_url(&self, url: &str) {
        let url = url.trim_end_matches('/').to_string();
        if let Ok(mut guard) = self.base_url.write() {
            *guard = url;
        }
    }

    pub fn events_url(&self) -> String {
        format!("{}/api/events", self.url())
    }

    pub async fn health(&self) -> Result<HealthStatus> {
        let url = format!("{}/api/health", self.url());
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json::<HealthStatus>().await?)
    }

    /// GET /api/documents?factory=net&type=skill
    pub async fn documents(
        &self,
        factory: Option<&str>,
        doc_type: Option<&str>,
    ) -> Result<Vec<DocumentSummary>> {
        let mut params: Vec<(&str, &str)> = Vec::new();
        if let Some(f) = factory {
            params.push(("factory", f));
        }
        if let Some(t) = doc_type {
            params.push(("type", t));
        }

        let resp = self
            .client
            .get(format!("{}/api/documents", self.url()))
            .query(&params)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json::<Vec<DocumentSummary>>().await?)
    }

    /// GET /api/documents/:type/:factory/:name
    pub async fn document(
        &self,
        doc_type: &str,
        factory: &str,
        name: &str,
    ) -> Result<DocumentDetail> {
        let url = format!("{}/api/documents/{doc_type}/{factory}/{name}", self.url());
        let resp = self.client.get(&url).send().await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ApiError::NotFound {
                uri: format!("ingenieria://{doc_type}/{factory}/{name}"),
            });
        }
        let resp = resp.error_for_status()?;
        Ok(resp.json::<DocumentDetail>().await?)
    }

    /// GET /api/search?q=...&factory=net
    pub async fn search(&self, q: &str, factory: Option<&str>) -> Result<SearchResponse> {
        let mut params: Vec<(&str, &str)> = vec![("q", q)];
        if let Some(f) = factory {
            params.push(("factory", f));
        }

        let resp = self
            .client
            .get(format!("{}/api/search", self.url()))
            .query(&params)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json::<SearchResponse>().await?)
    }
}
