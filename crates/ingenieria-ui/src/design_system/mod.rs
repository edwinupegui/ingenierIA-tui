/// Design System — Reusable primitives for consistent UI.
///
/// Provides: Pane (bordered container), Dialog (centered modal),
/// ListItem (selectable row), StatusIcon (semantic), KeyboardHint (shortcut display),
/// and tokens (spacing, borders, z-index).
pub mod dialog;
pub mod diff_bars;
pub mod keyboard_hint;
pub mod list_item;
pub mod pane;
pub mod status_icon;
pub mod tokens;

pub use pane::Pane;
