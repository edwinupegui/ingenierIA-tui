//! Thinking collapse — toggle between collapsed/expanded display of AI thinking blocks.
//!
//! When the AI returns thinking/reasoning content, it can be collapsed to save
//! screen space: "▶ Pensando... (2,847 chars)" vs the full content.

/// Display mode for thinking blocks.
#[derive(Debug, Clone, PartialEq)]
pub enum ThinkingDisplay {
    /// Collapsed: shows summary line "▶ Pensando... (N chars)".
    Collapsed { char_count: usize },
    /// Expanded: shows full thinking content.
    Expanded { content: String },
}

impl ThinkingDisplay {
    /// Create from content, starting collapsed.
    pub fn from_content(content: String) -> Self {
        Self::Collapsed { char_count: content.len() }
    }

    /// Toggle between collapsed and expanded.
    pub fn toggle(&mut self, full_content: &str) {
        *self = match self {
            Self::Collapsed { .. } => Self::Expanded { content: full_content.to_string() },
            Self::Expanded { .. } => Self::Collapsed { char_count: full_content.len() },
        };
    }

    /// Whether currently collapsed.
    pub fn is_collapsed(&self) -> bool {
        matches!(self, Self::Collapsed { .. })
    }

    /// Render the collapsed summary line.
    pub fn summary_text(&self) -> String {
        match self {
            Self::Collapsed { char_count } => {
                format!("▶ Pensando... ({} chars)", format_count(*char_count))
            }
            Self::Expanded { content } => content.clone(),
        }
    }
}

/// Format a character count with thousands separator.
fn format_count(n: usize) -> String {
    if n < 1_000 {
        return n.to_string();
    }
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_collapsed() {
        let td = ThinkingDisplay::from_content("some thinking".into());
        assert!(td.is_collapsed());
    }

    #[test]
    fn toggle_expands_then_collapses() {
        let content = "detailed thinking process";
        let mut td = ThinkingDisplay::from_content(content.into());
        td.toggle(content);
        assert!(!td.is_collapsed());
        td.toggle(content);
        assert!(td.is_collapsed());
    }

    #[test]
    fn summary_shows_char_count() {
        let td = ThinkingDisplay::Collapsed { char_count: 2847 };
        let text = td.summary_text();
        assert!(text.contains("2,847"));
        assert!(text.contains("Pensando"));
    }

    #[test]
    fn format_count_thousands() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1000), "1,000");
        assert_eq!(format_count(12345), "12,345");
        assert_eq!(format_count(1234567), "1,234,567");
    }
}
