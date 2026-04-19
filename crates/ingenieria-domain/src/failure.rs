//! Failure taxonomy (E13) — reemplaza strings opacos por fallos estructurados.
//!
//! Cada fallo lleva: categoria (14 tipos), severity, mensaje humano,
//! opcional recovery hint. Las categorias mapean a colores de toast y
//! guian acciones de recovery.
//!
//! Referencias: CLAW `rust/crates/runtime/src/recovery_recipes.rs` (631 LOC)
//! `FailureScenario`, `RecoveryStep`, `EscalationPolicy`.

use serde::{Deserialize, Serialize};

/// Severidad — mapea a `ToastLevel` y color del borde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Transitorio, el retry ya esta programado.
    Info,
    /// Usuario puede seguir pero debe reaccionar.
    Warning,
    /// Bloquea la operacion actual, action requerida.
    Error,
    /// Fallo critico, sesion comprometida.
    Critical,
}

/// 14 categorias de fallo que cubren el dominio del TUI + MCP + provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    // ── Provider / API ─────────────────────────────────────────────
    /// API rate limit (429) o server error (5xx) al entregar el prompt.
    PromptDelivery,
    /// El modelo se nego a responder (content filter, policy).
    ModelRefusal,
    /// El modelo genero input invalido para un tool.
    InvalidToolInput,
    /// API key faltante o expirada.
    ApiKeyInvalid,
    /// Timeout del stream sin deltas.
    StreamTimeout,
    // ── Permission / Trust ─────────────────────────────────────────
    /// Usuario denego el permiso para un tool.
    TrustDenied,
    /// Limite de costos/tokens alcanzado.
    BudgetExceeded,
    // ── MCP / Tools ────────────────────────────────────────────────
    /// Servidor MCP no respondio.
    McpTimeout,
    /// Servidor MCP devolvio error.
    McpError,
    /// Tool no disponible o no registrado.
    ToolNotFound,
    // ── Sistema ────────────────────────────────────────────────────
    /// Error al compilar (bash/cargo/etc.).
    CompileError,
    /// Fallo de I/O (fs, network no-http).
    IoError,
    /// Parseo de datos fallo (JSON, schema).
    ParseError,
    /// Categoria catch-all cuando no se puede inferir.
    Unknown,
}

impl FailureCategory {
    /// Severity default para la categoria.
    pub fn default_severity(&self) -> Severity {
        match self {
            Self::PromptDelivery | Self::StreamTimeout | Self::McpTimeout => Severity::Warning,
            Self::TrustDenied | Self::BudgetExceeded => Severity::Warning,
            Self::ModelRefusal | Self::InvalidToolInput | Self::ToolNotFound => Severity::Error,
            Self::McpError | Self::CompileError | Self::IoError | Self::ParseError => {
                Severity::Error
            }
            Self::ApiKeyInvalid => Severity::Critical,
            Self::Unknown => Severity::Error,
        }
    }

    /// Recovery hint humano. None = sin sugerencia automatica.
    pub fn recovery_hint(&self) -> Option<&'static str> {
        match self {
            Self::PromptDelivery => Some("Reintentar en 30s o cambiar de modelo"),
            Self::ModelRefusal => Some("Reformular el prompt con mas contexto"),
            Self::InvalidToolInput => Some("Informar al AI del schema correcto"),
            Self::TrustDenied => Some("Revisar /permissions si es intencional"),
            Self::BudgetExceeded => Some("Usar /costs --limit o /compact"),
            Self::McpTimeout => Some("Ejecutar /doctor para verificar servers"),
            Self::McpError => Some("Ver logs MCP o /doctor"),
            Self::ToolNotFound => Some("Verificar con /features y /skills"),
            Self::ApiKeyInvalid => Some("Re-autenticar via /config"),
            Self::StreamTimeout => Some("Reintentar la solicitud"),
            Self::CompileError => Some("Revisar el output y corregir"),
            Self::IoError => Some("Verificar permisos y disponibilidad"),
            Self::ParseError => Some("Revisar formato de datos entrante"),
            Self::Unknown => None,
        }
    }

    /// Nombre corto para logs/UI.
    pub fn label(&self) -> &'static str {
        match self {
            Self::PromptDelivery => "Prompt delivery",
            Self::ModelRefusal => "Model refusal",
            Self::InvalidToolInput => "Invalid tool input",
            Self::TrustDenied => "Trust denied",
            Self::BudgetExceeded => "Budget exceeded",
            Self::McpTimeout => "MCP timeout",
            Self::McpError => "MCP error",
            Self::ToolNotFound => "Tool not found",
            Self::ApiKeyInvalid => "API key invalid",
            Self::StreamTimeout => "Stream timeout",
            Self::CompileError => "Compile error",
            Self::IoError => "I/O error",
            Self::ParseError => "Parse error",
            Self::Unknown => "Unknown failure",
        }
    }
}

/// Fallo estructurado — reemplaza `ChatStreamError(String)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredFailure {
    pub category: FailureCategory,
    pub severity: Severity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_hint: Option<String>,
    /// HTTP status code si aplica (429, 500, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    pub timestamp: String,
}

impl StructuredFailure {
    /// Construye un `StructuredFailure` desde un mensaje de error en string.
    /// Usa heuristicas para categorizar (status codes, keywords).
    pub fn from_error(message: impl Into<String>) -> Self {
        let message = message.into();
        let status_code = extract_status_code(&message);
        let category = categorize(&message, status_code);
        let severity = category.default_severity();
        let recovery_hint = category.recovery_hint().map(|s| s.to_string());
        Self {
            category,
            severity,
            message,
            recovery_hint,
            status_code,
            timestamp: crate::time::now_iso(),
        }
    }

