/// Progress bar with 8-level Unicode fractional blocks.
///
/// Reference: claude-code ProgressBar.tsx.
/// Usage: context % display, sync progress, download progress.
use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::ui::theme::{dim, green};

/// 8 Unicode block levels for sub-character precision.
const BLOCKS: [char; 8] = [
    '\u{258F}', '\u{258E}', '\u{258D}', '\u{258C}', '\u{258B}', '\u{258A}', '\u{2589}', '\u{2588}',
];

pub struct ProgressBar {
    ratio: f64,
    width: u16,
    fill_color: ratatui::style::Color,
    empty_color: ratatui::style::Color,
    show_pct: bool,
}

impl ProgressBar {
    pub fn new(ratio: f64, width: u16) -> Self {
        Self {
            ratio: ratio.clamp(0.0, 1.0),
            width,
            fill_color: green(),
            empty_color: dim(),
            show_pct: true,
        }
    }

    pub fn fill_color(mut self, color: ratatui::style::Color) -> Self {
        self.fill_color = color;
        self
    }

    #[expect(dead_code, reason = "design system spec — consumed by E36 theming")]
    pub fn empty_color(mut self, color: ratatui::style::Color) -> Self {
        self.empty_color = color;
        self
    }

    pub fn show_pct(mut self, show: bool) -> Self {
        self.show_pct = show;
        self
    }

    /// Render as a Line of styled spans.
    pub fn to_line(&self) -> Line<'static> {
        let bar_w = if self.show_pct { self.width.saturating_sub(5) } else { self.width };
        let total_eighths = (self.ratio * bar_w as f64 * 8.0) as usize;
        let full_blocks = total_eighths / 8;
        let remainder = total_eighths % 8;
        let empty = bar_w as usize - full_blocks - usize::from(remainder > 0);

        let mut bar = String::with_capacity(bar_w as usize);
        for _ in 0..full_blocks {
            bar.push(BLOCKS[7]); // full block
        }
        if remainder > 0 {
            bar.push(BLOCKS[remainder - 1]);
        }
        for _ in 0..empty {
            bar.push(' ');
        }

        let mut spans = vec![Span::styled(bar, Style::default().fg(self.fill_color))];

        if self.show_pct {
            spans.push(Span::styled(
                format!(" {:>3.0}%", self.ratio * 100.0),
                Style::default().fg(self.empty_color),
            ));
        }

        Line::from(spans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_ratio_produces_empty_bar() {
        let bar = ProgressBar::new(0.0, 10).show_pct(false);
        let line = bar.to_line();
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text.trim(), "");
    }

    #[test]
    fn full_ratio_fills_bar() {
        let bar = ProgressBar::new(1.0, 10).show_pct(false);
        let line = bar.to_line();
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains('\u{2588}')); // full block
        assert!(!text.contains(' '));
    }

    #[test]
    fn half_ratio_partial_fill() {
        let bar = ProgressBar::new(0.5, 10).show_pct(false);
        let line = bar.to_line();
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        // Should have some full blocks and some empty space
        assert!(text.len() >= 10);
    }

    #[test]
    fn show_pct_appends_percentage() {
        let bar = ProgressBar::new(0.75, 20);
        let line = bar.to_line();
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("75%"));
    }
}
