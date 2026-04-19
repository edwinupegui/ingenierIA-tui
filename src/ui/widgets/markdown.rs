use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use ingenieria_ui::theme::ColorTheme;

use crate::ui::theme::{GLYPH_BULLET, GLYPH_BULLET_NESTED};

use super::markdown_code::render_code_block_lines;

/// Convierte markdown a Lines de ratatui con color semántico usando pulldown-cmark.
pub fn render_markdown(content: &str, theme: &ColorTheme) -> Vec<Line<'static>> {
    let opts = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    let parser = Parser::new_ext(content, opts);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();

    // Style stack for nested inline formatting
    let mut bold = false;
    let mut italic = false;
    let code_inline = false;
    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut heading_level: Option<u8> = None;
    let mut list_depth: usize = 0;
    let mut ordered_index: Option<u64> = None;
    let mut link_url: Option<String> = None;
    let mut in_link = false;

    // Table rendering state (pulldown-cmark emits Table/TableHead/TableRow/TableCell).
    let mut in_table = false;
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut table_current_row: Vec<String> = Vec::new();
    let mut table_current_cell: String = String::new();
    let mut table_header_len: usize = 0; // nro de columnas del header

    for event in parser {
        match event {
            // ── Headings ──────────────────────────────────────────────
            Event::Start(Tag::Heading { level, .. }) => {
                heading_level = Some(level as u8);
                current_spans.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                let level = heading_level.take().unwrap_or(1);
                let style = match level {
                    1 => Style::default().fg(theme.blue).add_modifier(Modifier::BOLD),
                    2 => Style::default().fg(theme.yellow).add_modifier(Modifier::BOLD),
                    _ => Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                };
                // Merge all spans into one heading line with the heading style
                let text: String = current_spans.drain(..).map(|s| s.content.to_string()).collect();
                lines.push(Line::from(Span::styled(format!("  {text}"), style)));
            }

            // ── Code blocks ───────────────────────────────────────────
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                code_lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => lang.to_string(),
                    _ => String::new(),
                };
                let tag = if code_lang.is_empty() { "code" } else { &code_lang };
                let fill = 38_usize.saturating_sub(tag.len());
                let header = format!("  ╭─ {tag} {}", "─".repeat(fill).to_string() + "╮");
                lines.push(Line::from(Span::styled(header, Style::default().fg(theme.green))));
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                code_lang.clear();
                lines.push(Line::from(Span::styled(
                    "  ╰────────────────────────────────────────╯",
                    Style::default().fg(theme.border),
                )));
            }

            // ── Paragraphs ────────────────────────────────────────────
            Event::Start(Tag::Paragraph) => {
                current_spans.clear();
            }
            Event::End(TagEnd::Paragraph) => {
                if !current_spans.is_empty() {
                    let mut spans = vec![Span::raw("  ")];
                    spans.append(&mut current_spans);
                    lines.push(Line::from(spans));
                }
                lines.push(Line::from(""));
            }

            // ── Lists ─────────────────────────────────────────────────
            Event::Start(Tag::List(first)) => {
                ordered_index = first;
                list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                list_depth = list_depth.saturating_sub(1);
                if list_depth == 0 {
                    ordered_index = None;
                }
            }
            Event::Start(Tag::Item) => {
                current_spans.clear();
            }
            Event::End(TagEnd::Item) => {
                let mut spans = Vec::new();
                if list_depth > 1 {
                    // Nested list
                    spans.push(Span::styled(
                        format!("    {GLYPH_BULLET_NESTED} "),
                        Style::default().fg(theme.text_dim),
                    ));
                } else if let Some(idx) = &mut ordered_index {
                    spans.push(Span::styled(
                        format!("  {idx}. "),
                        Style::default().fg(theme.yellow),
                    ));
                    *idx += 1;
                } else {
                    spans.push(Span::styled(
                        format!("  {GLYPH_BULLET} "),
                        Style::default().fg(theme.blue),
                    ));
                }
                spans.append(&mut current_spans);
                lines.push(Line::from(spans));
            }

            // ── Inline formatting ─────────────────────────────────────
            Event::Start(Tag::Strong) => bold = true,
            Event::End(TagEnd::Strong) => bold = false,
            Event::Start(Tag::Emphasis) => italic = true,
            Event::End(TagEnd::Emphasis) => italic = false,

            // ── Text content ──────────────────────────────────────────
            Event::Text(text) => {
                if in_table {
                    table_current_cell.push_str(&text);
                } else if in_code_block {
                    render_code_block_lines(&mut lines, &text, &code_lang, theme);
                } else if code_inline {
                    current_spans
                        .push(Span::styled(text.to_string(), Style::default().fg(theme.green)));
                } else if heading_level.is_some() {
                    // Accumulate heading text, style applied at End
                    current_spans.push(Span::raw(text.to_string()));
                } else if in_link {
                    current_spans.push(Span::styled(
                        text.to_string(),
                        Style::default().fg(theme.blue).add_modifier(Modifier::UNDERLINED),
                    ));
                } else {
                    let mut style = Style::default().fg(theme.text_secondary);
                    if bold {
                        style = style.add_modifier(Modifier::BOLD).fg(theme.text);
                    }
                    if italic {
                        style = style.add_modifier(Modifier::ITALIC);
                    }
                    current_spans.push(Span::styled(text.to_string(), style));
                }
            }

            Event::Code(code) => {
                if in_table {
                    table_current_cell.push_str(&code);
                } else {
                    current_spans
                        .push(Span::styled(code.to_string(), Style::default().fg(theme.green)));
                }
            }

            // ── Thematic break (---) ──────────────────────────────────
            Event::Rule => {
                lines.push(Line::from(Span::styled(
                    "  ────────────────────────────────────────────".to_string(),
                    Style::default().fg(theme.border),
                )));
            }

            Event::SoftBreak | Event::HardBreak => {
                if !current_spans.is_empty() {
                    let mut spans = vec![Span::raw("  ")];
                    spans.append(&mut current_spans);
                    lines.push(Line::from(spans));
                }
            }

            // ── Tables ────────────────────────────────────────────────
            Event::Start(Tag::Table(_)) => {
                in_table = true;
                table_rows.clear();
                table_header_len = 0;
            }
            Event::End(TagEnd::Table) => {
                render_table_lines(&mut lines, &table_rows, table_header_len, theme);
                lines.push(Line::from(""));
                in_table = false;
                table_rows.clear();
                table_current_row.clear();
                table_current_cell.clear();
                table_header_len = 0;
            }
            Event::Start(Tag::TableHead) => {
                table_current_row.clear();
            }
            Event::End(TagEnd::TableHead) => {
                // pulldown-cmark emite cells del header DIRECTO dentro de
                // TableHead (sin TableRow); por eso recolectamos aqui.
                if !table_current_row.is_empty() {
                    table_header_len = table_current_row.len();
                    table_rows.push(std::mem::take(&mut table_current_row));
                }
            }
            Event::Start(Tag::TableRow) => {
                table_current_row.clear();
            }
            Event::End(TagEnd::TableRow) => {
                table_rows.push(std::mem::take(&mut table_current_row));
            }
            Event::Start(Tag::TableCell) => {
                table_current_cell.clear();
            }
            Event::End(TagEnd::TableCell) => {
                table_current_row.push(std::mem::take(&mut table_current_cell));
            }

            // Links: show text styled as link, append URL in dim
            Event::Start(Tag::Link { dest_url, .. }) => {
                link_url = Some(dest_url.to_string());
                in_link = true;
            }
            Event::End(TagEnd::Link) => {
                if let Some(url) = link_url.take() {
                    if !url.is_empty() {
                        current_spans.push(Span::styled(
                            format!(" ({url})"),
                            Style::default().fg(theme.text_dim),
                        ));
                    }
                }
                in_link = false;
            }

            _ => {}
        }
    }

    // Flush remaining spans
    if !current_spans.is_empty() {
        let mut spans = vec![Span::raw("  ")];
        spans.append(&mut current_spans);
        lines.push(Line::from(spans));
    }

    lines
}

