//! Handler de eventos para elicitation modal (E18).
//!
//! El modal se abre al recibir `Action::ElicitationRequested`. Mientras
//! `state.chat.pending_elicitation.is_some()` el teclado se enruta aqui con
//! prioridad (ver `keys_chat.rs`).

use crate::services::mcp::elicitation::{
    ElicitationField, ElicitationRequest, ElicitationResponder, ElicitationResponse, FieldValue,
};
use crate::state::chat_types::PendingElicitation;

use super::App;

impl App {
    /// Registra un elicitation pendiente y fuerza que la pantalla sea Chat
    /// para que el modal sea visible.
    pub(super) fn handle_elicitation_requested(
        &mut self,
        request: ElicitationRequest,
        responder: ElicitationResponder,
    ) {
        if self.state.chat.pending_elicitation.is_some() {
            // Solo soportamos un elicitation simultaneo. Cancelamos el nuevo.
            responder.respond(ElicitationResponse::Cancel);
            self.notify("⚠ Elicitation descartada (ya hay una activa)".into());
            return;
        }
        self.state.chat.pending_elicitation = Some(PendingElicitation::new(request, responder));
        self.state.screen = crate::state::AppScreen::Chat;
    }

    /// Handler de un caracter dentro del modal. Retorna `true` si lo consumio.
    pub(super) fn on_char_elicitation(&mut self, c: char) -> bool {
        let Some(pending) = self.state.chat.pending_elicitation.as_mut() else {
            return false;
        };
        match &pending.request.field {
            ElicitationField::Text { .. } => {
                pending.text_buffer.push(c);
                true
            }
            ElicitationField::Confirm { .. } => match c {
                'y' | 'Y' => {
                    self.resolve_elicitation(ElicitationResponse::Accept(FieldValue::Confirmed(
                        true,
                    )));
                    true
                }
                'n' | 'N' => {
                    self.resolve_elicitation(ElicitationResponse::Accept(FieldValue::Confirmed(
                        false,
                    )));
                    true
                }
                _ => true, // tragamos todo mientras esta el modal abierto
            },
            ElicitationField::MultiSelect { .. } if c == ' ' => {
                toggle_multi_selection(pending);
                true
            }
            _ => true,
        }
    }

    /// Handler de backspace. Solo aplica en Text. Retorna `true` si consumio.
    pub(super) fn on_backspace_elicitation(&mut self) -> bool {
        let Some(pending) = self.state.chat.pending_elicitation.as_mut() else {
            return false;
        };
        if matches!(pending.request.field, ElicitationField::Text { .. }) {
            pending.text_buffer.pop();
        }
        true
    }

    /// Handler de arrow up/down para mover cursor en Select/MultiSelect.
    pub(super) fn on_up_elicitation(&mut self) -> bool {
        let Some(pending) = self.state.chat.pending_elicitation.as_mut() else {
            return false;
        };
        let n = pending.request.field.option_count();
        if n > 0 && pending.cursor > 0 {
            pending.cursor -= 1;
        }
        true
    }

    pub(super) fn on_down_elicitation(&mut self) -> bool {
        let Some(pending) = self.state.chat.pending_elicitation.as_mut() else {
            return false;
        };
        let n = pending.request.field.option_count();
        if n > 0 && pending.cursor + 1 < n {
            pending.cursor += 1;
        }
        true
    }

    /// Handler de Enter: acepta el valor actual y cierra el modal.
    pub(super) fn on_enter_elicitation(&mut self) -> bool {
        let Some(pending) = self.state.chat.pending_elicitation.as_ref() else {
            return false;
        };
        let value = match &pending.request.field {
            ElicitationField::Text { .. } => FieldValue::Text(pending.text_buffer.clone()),
            ElicitationField::Select { .. } => FieldValue::Selected(pending.cursor),
            ElicitationField::Confirm { .. } => FieldValue::Confirmed(true),
            ElicitationField::MultiSelect { .. } => {
                FieldValue::MultiSelected(pending.multi_selected.iter().copied().collect())
            }
        };
        self.resolve_elicitation(ElicitationResponse::Accept(value));
        true
    }

    /// Handler de Esc: cancela y cierra.
    pub(super) fn on_esc_elicitation(&mut self) -> bool {
        if self.state.chat.pending_elicitation.is_none() {
            return false;
        }
        self.resolve_elicitation(ElicitationResponse::Cancel);
        true
    }

    /// Consume el pending y envia la respuesta al oneshot.
    fn resolve_elicitation(&mut self, response: ElicitationResponse) {
        if let Some(pending) = self.state.chat.pending_elicitation.take() {
            pending.responder.respond(response);
        }
    }
}

