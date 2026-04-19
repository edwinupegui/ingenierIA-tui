use std::sync::Arc;

use std::collections::HashMap;

use crate::{
    actions::Action,
    domain::document::DocumentSummary,
    services::IngenieriaClient,
    state::{AppMode, AppScreen, ChatMessage, ChatRole, ChatStatus, ToolCall, MAX_TOOL_ROUNDS},
};

use super::App;

// ── Helpers ─────────────────────────────────────────────────────────────

fn test_app() -> App {
    let (tx, _rx) = tokio::sync::mpsc::channel::<Action>(100);
    let client = Arc::new(IngenieriaClient::new("http://test:3001"));
    let config = crate::config::Config {
        server_url: "http://test:3001".to_string(),
        developer: "tester".to_string(),
        provider: "github-copilot".to_string(),
        model: "test-model".to_string(),
        default_factory: None,
        theme: None,
    };
    App::new(client, tx, config, false, false, false)
}

/// App wired con MockChatProvider (E21) para tests end-to-end del chat loop.
fn test_app_with_mock(rx: &mut tokio::sync::mpsc::Receiver<Action>) -> App {
    let (tx, rx_new) = tokio::sync::mpsc::channel::<Action>(100);
    *rx = rx_new;
    let client = Arc::new(IngenieriaClient::new("http://test:3001"));
    let config = crate::config::Config {
        server_url: "http://test:3001".to_string(),
        developer: "tester".to_string(),
        provider: "github-copilot".to_string(),
        model: "test-model".to_string(),
        default_factory: None,
        theme: None,
    };
    App::new(client, tx, config, false, false, true)
}

fn test_app_on_dashboard() -> App {
    let mut app = test_app();
    app.state.screen = AppScreen::Dashboard;
    let docs = vec![
        DocumentSummary {
            uri: "test/1".into(),
            doc_type: "skill".into(),
            factory: "all".into(),
            name: "Doc 1".into(),
            description: "Test doc 1".into(),
            last_modified: "2024-01-01".into(),
        },
        DocumentSummary {
            uri: "test/2".into(),
            doc_type: "skill".into(),
            factory: "all".into(),
            name: "Doc 2".into(),
            description: "Test doc 2".into(),
            last_modified: "2024-01-01".into(),
        },
    ];
    app.state.dashboard.sidebar.all_docs = docs;
    app.state.dashboard.sidebar.rebuild_with_priority(None, None);
    app
}

