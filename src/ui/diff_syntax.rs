//! Syntax highlighting for diff lines using syntect (E41 completion).
//!
//! Applies language-specific coloring to diff lines while preserving the
//! diff prefix (+/-/space) and background highlights from word-level diff.

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use std::sync::OnceLock;

// ── Lazy-loaded syntect state ───────────────────────────────────────────────

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

/// The theme name used for syntax highlighting in diffs.
const DIFF_THEME: &str = "base16-ocean.dark";

// ── Public API ──────────────────────────────────────────────────────────────

/// Detect syntect syntax from file extension. Returns None if unknown.
pub fn detect_syntax(path: &str) -> Option<&'static str> {
    let ss = syntax_set();
    let ext = path.rsplit('.').next()?;
    ss.find_syntax_by_extension(ext).map(|s| s.name.as_str())
}

/// Highlight a single line of code and return ratatui Spans.
/// The `prefix` (e.g. "- " or "+ ") and `base_fg` are applied as-is;
/// syntax colors override only the code portion.
pub fn highlight_line<'a>(
    syntax_name: &str,
    code: &str,
    prefix: &'static str,
    prefix_style: Style,
    indent: &'static str,
) -> Option<Line<'a>> {
    let ss = syntax_set();
    let ts = theme_set();
    let syntax = ss.find_syntax_by_name(syntax_name)?;
    let theme = ts.themes.get(DIFF_THEME)?;
    let mut h = HighlightLines::new(syntax, theme);
    let regions = h.highlight_line(code, ss).ok()?;

    let mut spans =
        vec![Span::styled(indent, Style::default()), Span::styled(prefix, prefix_style)];

    for (style, text) in regions {
        let fg = syntect_to_ratatui_color(style.foreground);
        spans.push(Span::styled(text.to_string(), Style::default().fg(fg)));
    }

    Some(Line::from(spans))
}

/// Convert syntect RGBA to ratatui Color.
fn syntect_to_ratatui_color(c: syntect::highlighting::Color) -> ratatui::style::Color {
    ratatui::style::Color::Rgb(c.r, c.g, c.b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::{green, red};

    #[test]
    fn detect_syntax_known_extensions() {
        assert!(detect_syntax("foo.rs").is_some());
        assert!(detect_syntax("bar.py").is_some());
        assert!(detect_syntax("baz.js").is_some());
    }

    #[test]
    fn detect_syntax_unknown_returns_none() {
        assert!(detect_syntax("file.zzz_unknown_ext").is_none());
    }

    #[test]
    fn highlight_line_produces_spans() {
        let result =
            highlight_line("Rust", "let x = 42;", "+ ", Style::default().fg(green()), "  ");
        assert!(result.is_some());
        let line = result.unwrap();
        // At least indent + prefix + some code spans
        assert!(line.spans.len() >= 3);
    }

    #[test]
    fn highlight_line_unknown_syntax_returns_none() {
        let result =
            highlight_line("NonExistentLanguage", "code", "- ", Style::default().fg(red()), "");
        assert!(result.is_none());
    }

    #[test]
    fn syntect_color_converts() {
        let c = syntect::highlighting::Color { r: 255, g: 128, b: 0, a: 255 };
        let rc = syntect_to_ratatui_color(c);
        assert_eq!(rc, ratatui::style::Color::Rgb(255, 128, 0));
    }
}
