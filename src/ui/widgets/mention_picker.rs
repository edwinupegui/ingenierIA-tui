//! Widget del `@` mention picker: popover sobre el input del chat.
//!
//! Render similar al `slash_autocomplete` pero con badge de tipo por item
//! (skill/agent/workflow/adr/policy/command). Lista por score del matcher,
//! selección con ↑/↓ y Enter inserta `@kind:name` en el input.
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::state::mention_picker::{MentionKind, MentionPicker};
use crate::ui::theme::{border, dim, surface};
use ingenieria_ui::theme::ColorTheme;

const NAME_COL_WIDTH: u16 = 26;
const BADGE_COL_WIDTH: u16 = 10;

pub fn render(
    f: &mut Frame,
    picker: &MentionPicker,
    input_area: Rect,
    frame: Rect,
    colors: ColorTheme,
) {
    if !picker.visible || picker.matches.is_empty() {
        return;
    }

    let item_count = picker.matches.len();
    let popup_height = item_count as u16 + 2;
    let popup_width = frame.width.saturating_sub(2).max(52);
    let popup_x = frame.x + (frame.width.saturating_sub(popup_width)) / 2;
    let popup_y = input_area.y.saturating_sub(popup_height);

    let area = Rect { x: popup_x, y: popup_y, width: popup_width, height: popup_height };
    f.render_widget(Clear, area);

    let hint = format!(" @ mentions · {} items · Tab/Enter: insertar ", item_count);
    let hint_line =
        Line::styled(hint, Style::default().fg(dim())).alignment(ratatui::layout::Alignment::Right);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border()))
        .style(Style::default().bg(surface()))
        .title_bottom(hint_line);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let desc_width = inner.width.saturating_sub(NAME_COL_WIDTH + BADGE_COL_WIDTH + 4) as usize;

    let lines: Vec<Line<'_>> = picker
        .matches
        .iter()
        .enumerate()
        .filter_map(|(i, (idx, _))| {
            picker
                .items
                .get(*idx)
                .map(|item| render_row(item, i == picker.cursor, desc_width, colors))
        })
        .collect();

    f.render_widget(Paragraph::new(lines).style(Style::default().bg(surface())), inner);
}

fn render_row<'a>(
    item: &'a crate::state::mention_picker::MentionItem,
    selected: bool,
    desc_width: usize,
    colors: ColorTheme,
) -> Line<'a> {
    let (name_style, desc_style, bg_style, badge_fg) = if selected {
        (
            Style::default().fg(colors.bg).bg(colors.purple).add_modifier(Modifier::BOLD),
            Style::default().fg(colors.bg).bg(colors.purple),
            Style::default().bg(colors.purple),
            colors.bg,
        )
    } else {
        (
            Style::default().fg(colors.text),
            Style::default().fg(colors.text_dim),
            Style::default(),
            badge_color(item.kind, colors),
        )
    };

    let name_padded = format!("{:<width$}", item.name, width = NAME_COL_WIDTH as usize);
    let name_truncated: String = name_padded.chars().take(NAME_COL_WIDTH as usize).collect();

    let badge_label = item.kind.label();
    let badge_padded = format!(
        "[{}]{}",
        badge_label,
        " ".repeat(BADGE_COL_WIDTH.saturating_sub(badge_label.len() as u16 + 2) as usize)
    );

    let desc_str: String = if item.description.chars().count() > desc_width {
        let cut: String = item.description.chars().take(desc_width.saturating_sub(1)).collect();
        format!("{cut}…")
    } else {
        item.description.clone()
    };

    Line::from(vec![
        Span::styled(" ", bg_style),
        Span::styled(name_truncated, name_style),
        Span::styled(" ", bg_style),
        Span::styled(
            badge_padded,
            Style::default().fg(badge_fg).bg(if selected { colors.purple } else { colors.bg }),
        ),
        Span::styled(" ", bg_style),
        Span::styled(desc_str, desc_style),
    ])
}

fn badge_color(kind: MentionKind, colors: ColorTheme) -> ratatui::style::Color {
    match kind {
        MentionKind::Skill => colors.purple,
        MentionKind::Agent => colors.cyan,
        MentionKind::Workflow => colors.blue,
        MentionKind::Adr => colors.yellow,
        MentionKind::Policy => colors.red,
        MentionKind::Command => colors.green,
        MentionKind::Other => colors.text_dim,
    }
}