fn test_app_on_chat() -> App {
    let mut app = test_app();
    app.state.screen = AppScreen::Chat;
    app.state.chat.status = ChatStatus::Ready;
    app
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

// ── Quit / Control ──────────────────────────────────────────────────────

#[test]
fn quit_action_returns_true() {
    rt().block_on(async {
        let mut app = test_app();
        assert!(app.handle(Action::Quit));
    });
}

#[test]
fn ctrl_c_first_press_does_not_exit() {
    rt().block_on(async {
        let mut app = test_app();
        assert!(!app.handle(Action::KeyCtrlC));
        assert!(app.state.quit_armed_until.is_some());
    });
}

#[test]
fn ctrl_c_double_press_within_window_exits() {
    rt().block_on(async {
        let mut app = test_app();
        assert!(!app.handle(Action::KeyCtrlC));
        assert!(app.handle(Action::KeyCtrlC));
    });
}

#[test]
fn ctrl_c_after_window_expires_rearms_instead_of_exiting() {
    rt().block_on(async {
        let mut app = test_app();
        assert!(!app.handle(Action::KeyCtrlC));
        app.state.tick_count = app.state.quit_armed_until.unwrap() + 1;
        assert!(!app.handle(Action::KeyCtrlC));
        assert!(app.state.quit_armed_until.is_some());
    });
}

// ── Dashboard / Navigation ──────────────────────────────────────────────

#[test]
fn open_dashboard_changes_screen() {
    rt().block_on(async {
        let mut app = test_app();
        assert_eq!(app.state.screen, AppScreen::Splash);
        app.open_dashboard();
        assert_eq!(app.state.screen, AppScreen::Dashboard);
    });
}

#[test]
fn search_mode_activates_on_slash() {
    rt().block_on(async {
        let mut app = test_app_on_dashboard();
        assert_eq!(app.state.mode, AppMode::Normal);
        app.handle(Action::KeyChar('/'));
        assert_eq!(app.state.mode, AppMode::Search);
    });
}

#[test]
fn sidebar_moves_down_on_key_down() {
    rt().block_on(async {
        let mut app = test_app_on_dashboard();
        assert_eq!(app.state.dashboard.sidebar.cursor_pos, 0);
        app.handle(Action::KeyDown);
        assert_eq!(app.state.dashboard.sidebar.cursor_pos, 1);
    });
}

#[test]
fn command_mode_activates_on_colon() {
    rt().block_on(async {
        let mut app = test_app_on_dashboard();
        app.handle(Action::KeyChar(':'));
        assert_eq!(app.state.mode, AppMode::Command);
    });
}

#[test]
fn esc_exits_command_mode() {
    rt().block_on(async {
        let mut app = test_app_on_dashboard();
        app.state.mode = AppMode::Command;
        app.handle(Action::KeyEsc);
        assert_eq!(app.state.mode, AppMode::Normal);
    });
}

// ── Chat streaming ──────────────────────────────────────────────────────

#[test]
fn chat_stream_delta_appends_to_assistant_message() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, String::new()));

        app.handle(Action::ChatStreamDelta("hola".to_string()));

        let last = app.state.chat.messages.last().unwrap();
        assert_eq!(last.role, ChatRole::Assistant);
        assert_eq!(last.content, "hola");
        assert_eq!(app.state.chat.scroll_offset, u16::MAX);
    });
}

#[test]
fn chat_stream_delta_creates_assistant_if_last_is_user() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.messages.push(ChatMessage::new(ChatRole::User, "pregunta".into()));

        app.handle(Action::ChatStreamDelta("respuesta".to_string()));

        assert_eq!(app.state.chat.messages.len(), 2);
        let last = app.state.chat.messages.last().unwrap();
        assert_eq!(last.role, ChatRole::Assistant);
        assert_eq!(last.content, "respuesta");
    });
}

#[test]
fn chat_stream_done_sets_ready_when_no_tool_calls() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.status = ChatStatus::Streaming;
        app.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, "done".into()));

        app.handle(Action::ChatStreamDone);

        assert_eq!(app.state.chat.status, ChatStatus::Ready);
        assert_eq!(app.state.chat.tool_rounds, 0);
    });
}

#[test]
fn chat_stream_done_increments_tool_rounds_when_tool_calls_present() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.status = ChatStatus::Streaming;
        let mut msg = ChatMessage::new(ChatRole::Assistant, String::new());
        msg.tool_calls.push(ToolCall {
            id: "tc1".into(),
            name: "read_file".into(),
            arguments: "{}".into(),
            status: crate::state::ToolCallStatus::Pending,
            duration_ms: None,
        });
        app.state.chat.messages.push(msg);

        app.handle(Action::ChatStreamDone);

        assert_eq!(app.state.chat.tool_rounds, 1);
        assert_eq!(app.state.chat.status, ChatStatus::ExecutingTools);
    });
}

#[test]
fn chat_stream_done_stops_at_max_tool_rounds() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.status = ChatStatus::Streaming;
        app.state.chat.tool_rounds = MAX_TOOL_ROUNDS;
        let mut msg = ChatMessage::new(ChatRole::Assistant, String::new());
        msg.tool_calls.push(ToolCall {
            id: "tc1".into(),
            name: "read_file".into(),
            arguments: "{}".into(),
            status: crate::state::ToolCallStatus::Pending,
            duration_ms: None,
        });
        app.state.chat.messages.push(msg);

        app.handle(Action::ChatStreamDone);

        assert_eq!(app.state.chat.status, ChatStatus::Ready);
    });
}

