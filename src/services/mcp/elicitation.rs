//! MCP Elicitation (E18): tipos de dominio + bridge hacia el App.
//!
//! Modelo simplificado de `elicitation/create` del protocolo MCP. Cada request
//! describe un unico `ElicitationField` (Text, Select, Confirm, MultiSelect)
//! con un mensaje introductorio. El cliente MCP (o cualquier componente que
//! quiera pedir input al usuario) envia un `Action::ElicitationRequested` al
//! app_tx y espera la respuesta via oneshot channel.
//!
//! La UI es responsable de:
//! - Guardar el request + sender en `AppState` como `PendingElicitation`.
//! - Mostrar el modal con los key-bindings correspondientes.
//! - Invocar `.respond()` cuando el usuario acepte/decline/cancele.

use tokio::sync::{mpsc::Sender, oneshot};

use crate::actions::Action;

/// Forma del campo que se le pide al usuario.
#[allow(dead_code, reason = "API publica consumida por integraciones MCP futuras (servers reales)")]
#[derive(Debug, Clone, PartialEq)]
pub enum ElicitationField {
    /// Texto libre. `placeholder` orienta al usuario.
    Text { label: String, placeholder: String },
    /// Seleccion unica entre opciones.
    Select { label: String, options: Vec<String> },
    /// Prompt Y/N.
    Confirm { prompt: String },
    /// Seleccion multiple entre opciones.
    MultiSelect { label: String, options: Vec<String> },
}

impl ElicitationField {
    /// Numero de opciones navegables (0 para Text/Confirm).
    pub fn option_count(&self) -> usize {
        match self {
            Self::Select { options, .. } | Self::MultiSelect { options, .. } => options.len(),
            _ => 0,
        }
    }

    /// Label corto para diagnosticos.
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Text { .. } => "text",
            Self::Select { .. } => "select",
            Self::Confirm { .. } => "confirm",
            Self::MultiSelect { .. } => "multi-select",
        }
    }
}

/// Peticion completa enviada al usuario.
#[derive(Debug, Clone, PartialEq)]
pub struct ElicitationRequest {
    /// Texto introductorio visible en el modal (ej: "El server pide confirmar X").
    pub message: String,
    /// Fuente opcional (ej: nombre del MCP server que pidio la elicitation).
    pub source: Option<String>,
    /// Campo solicitado.
    pub field: ElicitationField,
}

/// Valor con el que el usuario respondio.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Text(String),
    Selected(usize),
    Confirmed(bool),
    MultiSelected(Vec<usize>),
}

/// Respuesta final del usuario al request.
#[allow(
    dead_code,
    reason = "Decline es parte de la API publica del protocolo MCP, aun no emitido desde UI"
)]
#[derive(Debug, Clone, PartialEq)]
pub enum ElicitationResponse {
    /// Usuario acepto y proporciono valor.
    Accept(FieldValue),
    /// Usuario rechazo (equivalente a "No gracias" explicito).
    Decline,
    /// Usuario cancelo con Esc.
    Cancel,
}

/// Envia una elicitation al App y espera la respuesta del usuario.
///
/// Devuelve `Err` si el canal de actions esta cerrado (app termino) o si el
/// sender fue droppeado sin responder (equivalente a Cancel).
#[allow(dead_code, reason = "API de bridge consumida cuando McpClient reciba elicitation/create")]
pub async fn request_elicitation(
    app_tx: &Sender<Action>,
    request: ElicitationRequest,
) -> anyhow::Result<ElicitationResponse> {
    let (tx, rx) = oneshot::channel();
    app_tx
        .send(Action::ElicitationRequested { request, responder: ElicitationResponder::new(tx) })
        .await
        .map_err(|_| anyhow::anyhow!("canal de actions cerrado"))?;
    rx.await.map_err(|_| anyhow::anyhow!("elicitation cancelada sin respuesta"))
}

