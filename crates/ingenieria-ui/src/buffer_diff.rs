//! Render statistics — measures how many cells changed per frame.
//!
//! Since ratatui already performs cell-level diffing internally, this module
//! provides observability rather than reimplementing the diff. Enable with
//! `INGENIERIA_RENDER_STATS=1` to log per-frame change counts.
//!
//! Also provides a `diff_cell_count` function to compare two ratatui buffers
//! cell by cell, useful for verifying ratatui's internal diff is effective.

use ratatui::buffer::Buffer;

/// Statistics from a single frame render.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameStats {
    pub total_cells: usize,
    pub changed_cells: usize,
}

impl FrameStats {
    /// Ratio of changed cells (0.0 = nothing changed, 1.0 = full redraw).
    pub fn change_ratio(&self) -> f64 {
        if self.total_cells == 0 {
            return 0.0;
        }
        self.changed_cells as f64 / self.total_cells as f64
    }
}

/// Compare two ratatui buffers cell by cell. Returns count of differing cells.
///
/// Buffers must have the same dimensions; returns total cells as changed
/// if dimensions mismatch (full redraw scenario).
pub fn diff_cell_count(prev: &Buffer, curr: &Buffer) -> FrameStats {
    let total_cells = curr.area.width as usize * curr.area.height as usize;

    if prev.area != curr.area {
        return FrameStats { total_cells, changed_cells: total_cells };
    }

    let changed_cells =
        prev.content().iter().zip(curr.content().iter()).filter(|(a, b)| a != b).count();

    FrameStats { total_cells, changed_cells }
}

/// Whether render stats logging is enabled via environment variable.
pub fn is_stats_enabled() -> bool {
    std::env::var("INGENIERIA_RENDER_STATS").is_ok_and(|v| v == "1" || v == "true")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn identical_buffers_zero_changes() {
        let area = Rect::new(0, 0, 10, 5);
        let buf = Buffer::empty(area);
        let stats = diff_cell_count(&buf, &buf);
        assert_eq!(stats.changed_cells, 0);
        assert_eq!(stats.total_cells, 50);
        assert!((stats.change_ratio() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn different_size_means_full_redraw() {
        let a = Buffer::empty(Rect::new(0, 0, 10, 5));
        let b = Buffer::empty(Rect::new(0, 0, 20, 10));
        let stats = diff_cell_count(&a, &b);
        assert_eq!(stats.changed_cells, stats.total_cells);
    }

    #[test]
    fn partial_change_detected() {
        let area = Rect::new(0, 0, 10, 5);
        let buf_a = Buffer::empty(area);
        let mut buf_b = Buffer::empty(area);
        // Modify a few cells via set_string
        buf_b.set_string(0, 0, "XY", ratatui::style::Style::default());
        let stats = diff_cell_count(&buf_a, &buf_b);
        assert_eq!(stats.changed_cells, 2);
        assert_eq!(stats.total_cells, 50);
    }
}