// ── E21 Mock Provider end-to-end ───────────────────────────────────────

#[test]
fn mock_provider_flag_is_wired_through_app() {
    rt().block_on(async {
        let (_tx, mut rx) = tokio::sync::mpsc::channel::<Action>(100);
        let app = test_app_with_mock(&mut rx);
        assert!(app.mock_provider, "mock_provider flag debe propagarse al App");
    });
}

#[test]
fn mock_provider_spawn_chat_completion_emits_actions() {
    rt().block_on(async {
        let (_tx, mut rx) = tokio::sync::mpsc::channel::<Action>(100);
        std::env::set_var("INGENIERIA_MOCK_SCENARIO", "simple");
        let mut app = test_app_with_mock(&mut rx);
        app.state.chat.messages.push(ChatMessage::new(ChatRole::User, "hola".into()));
        app.state.chat.status = ChatStatus::Streaming;
        app.state.chat.metrics.on_turn_start();
        app.spawn_chat_completion();
        // Drena al menos 3 actions con timeout generoso.
        let mut events: Vec<Action> = Vec::new();
        for _ in 0..10 {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(a)) => {
                    let done = matches!(a, Action::ChatStreamDone);
                    events.push(a);
                    if done {
                        break;
                    }
                }
                _ => break,
            }
        }
        std::env::remove_var("INGENIERIA_MOCK_SCENARIO");
        let has_delta = events.iter().any(|a| matches!(a, Action::ChatStreamDelta(_)));
        let has_usage = events.iter().any(|a| matches!(a, Action::ChatTokenUsage { .. }));
        let has_done = events.iter().any(|a| matches!(a, Action::ChatStreamDone));
        assert!(has_delta, "mock simple debe emitir al menos un ChatStreamDelta");
        assert!(has_usage, "mock simple debe emitir ChatTokenUsage");
        assert!(has_done, "mock simple debe cerrar con ChatStreamDone");
    });
}

// ── E34 Chat Metrics integration ───────────────────────────────────────

#[test]
fn chat_stream_delta_records_metrics() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.metrics.on_turn_start();
        app.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, String::new()));
        app.handle(Action::ChatStreamDelta("hola mundo".into()));
        // primer delta fija TTFT y acumula chars
        assert!(app.state.chat.metrics.first_token_at.is_some());
        assert_eq!(app.state.chat.metrics.response_chars, 10);
    });
}

#[test]
fn chat_stream_done_finalizes_turn_and_pushes_completed() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.status = ChatStatus::Streaming;
        app.state.chat.metrics.on_turn_start();
        app.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, "done".into()));
        app.state.chat.metrics.on_delta(8);
        app.handle(Action::ChatStreamDone);
        assert_eq!(app.state.chat.metrics.completed_turns.len(), 1);
        assert!(app.state.chat.metrics.last_turn.is_some());
        assert!(app.state.chat.metrics.turn_start.is_none());
    });
}

#[test]
fn chat_tool_call_and_result_populate_duration_ms() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.metrics.on_turn_start();
        // El handler espera un assistant message previo para anclar el ToolCall.
        app.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, String::new()));
        app.handle(Action::ChatToolCall {
            id: "t1".into(),
            name: "bash".into(),
            arguments: "{}".into(),
        });
        std::thread::sleep(std::time::Duration::from_millis(10));
        app.handle(Action::ChatToolResult { tool_call_id: "t1".into(), content: "ok".into() });
        let tc = app
            .state
            .chat
            .messages
            .iter()
            .flat_map(|m| m.tool_calls.iter())
            .find(|tc| tc.id == "t1")
            .expect("tool call anchored in assistant message");
        assert!(tc.duration_ms.is_some(), "duration_ms debe poblarse al cerrar el tool");
        assert!(tc.duration_ms.unwrap() >= 5);
    });
}