/// Renderea una tabla como ASCII con bordes │ y header destacado. Pads cada
/// celda al ancho maximo de su columna (calculado sobre todas las filas).
fn render_table_lines(
    lines: &mut Vec<Line<'static>>,
    rows: &[Vec<String>],
    header_len: usize,
    theme: &ColorTheme,
) {
    if rows.is_empty() {
        return;
    }
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return;
    }
    let mut widths = vec![0usize; cols];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            let w = cell.chars().count();
            if w > widths[i] {
                widths[i] = w;
            }
        }
    }

    // Cada celda se renderea como ` cell ` con paddings simetricos.
    let dash_segments: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
    let top = format!("  ┌{}┐", dash_segments.join("┬"));
    let sep = format!("  ├{}┤", dash_segments.join("┼"));
    let bottom = format!("  └{}┘", dash_segments.join("┴"));

    lines.push(Line::from(Span::styled(top, Style::default().fg(theme.border))));
    for (row_idx, row) in rows.iter().enumerate() {
        let is_header = header_len > 0 && row_idx == 0;
        let mut spans: Vec<Span<'static>> = Vec::with_capacity(cols * 2 + 1);
        spans.push(Span::styled("  │".to_string(), Style::default().fg(theme.border)));
        for (i, width) in widths.iter().enumerate() {
            let cell = row.get(i).cloned().unwrap_or_default();
            let pad = width.saturating_sub(cell.chars().count());
            let content = format!(" {cell}{}", " ".repeat(pad));
            let style = if is_header {
                Style::default().fg(theme.yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_secondary)
            };
            spans.push(Span::styled(content, style));
            spans.push(Span::styled(" │".to_string(), Style::default().fg(theme.border)));
        }
        lines.push(Line::from(spans));
        if is_header {
            lines.push(Line::from(Span::styled(sep.clone(), Style::default().fg(theme.border))));
        }
    }
    lines.push(Line::from(Span::styled(bottom, Style::default().fg(theme.border))));
}

