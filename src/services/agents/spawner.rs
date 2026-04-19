//! Spawner de subagentes (E22a).
//!
//! Construye un provider, ejecuta una vuelta single-shot (system + user
//! prompt) y emite un `Action::AgentResult` con el output completo.
//!
//! Sprint 10 es MVP: sin tools, sin retry, sin streaming hacia el chat
//! principal — el resultado es un buffer acumulado. Sprint 11 puede agregar
//! tool routing y streaming si se necesita.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::StreamExt;
use tokio::sync::mpsc::Sender;

use crate::actions::Action;
use crate::services::auth::CopilotAuth;
#[cfg(feature = "copilot")]
use crate::services::chat::copilot_provider::CopilotProvider;
use crate::services::chat::{
    claude_provider::ClaudeProvider, mock_provider::MockChatProvider, ChatEvent, ChatProvider,
};
use crate::state::{ChatMessage, ChatRole};

use super::registry::AgentStatus;
use super::role::AgentRole;

/// Tope de subagentes ejecutandose simultaneamente. El check vive en el
/// reducer (slash command) — el spawner asume que ya lo paso.
pub const MAX_CONCURRENT_AGENTS: usize = 3;

/// Snapshot de credenciales pasado al task spawneado para construir el
/// provider sin tocar el `AppState`. La captura efectiva (incluido el lookup
/// de la API key de Claude) la hace el caller en el reducer, donde
/// `crate::app::wizard` es accesible.
#[derive(Debug, Clone)]
pub struct AgentCreds {
    pub claude_key: Option<String>,
    pub copilot_auth: Option<CopilotAuth>,
    pub mock: bool,
}

/// Spawnea un subagent en background.
///
/// El cancel token es compartido con el `AgentInfo` registrado: si el
/// usuario invoca `/agent-cancel`, el spawner descarta el resultado.
pub fn spawn_agent_task(
    id: String,
    role: AgentRole,
    user_prompt: String,
    creds: AgentCreds,
    model: String,
    cancel: Arc<AtomicBool>,
    tx: Sender<Action>,
) {
    tokio::spawn(async move {
        let provider = match build_provider(&creds) {
            Some(p) => p,
            None => {
                send_result(&tx, id, AgentStatus::Failed, Some("sin provider configurado".into()))
                    .await;
                return;
            }
        };

        let messages = vec![
            ChatMessage::new(ChatRole::System, role.system_prompt().to_string()),
            ChatMessage::new(ChatRole::User, user_prompt),
        ];

        match provider.stream_chat(&model, &messages, &[]).await {
            Ok(stream) => {
                let outcome = drain_stream(stream, &cancel).await;
                emit_outcome(&tx, id, &cancel, outcome).await;
            }
            Err(e) => {
                send_result(&tx, id, AgentStatus::Failed, Some(e.to_string())).await;
            }
        }
    });
}

/// Resultado interno de drenar el stream del provider.
enum DrainOutcome {
    Completed(String),
    Cancelled,
}

async fn drain_stream(
    mut stream: crate::services::chat::ChatStream,
    cancel: &Arc<AtomicBool>,
) -> DrainOutcome {
    let mut buffer = String::new();
    while let Some(event) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            return DrainOutcome::Cancelled;
        }
        match event {
            ChatEvent::Delta(s) => buffer.push_str(&s),
            ChatEvent::Done => break,
            // Tool calls, Usage y ThinkingDelta se ignoran en MVP.
            ChatEvent::ToolCall { .. } | ChatEvent::Usage { .. } | ChatEvent::ThinkingDelta(_) => {}

        }
    }
    DrainOutcome::Completed(buffer)
}

async fn emit_outcome(
    tx: &Sender<Action>,
    id: String,
    cancel: &Arc<AtomicBool>,
    outcome: DrainOutcome,
) {
    match outcome {
        _ if cancel.load(Ordering::Relaxed) => {
            send_result(tx, id, AgentStatus::Cancelled, None).await;
        }
        DrainOutcome::Cancelled => {
            send_result(tx, id, AgentStatus::Cancelled, None).await;
        }
        DrainOutcome::Completed(buffer) => {
            send_result(tx, id, AgentStatus::Done, Some(buffer)).await;
        }
    }
}

async fn send_result(tx: &Sender<Action>, id: String, status: AgentStatus, result: Option<String>) {
    let _ = tx.send(Action::AgentResult { id, status, result }).await; // receptor puede haberse caído si el TUI cerró
}

fn build_provider(creds: &AgentCreds) -> Option<Box<dyn ChatProvider>> {
    if creds.mock {
        return Some(Box::new(MockChatProvider::from_env()));
    }
    if let Some(key) = creds.claude_key.clone() {
        return Some(Box::new(ClaudeProvider::new(key)));
    }
    #[cfg(feature = "copilot")]
    {
        creds
            .copilot_auth
            .clone()
            .map(|auth| Box::new(CopilotProvider::new(auth)) as Box<dyn ChatProvider>)
    }
    #[cfg(not(feature = "copilot"))]
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_creds_complete_with_done_status() {
        let creds = AgentCreds { claude_key: None, copilot_auth: None, mock: true };
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Action>(8);
        spawn_agent_task(
            "a1".into(),
            AgentRole::Discovery,
            "ping".into(),
            creds,
            "claude-haiku-4-5-20251001".into(),
            cancel,
            tx,
        );

        let action = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout esperando AgentResult")
            .expect("canal cerrado");

        match action {
            Action::AgentResult { id, status, result } => {
                assert_eq!(id, "a1");
                assert_eq!(status, AgentStatus::Done);
                assert!(result.is_some());
                assert!(!result.unwrap().is_empty());
            }
            other => panic!("Unexpected action: {other:?}"),
        }
    }

    #[tokio::test]
    async fn no_provider_emits_failed_status() {
        let creds = AgentCreds { claude_key: None, copilot_auth: None, mock: false };
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Action>(8);
        spawn_agent_task(
            "a2".into(),
            AgentRole::Generic("x".into()),
            "ping".into(),
            creds,
            "model".into(),
            cancel,
            tx,
        );
        let action = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout")
            .expect("canal cerrado");
        match action {
            Action::AgentResult { status, result, .. } => {
                assert_eq!(status, AgentStatus::Failed);
                assert!(result.unwrap().contains("sin provider"));
            }
            other => panic!("Unexpected action: {other:?}"),
        }
    }

    #[tokio::test]
    async fn cancel_pre_stream_emits_cancelled() {
        let creds = AgentCreds { claude_key: None, copilot_auth: None, mock: true };
        let cancel = Arc::new(AtomicBool::new(true));
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Action>(8);
        spawn_agent_task(
            "a3".into(),
            AgentRole::Testing,
            "ping".into(),
            creds,
            "claude-haiku-4-5-20251001".into(),
            cancel,
            tx,
        );
        let action = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout")
            .expect("canal cerrado");
        match action {
            Action::AgentResult { status, .. } => assert_eq!(status, AgentStatus::Cancelled),
            other => panic!("Unexpected action: {other:?}"),
        }
    }
}