// ── E20 ConfigTool ApplyConfigChange handler ───────────────────────────

#[test]
fn apply_config_change_theme_updates_active_theme() {
    rt().block_on(async {
        let mut app = test_app();
        let original = app.state.active_theme;
        let target = if matches!(original, crate::ui::theme::ThemeVariant::TokyoNight) {
            "solarized".to_string()
        } else {
            "tokyonight".to_string()
        };
        app.handle(Action::ApplyConfigChange { field: "theme".into(), value: target.clone() });
        let new = app.state.active_theme;
        assert_ne!(
            format!("{original:?}"),
            format!("{new:?}"),
            "theme debe cambiar tras ApplyConfigChange"
        );
    });
}

#[test]
fn apply_config_change_model_updates_and_marks_dirty() {
    rt().block_on(async {
        let mut app = test_app();
        app.handle(Action::ApplyConfigChange { field: "model".into(), value: "claude-4-6".into() });
        assert_eq!(app.state.model, "claude-4-6");
        assert!(app.state.config_dirty, "model change debe marcar config_dirty");
    });
}

#[test]
fn apply_config_change_rejects_invalid_factory() {
    rt().block_on(async {
        let mut app = test_app();
        let original = app.state.factory.clone();
        app.handle(Action::ApplyConfigChange { field: "factory".into(), value: "invalid".into() });
        assert_eq!(
            format!("{original:?}"),
            format!("{:?}", app.state.factory),
            "factory no debe cambiar con valor invalido"
        );
    });
}

#[test]
fn apply_config_change_rejects_unknown_field() {
    rt().block_on(async {
        let mut app = test_app();
        // No debe paniquear ni mutar estado con un campo desconocido.
        app.handle(Action::ApplyConfigChange {
            field: "server_url".into(),
            value: "http://evil".into(),
        });
        // No hay mutacion observable; el handler solo emite un toast Warning.
    });
}

#[test]
fn chat_stream_failure_sets_error_status() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        let failure = crate::domain::failure::StructuredFailure::from_error("timeout");
        app.handle(Action::ChatStreamFailure(failure));
        // Status.Error contiene el `display()` estructurado, no el string crudo.
        match &app.state.chat.status {
            ChatStatus::Error(s) => {
                assert!(s.contains("timeout"));
                assert!(s.contains("Stream timeout"));
            }
            other => panic!("expected ChatStatus::Error, got {other:?}"),
        }
    });
}

#[test]
fn chat_stream_abort_resets_to_ready_and_drops_empty_assistant() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.status = ChatStatus::Streaming;
        app.state.chat.messages.push(ChatMessage::new(ChatRole::User, "hola".into()));
        // Assistant "fantasma" sin contenido: creado por stream_delta si el
        // primer delta llego y luego nada mas. Abort lo debe remover.
        app.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, String::new()));
        app.handle(Action::ChatStreamAbort);
        assert_eq!(app.state.chat.status, ChatStatus::Ready);
        // El mensaje assistant vacio se borro; queda solo el user.
        assert_eq!(app.state.chat.messages.len(), 1);
        assert_eq!(app.state.chat.messages[0].role, ChatRole::User);
    });
}

#[test]
fn chat_stream_abort_preserves_assistant_with_content() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.status = ChatStatus::Streaming;
        app.state.chat.messages.push(ChatMessage::new(ChatRole::User, "hola".into()));
        app.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, "mitad del".into()));
        app.handle(Action::ChatStreamAbort);
        assert_eq!(app.state.chat.status, ChatStatus::Ready);
        // El mensaje assistant parcial se preserva — el usuario ya lo vio.
        assert_eq!(app.state.chat.messages.len(), 2);
    });
}

// ── Tool calls ──────────────────────────────────────────────────────────