    /// Construye un fallo con categoria explicita.
    pub fn new(category: FailureCategory, message: impl Into<String>) -> Self {
        let severity = category.default_severity();
        let recovery_hint = category.recovery_hint().map(|s| s.to_string());
        Self {
            category,
            severity,
            message: message.into(),
            recovery_hint,
            status_code: None,
            timestamp: crate::time::now_iso(),
        }
    }

    /// Render humano: `[categoria] mensaje · hint`.
    pub fn display(&self) -> String {
        let mut out = format!("[{}] {}", self.category.label(), self.message);
        if let Some(hint) = &self.recovery_hint {
            out.push_str(" · ");
            out.push_str(hint);
        }
        out
    }
}

/// Extrae un status code HTTP (100-599) rodeado de boundaries no-numericos.
fn extract_status_code(error: &str) -> Option<u16> {
    let bytes = error.as_bytes();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let is_left = i == 0 || !bytes[i - 1].is_ascii_digit();
        let is_right = i + 3 == bytes.len() || !bytes[i + 3].is_ascii_digit();
        if is_left
            && is_right
            && bytes[i].is_ascii_digit()
            && bytes[i + 1].is_ascii_digit()
            && bytes[i + 2].is_ascii_digit()
        {
            if let Ok(s) = std::str::from_utf8(&bytes[i..i + 3]) {
                if let Ok(code) = s.parse::<u16>() {
                    if (100..=599).contains(&code) {
                        return Some(code);
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn categorize(message: &str, status_code: Option<u16>) -> FailureCategory {
    let lower = message.to_lowercase();

    // Status codes primero
    if let Some(code) = status_code {
        match code {
            401 | 403 => return FailureCategory::ApiKeyInvalid,
            404 => {
                if lower.contains("tool") || lower.contains("mcp") {
                    return FailureCategory::ToolNotFound;
                }
            }
            408 | 504 => return FailureCategory::StreamTimeout,
            429 | 529 => return FailureCategory::PromptDelivery,
            500..=503 => return FailureCategory::PromptDelivery,
            _ => {}
        }
    }

    // Keywords
    if lower.contains("timeout") || lower.contains("timed out") {
        if lower.contains("mcp") {
            return FailureCategory::McpTimeout;
        }
        return FailureCategory::StreamTimeout;
    }
    if lower.contains("api key") || lower.contains("unauthorized") || lower.contains("auth") {
        return FailureCategory::ApiKeyInvalid;
    }
    if lower.contains("rate limit") || lower.contains("too many requests") {
        return FailureCategory::PromptDelivery;
    }
    if lower.contains("refused") || lower.contains("blocked by policy") {
        return FailureCategory::ModelRefusal;
    }
    if lower.contains("invalid") && lower.contains("tool") {
        return FailureCategory::InvalidToolInput;
    }
    if lower.contains("denied") || lower.contains("permission") {
        return FailureCategory::TrustDenied;
    }
    if lower.contains("budget") || lower.contains("limit exceeded") {
        return FailureCategory::BudgetExceeded;
    }
    if lower.contains("mcp") {
        return FailureCategory::McpError;
    }
    if lower.contains("parse") || lower.contains("deserialize") || lower.contains("json") {
        return FailureCategory::ParseError;
    }
    if lower.contains("compile") || lower.contains("cargo") {
        return FailureCategory::CompileError;
    }
    if lower.contains("io error")
        || lower.contains("not found")
        || lower.contains("no such")
        || lower.contains("error sending request")
        || lower.contains("connection refused")
        || lower.contains("connection reset")
    {
        return FailureCategory::IoError;
    }

    FailureCategory::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_code_429_is_prompt_delivery() {
        let f = StructuredFailure::from_error("429 Too Many Requests");
        assert_eq!(f.category, FailureCategory::PromptDelivery);
        assert_eq!(f.status_code, Some(429));
    }

    #[test]
    fn status_code_401_is_api_key_invalid() {
        let f = StructuredFailure::from_error("401 unauthorized");
        assert_eq!(f.category, FailureCategory::ApiKeyInvalid);
        assert_eq!(f.severity, Severity::Critical);
    }

    #[test]
    fn timeout_keyword_categorizes() {
        let f = StructuredFailure::from_error("request timed out after 60s");
        assert_eq!(f.category, FailureCategory::StreamTimeout);
    }

    #[test]
    fn mcp_timeout_specific() {
        let f = StructuredFailure::from_error("MCP server timeout: ingenieria-net");
        assert_eq!(f.category, FailureCategory::McpTimeout);
    }

    #[test]
    fn unknown_when_no_signal() {
        let f = StructuredFailure::from_error("some weird thing happened");
        assert_eq!(f.category, FailureCategory::Unknown);
    }

    #[test]
    fn recovery_hint_present_for_known_categories() {
        let f = StructuredFailure::new(FailureCategory::BudgetExceeded, "limit hit");
        assert!(f.recovery_hint.is_some());
        assert!(f.display().contains("/costs"));
    }

    // NOTE: severity_maps_to_toast_level test moved to src/domain/failure.rs
    // (depends on state::ToastLevel, which lives in the main crate).

    #[test]
    fn extract_status_avoids_interior_digits() {
        // No debe matchear "1234" como 123
        assert_eq!(extract_status_code("code=1234 err"), None);
        assert_eq!(extract_status_code("got 500 response"), Some(500));
    }

    #[test]
    fn display_includes_category_and_hint() {
        let f = StructuredFailure::new(FailureCategory::ApiKeyInvalid, "token expired");
        let s = f.display();
        assert!(s.contains("API key invalid"));
        assert!(s.contains("token expired"));
        assert!(s.contains("/config"));
    }
}
