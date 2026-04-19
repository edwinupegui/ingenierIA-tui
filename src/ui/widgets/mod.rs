pub mod agent_panel;
#[cfg(feature = "autoskill")]
mod autoskill_picker;
mod command_palette;
pub mod cost_panel;
pub mod doc_picker;
pub mod doctor;
pub mod elicitation_modal;
pub mod enforcement;
pub mod gauge;
pub mod hints;
pub mod history_search;
pub mod markdown;
mod markdown_code;
pub mod mention_picker;
pub mod message_nav;
#[expect(dead_code, reason = "E38 spec — consumed when chat/dashboard integrate MinDisplayTime")]
pub mod min_display_time;
mod model_picker;
pub mod monitor_panel;
pub mod notifications;
pub mod onboarding_checklist;
pub mod permission_modal;
#[expect(
    dead_code,
    reason = "E38 spec — consumed when context %, sync, and download use progress bars"
)]
pub mod progress_bar;
#[expect(dead_code, reason = "E38 spec — consumed when dashboard/doc_picker use scroll indicators")]
pub mod scroll_indicator;
mod search_overlay;
mod sessions_panel;
pub mod slash_autocomplete;
mod theme_picker;
pub mod tip_card;
pub mod toasts;
pub mod todo_panel;
pub mod tool_monitor;
pub mod transcript_modal;

pub use agent_panel::render_agent_panel;
#[cfg(feature = "autoskill")]
pub use autoskill_picker::render_autoskill_picker;
pub use command_palette::render_command_palette;
pub use cost_panel::render_cost_panel;
pub use doctor::render_doctor;
pub use elicitation_modal::render_elicitation_modal;
pub use enforcement::render_enforcement;
pub use model_picker::render_model_picker;
pub use monitor_panel::render_monitor_panel;
pub use notifications::render_notifications;
pub use onboarding_checklist::{render_checklist, CHECKLIST_HEIGHT};
pub use permission_modal::render_permission_modal;
pub use search_overlay::render_search_overlay;
pub use sessions_panel::render_sessions_panel;
pub use theme_picker::render_theme_picker;
pub use tip_card::render_tip;
pub use toasts::render_toasts;
pub use tool_monitor::render_tool_monitor;
pub use transcript_modal::render_transcript_modal;

// ── Shared utilities ────────────────────────────────────────────────────────

pub fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}..", &s[..max - 2])
    }
}

/// Extract HH:MM:SS from an ISO timestamp string (e.g. "2025-01-15T14:30:00Z" → "14:30:00").
pub fn extract_time(timestamp: &str) -> &str {
    timestamp.get(11..19).unwrap_or("")
}

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
    }
}