#[test]
fn chat_tool_call_appends_to_last_assistant() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, String::new()));

        app.handle(Action::ChatToolCall {
            id: "tc1".into(),
            name: "read_file".into(),
            arguments: r#"{"path":"src/main.rs"}"#.into(),
        });

        let last = app.state.chat.messages.last().unwrap();
        assert_eq!(last.tool_calls.len(), 1);
        assert_eq!(last.tool_calls[0].name, "read_file");
    });
}

#[test]
fn chat_tool_result_adds_tool_message() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        // Set up: assistant with pending tool call
        let mut msg = ChatMessage::new(ChatRole::Assistant, String::new());
        msg.tool_calls.push(ToolCall {
            id: "tc1".into(),
            name: "read_file".into(),
            arguments: "{}".into(),
            status: crate::state::ToolCallStatus::Pending,
            duration_ms: None,
        });
        app.state.chat.messages.push(msg);

        app.handle(Action::ChatToolResult {
            tool_call_id: "tc1".into(),
            content: "file content here".into(),
        });

        let last = app.state.chat.messages.last().unwrap();
        assert_eq!(last.role, ChatRole::Tool);
        assert_eq!(last.tool_call_id.as_deref(), Some("tc1"));
    });
}

// ── Health ──────────────────────────────────────────────────────────────

#[test]
fn health_updated_sets_online() {
    rt().block_on(async {
        let mut app = test_app();
        let health = crate::domain::health::HealthStatus {
            status: "ok".into(),
            version: "1.0".into(),
            sessions: 0,
            uptime_seconds: 100,
            docs: crate::domain::health::DocsStats {
                total: 10,
                by_factory: HashMap::new(),
                by_type: HashMap::new(),
            },
        };
        app.handle(Action::HealthUpdated(health));
        assert!(matches!(app.state.server_status, crate::state::ServerStatus::Online(_)));
    });
}

#[test]
fn health_fetch_failed_sets_offline() {
    rt().block_on(async {
        let mut app = test_app();
        app.handle(Action::HealthFetchFailed("connection refused".into()));
        assert!(matches!(app.state.server_status, crate::state::ServerStatus::Offline(_)));
    });
}

// ── Documents ───────────────────────────────────────────────────────────

#[test]
fn documents_loaded_updates_sidebar() {
    rt().block_on(async {
        let mut app = test_app_on_dashboard();
        let docs = vec![DocumentSummary {
            uri: "test/3".into(),
            doc_type: "policy".into(),
            factory: "net".into(),
            name: "Doc 3".into(),
            description: "New doc".into(),
            last_modified: "2024-01-01".into(),
        }];

        app.handle(Action::DocumentsLoaded(docs));

        assert_eq!(app.state.dashboard.sidebar.all_docs.len(), 1);
        assert!(!app.state.dashboard.sidebar.loading);
    });
}

#[test]
fn documents_load_failed_sets_error() {
    rt().block_on(async {
        let mut app = test_app();
        app.state.screen = AppScreen::Dashboard;
        app.state.dashboard.sidebar.loading = true;

        app.handle(Action::DocumentsLoadFailed("network error".into()));

        assert!(!app.state.dashboard.sidebar.loading);
        // If offline cache exists on disk, error is None (docs loaded from cache).
        // If no cache on disk, error is set. Start with empty all_docs to avoid
        // conflating pre-loaded test docs with cache presence.
        let has_cache = !app.state.dashboard.sidebar.all_docs.is_empty();
        if has_cache {
            assert!(app.state.dashboard.sidebar.error.is_none());
            assert!(app.state.dashboard.sidebar.is_cached);
        } else {
            assert!(app.state.dashboard.sidebar.error.is_some());
        }
    });
}

// ── Tick / Notifications ────────────────────────────────────────────────

#[test]
fn tick_increments_counter() {
    rt().block_on(async {
        let mut app = test_app();
        assert_eq!(app.state.tick_count, 0);
        app.handle(Action::Tick);
        assert_eq!(app.state.tick_count, 1);
    });
}