/// Alterna el indice `cursor` en el set de selecciones multi.
fn toggle_multi_selection(pending: &mut PendingElicitation) {
    let idx = pending.cursor;
    if !pending.multi_selected.insert(idx) {
        pending.multi_selected.remove(&idx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::oneshot;

    fn test_app() -> App {
        let (tx, _rx) = tokio::sync::mpsc::channel::<crate::actions::Action>(8);
        let client = Arc::new(crate::services::IngenieriaClient::new("http://test"));
        let config = crate::config::Config {
            server_url: "http://test".into(),
            developer: "t".into(),
            provider: "github-copilot".into(),
            model: "m".into(),
            default_factory: None,
            theme: None,
        };
        App::new(client, tx, config, false, false, false)
    }

    fn pending_with(
        field: ElicitationField,
    ) -> (PendingElicitation, oneshot::Receiver<ElicitationResponse>) {
        let (tx, rx) = oneshot::channel();
        let pending = PendingElicitation::new(
            ElicitationRequest { message: "msg".into(), source: None, field },
            ElicitationResponder::new(tx),
        );
        (pending, rx)
    }

    #[test]
    fn toggle_multi_selection_flips_indices() {
        let (mut p, _rx) = pending_with(ElicitationField::MultiSelect {
            label: "x".into(),
            options: vec!["a".into(), "b".into(), "c".into()],
        });
        p.cursor = 1;
        toggle_multi_selection(&mut p);
        assert!(p.multi_selected.contains(&1));
        toggle_multi_selection(&mut p);
        assert!(!p.multi_selected.contains(&1));
    }

    #[tokio::test]
    async fn enter_on_text_sends_accept_with_buffer() {
        let mut app = test_app();
        let (pending, rx) =
            pending_with(ElicitationField::Text { label: "x".into(), placeholder: "".into() });
        app.state.chat.pending_elicitation = Some(pending);
        app.on_char_elicitation('h');
        app.on_char_elicitation('i');
        assert!(app.on_enter_elicitation());
        assert!(app.state.chat.pending_elicitation.is_none());
        let response = rx.await.expect("response");
        assert_eq!(response, ElicitationResponse::Accept(FieldValue::Text("hi".into())));
    }

    #[tokio::test]
    async fn y_key_on_confirm_sends_confirmed_true() {
        let mut app = test_app();
        let (pending, rx) = pending_with(ElicitationField::Confirm { prompt: "ok?".into() });
        app.state.chat.pending_elicitation = Some(pending);
        app.on_char_elicitation('Y');
        assert!(app.state.chat.pending_elicitation.is_none());
        let response = rx.await.expect("response");
        assert_eq!(response, ElicitationResponse::Accept(FieldValue::Confirmed(true)));
    }

    #[tokio::test]
    async fn arrows_move_select_cursor_and_enter_returns_selected() {
        let mut app = test_app();
        let (pending, rx) = pending_with(ElicitationField::Select {
            label: "x".into(),
            options: vec!["a".into(), "b".into(), "c".into()],
        });
        app.state.chat.pending_elicitation = Some(pending);
        app.on_down_elicitation();
        app.on_down_elicitation();
        app.on_up_elicitation();
        assert!(app.on_enter_elicitation());
        let response = rx.await.expect("response");
        assert_eq!(response, ElicitationResponse::Accept(FieldValue::Selected(1)));
    }

    #[tokio::test]
    async fn backspace_on_text_pops_char() {
        let mut app = test_app();
        let (pending, rx) =
            pending_with(ElicitationField::Text { label: "x".into(), placeholder: "".into() });
        app.state.chat.pending_elicitation = Some(pending);
        app.on_char_elicitation('a');
        app.on_char_elicitation('b');
        app.on_backspace_elicitation();
        app.on_enter_elicitation();
        let response = rx.await.expect("response");
        assert_eq!(response, ElicitationResponse::Accept(FieldValue::Text("a".into())));
    }

    #[tokio::test]
    async fn esc_sends_cancel() {
        let mut app = test_app();
        let (pending, rx) =
            pending_with(ElicitationField::Text { label: "x".into(), placeholder: "".into() });
        app.state.chat.pending_elicitation = Some(pending);
        assert!(app.on_esc_elicitation());
        let response = rx.await.expect("response");
        assert_eq!(response, ElicitationResponse::Cancel);
    }

    #[tokio::test]
    async fn second_request_is_rejected_when_one_is_already_active() {
        let mut app = test_app();
        let (first, _rx_first) = pending_with(ElicitationField::Confirm { prompt: "a".into() });
        app.state.chat.pending_elicitation = Some(first);
        let (tx, rx) = oneshot::channel();
        app.handle_elicitation_requested(
            ElicitationRequest {
                message: "b".into(),
                source: None,
                field: ElicitationField::Confirm { prompt: "b".into() },
            },
            ElicitationResponder::new(tx),
        );
        let response = rx.await.expect("response");
        assert_eq!(response, ElicitationResponse::Cancel);
    }
}