/// Stream-safe markdown rendering. Splits content at the last safe boundary,
/// renders the complete portion as markdown, and returns the partial tail as plain text.
///
/// Safe boundaries: empty lines (paragraph breaks) and closed code fences.
/// This prevents half-rendered headers, broken code blocks, and incomplete lists.
pub fn render_markdown_streaming(content: &str, theme: &ColorTheme) -> Vec<Line<'static>> {
    let boundary = find_safe_boundary(content);

    if boundary == 0 {
        // No safe boundary found — render everything as plain text
        return content
            .lines()
            .map(|l| {
                Line::from(Span::styled(
                    format!("  {l}"),
                    Style::default().fg(theme.text_secondary),
                ))
            })
            .collect();
    }

    let (complete, tail) = content.split_at(boundary);
    let mut lines = render_markdown(complete, theme);

    // Append remaining partial text as-is (no markdown parsing)
    if !tail.trim().is_empty() {
        for l in tail.lines() {
            lines.push(Line::from(Span::styled(
                format!("  {l}"),
                Style::default().fg(theme.text_secondary),
            )));
        }
    }

    lines
}

/// Find the byte offset of the last "safe boundary" in the content.
///
/// A safe boundary is a position after which we should NOT parse markdown,
/// because the content may be incomplete. We look for:
/// 1. The last empty line (paragraph break)
/// 2. Code fence balance (only split outside unclosed fences)
fn find_safe_boundary(content: &str) -> usize {
    // Count code fences to detect if we're inside an unclosed block
    let fence_count = content.matches("```").count();
    let in_unclosed_fence = !fence_count.is_multiple_of(2);

    if in_unclosed_fence {
        // Find the last opening fence that isn't closed — split before it
        if let Some(pos) = content.rfind("```") {
            return pos;
        }
    }

    // Find the last empty line (paragraph boundary)
    let bytes = content.as_bytes();
    let mut last_boundary = 0;

    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            // Check if next char is also \n (empty line)
            if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                last_boundary = i + 2; // Position after the empty line
                i += 2;
                continue;
            }
        }
        i += 1;
    }

    last_boundary
}

#[cfg(test)]
mod tests {
    use super::render_markdown;
    use ingenieria_ui::theme::TOKYO_NIGHT;

    #[test]
    fn tables_render_as_bordered_ascii() {
        let md = "| h1 | h2 |\n|---|---|\n| a | b |\n| ccc | d |";
        let lines = render_markdown(md, &TOKYO_NIGHT);
        let texts: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect::<String>())
            .collect();
        let rendered = texts.join("\n");
        assert!(rendered.contains("│"), "expected │ separators: {rendered}");
        assert!(
            rendered.contains("┌") && rendered.contains("┐"),
            "expected top corners: {rendered}"
        );
        assert!(rendered.contains("h1") && rendered.contains("h2"), "header cells missing");
        assert!(rendered.contains("ccc"), "body cell missing");
    }

    #[test]
    fn code_block_uses_theme_colors() {
        use ingenieria_ui::theme::GRUVBOX;
        let tokyo_lines = render_markdown(md_code(), &TOKYO_NIGHT);
        let gruvbox_lines = render_markdown(md_code(), &GRUVBOX);

        let tokyo_has_bg = tokyo_lines.iter().any(|l| {
            l.spans.iter().any(|s| {
                s.style.fg == Some(TOKYO_NIGHT.border) || s.style.fg == Some(TOKYO_NIGHT.green)
            })
        });
        let gruvbox_has_bg = gruvbox_lines.iter().any(|l| {
            l.spans
                .iter()
                .any(|s| s.style.fg == Some(GRUVBOX.border) || s.style.fg == Some(GRUVBOX.green))
        });
        assert!(tokyo_has_bg && gruvbox_has_bg);
        assert_ne!(TOKYO_NIGHT.border, GRUVBOX.border);
    }

    fn md_code() -> &'static str {
        "```rust\nfn foo() {}\n```"
    }
}