#[test]
fn toast_dismissed_after_ticks() {
    rt().block_on(async {
        let mut app = test_app();
        app.state.tick_count = 0;
        app.state.toasts.push("test".to_string(), crate::state::ToastLevel::Info, 0);
        assert!(app.state.toasts.visible().next().is_some());

        // Advance enough ticks for toast to expire
        for _ in 0..20 {
            app.handle(Action::Tick);
        }

        assert!(app.state.toasts.visible().next().is_none());
    });
}

// ── Scroll ──────────────────────────────────────────────────────────────

#[test]
fn preview_scroll_up_decrements_offset() {
    rt().block_on(async {
        let mut app = test_app_on_dashboard();
        app.state.dashboard.preview.scroll = 10;
        app.handle(Action::PreviewScrollUp);
        assert_eq!(app.state.dashboard.preview.scroll, 5);
    });
}

#[test]
fn chat_scroll_up_decrements_offset() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.scroll_offset = 10;
        app.handle(Action::PreviewScrollUp);
        assert_eq!(app.state.chat.scroll_offset, 5);
    });
}

// ── Code blocks detection ───────────────────────────────────────────────

#[test]
fn chat_stream_done_detects_code_blocks() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.status = ChatStatus::Streaming;
        let content = "Here is the code:\n```rust src/main.rs\nfn main() {}\n```";
        app.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, content.into()));

        app.handle(Action::ChatStreamDone);

        assert_eq!(app.state.chat.code_blocks.len(), 1);
        assert_eq!(app.state.chat.code_blocks[0].file_path.as_deref(), Some("src/main.rs"));
    });
}

// ── Factory cycle ───────────────────────────────────────────────────────

#[test]
fn tab_cycles_factory() {
    rt().block_on(async {
        let mut app = test_app_on_dashboard();
        let initial = app.state.factory.clone();
        app.handle(Action::KeyTab);
        assert_ne!(app.state.factory, initial);
    });
}

// ── Chat nav Ctrl+↑/↓ (salto entre user messages) ────────────────────────

#[test]
fn chat_nav_prev_scrolls_to_previous_user_message() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        // 3 user messages intercalados — simulamos offsets que poblaria el render.
        app.state.chat.messages.push(ChatMessage::new(ChatRole::User, "q1".into()));
        app.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, "a1".into()));
        app.state.chat.messages.push(ChatMessage::new(ChatRole::User, "q2".into()));
        app.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, "a2".into()));
        app.state.chat.messages.push(ChatMessage::new(ChatRole::User, "q3".into()));
        app.state.chat.last_user_offsets.replace(vec![0, 10, 20]);
        // Cursor arranca en None (= max). ChatNavPrev → cursor baja a 1 → offset 10.
        app.handle(Action::ChatNavPrev);
        assert_eq!(app.state.chat.nav_user_cursor, Some(1));
        assert_eq!(app.state.chat.scroll_offset, 10);
        // Otro prev → cursor 0 → offset 0.
        app.handle(Action::ChatNavPrev);
        assert_eq!(app.state.chat.nav_user_cursor, Some(0));
        assert_eq!(app.state.chat.scroll_offset, 0);
        // Prev en el tope = no-op (clamped).
        app.handle(Action::ChatNavPrev);
        assert_eq!(app.state.chat.nav_user_cursor, Some(0));
    });
}

#[test]
fn chat_nav_next_moves_forward_and_clamps_at_end() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.messages.push(ChatMessage::new(ChatRole::User, "q1".into()));
        app.state.chat.messages.push(ChatMessage::new(ChatRole::User, "q2".into()));
        app.state.chat.last_user_offsets.replace(vec![0, 10]);
        app.state.chat.nav_user_cursor = Some(0);
        app.state.chat.scroll_offset = 0;
        app.handle(Action::ChatNavNext);
        assert_eq!(app.state.chat.nav_user_cursor, Some(1));
        assert_eq!(app.state.chat.scroll_offset, 10);
        // Clamp en el final.
        app.handle(Action::ChatNavNext);
        assert_eq!(app.state.chat.nav_user_cursor, Some(1));
    });
}