/// Wrapper newtype sobre `oneshot::Sender<ElicitationResponse>` que implementa
/// `Debug` (requerido por `Action`) sin exponer el contenido del canal.
pub struct ElicitationResponder {
    inner: Option<oneshot::Sender<ElicitationResponse>>,
}

impl ElicitationResponder {
    /// Constructor usado tanto por `request_elicitation` como por tests del
    /// handler que necesitan un responder "mockeable".
    #[allow(dead_code, reason = "consumido por request_elicitation (E18) y tests del handler")]
    pub(crate) fn new(sender: oneshot::Sender<ElicitationResponse>) -> Self {
        Self { inner: Some(sender) }
    }

    /// Consume el responder enviando la respuesta. Si el receptor ya fue
    /// droppeado (app shutdown), el envio es no-op.
    pub fn respond(mut self, response: ElicitationResponse) {
        if let Some(tx) = self.inner.take() {
            let _ = tx.send(response);
        }
    }
}

impl std::fmt::Debug for ElicitationResponder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ElicitationResponder")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[test]
    fn field_option_count_only_counts_list_variants() {
        assert_eq!(
            ElicitationField::Text { label: "x".into(), placeholder: String::new() }.option_count(),
            0
        );
        assert_eq!(ElicitationField::Confirm { prompt: "?".into() }.option_count(), 0);
        assert_eq!(
            ElicitationField::Select {
                label: "x".into(),
                options: vec!["a".into(), "b".into(), "c".into()]
            }
            .option_count(),
            3
        );
        assert_eq!(
            ElicitationField::MultiSelect {
                label: "x".into(),
                options: vec!["a".into(), "b".into()]
            }
            .option_count(),
            2
        );
    }

    #[test]
    fn kind_labels_are_stable() {
        assert_eq!(
            ElicitationField::Text { label: "".into(), placeholder: "".into() }.kind_label(),
            "text"
        );
        assert_eq!(
            ElicitationField::Select { label: "".into(), options: vec![] }.kind_label(),
            "select"
        );
        assert_eq!(ElicitationField::Confirm { prompt: "".into() }.kind_label(), "confirm");
        assert_eq!(
            ElicitationField::MultiSelect { label: "".into(), options: vec![] }.kind_label(),
            "multi-select"
        );
    }

    #[test]
    fn responder_respond_is_no_op_if_rx_dropped() {
        let (tx, rx) = oneshot::channel::<ElicitationResponse>();
        drop(rx);
        let responder = ElicitationResponder::new(tx);
        // No panics aunque el receptor ya cerro.
        responder.respond(ElicitationResponse::Cancel);
    }

    #[tokio::test]
    async fn request_elicitation_delivers_request_and_awaits_response() {
        let (tx, mut rx) = mpsc::channel::<Action>(4);
        let request = ElicitationRequest {
            message: "Elige".into(),
            source: Some("test-server".into()),
            field: ElicitationField::Confirm { prompt: "seguro?".into() },
        };

        let handle = tokio::spawn(async move { request_elicitation(&tx, request.clone()).await });

        let action = rx.recv().await.expect("action");
        let responder = match action {
            Action::ElicitationRequested { request: got, responder } => {
                assert_eq!(got.message, "Elige");
                assert_eq!(got.source.as_deref(), Some("test-server"));
                responder
            }
            other => panic!("expected ElicitationRequested, got {other:?}"),
        };
        responder.respond(ElicitationResponse::Accept(FieldValue::Confirmed(true)));

        let response = handle.await.expect("join").expect("response");
        assert_eq!(response, ElicitationResponse::Accept(FieldValue::Confirmed(true)));
    }

    #[tokio::test]
    async fn request_elicitation_errors_when_app_tx_dropped() {
        let (tx, rx) = mpsc::channel::<Action>(1);
        drop(rx);
        let err = request_elicitation(
            &tx,
            ElicitationRequest {
                message: "x".into(),
                source: None,
                field: ElicitationField::Text { label: "x".into(), placeholder: String::new() },
            },
        )
        .await
        .expect_err("error");
        assert!(err.to_string().contains("canal de actions"));
    }
}