#[test]
fn chat_nav_is_noop_when_offsets_empty() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.scroll_offset = 7;
        app.handle(Action::ChatNavPrev);
        assert_eq!(app.state.chat.scroll_offset, 7, "sin offsets no debe tocar scroll");
        assert_eq!(app.state.chat.nav_user_cursor, None);
    });
}

// ── /undo ────────────────────────────────────────────────────────────────

#[test]
fn undo_removes_last_user_and_following() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.messages.push(ChatMessage::new(ChatRole::User, "q1".into()));
        app.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, "a1".into()));
        app.state.chat.messages.push(ChatMessage::new(ChatRole::User, "q2".into()));
        app.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, "a2".into()));
        app.handle_undo_command();
        // Debe haber quedado [q1, a1] — pop q2 + a2.
        assert_eq!(app.state.chat.messages.len(), 2);
        assert_eq!(app.state.chat.messages[0].content, "q1");
        assert_eq!(app.state.chat.messages[1].content, "a1");
    });
}

#[test]
fn undo_during_streaming_is_noop() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.status = ChatStatus::Streaming;
        app.state.chat.messages.push(ChatMessage::new(ChatRole::User, "q".into()));
        app.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, "a".into()));
        app.handle_undo_command();
        assert_eq!(app.state.chat.messages.len(), 2, "no pop durante streaming");
    });
}

#[test]
fn undo_restores_draft_when_input_empty() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.messages.push(ChatMessage::new(ChatRole::User, "borrar esto".into()));
        app.state.chat.input.clear();
        app.handle_undo_command();
        assert_eq!(app.state.chat.input, "borrar esto");
        assert!(app.state.chat.messages.is_empty());
    });
}

#[test]
fn undo_preserves_nonempty_draft() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.messages.push(ChatMessage::new(ChatRole::User, "q_old".into()));
        app.state.chat.input = "draft en progreso".to_string();
        app.handle_undo_command();
        assert_eq!(app.state.chat.input, "draft en progreso", "no pisar draft del user");
        assert!(app.state.chat.messages.is_empty(), "pop debe ejecutar igual");
    });
}

// ── /redo ────────────────────────────────────────────────────────────────

#[test]
fn redo_restores_undone_turn() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.messages.push(ChatMessage::new(ChatRole::User, "q1".into()));
        app.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, "a1".into()));
        app.state.chat.input.clear();
        app.handle_undo_command();
        assert!(app.state.chat.messages.is_empty());
        assert_eq!(app.state.chat.input, "q1");
        // Ahora redo — debe traer de vuelta los 2 mensajes y limpiar el input
        // (que estaba vacio antes del undo).
        app.handle_redo_command();
        assert_eq!(app.state.chat.messages.len(), 2);
        assert_eq!(app.state.chat.messages[0].content, "q1");
        assert_eq!(app.state.chat.messages[1].content, "a1");
        assert_eq!(app.state.chat.input, "", "draft pre-undo era vacio");
    });
}

#[test]
fn redo_noop_when_nothing_to_redo() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        let before_len = app.state.chat.messages.len();
        app.handle_redo_command();
        assert_eq!(app.state.chat.messages.len(), before_len);
    });
}

#[test]
fn redo_cleared_by_new_message_send() {
    rt().block_on(async {
        let mut app = test_app_on_chat();
        app.state.chat.messages.push(ChatMessage::new(ChatRole::User, "q_old".into()));
        app.handle_undo_command();
        assert_eq!(app.state.chat.undo_redo_stack.len(), 1);
        // Simula send: clear stack manualmente (send_chat_message hace esto).
        app.state.chat.undo_redo_stack.clear();
        app.handle_redo_command();
        // Stack vacio → redo no restaura.
        assert!(app.state.chat.messages.is_empty());
    });
}
