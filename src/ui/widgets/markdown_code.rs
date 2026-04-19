use ratatui::{
    style::{Modifier, Style},
    text::Span,
};

use ingenieria_ui::theme::ColorTheme;

// ── Syntax highlighting basico ──────────────────────────────────────────────

/// Keywords comunes por familia de lenguajes.
const KEYWORDS_RUST: &[&str] = &[
    "fn", "let", "mut", "pub", "use", "mod", "struct", "enum", "impl", "trait", "self", "Self",
    "match", "if", "else", "for", "while", "loop", "return", "async", "await", "where", "const",
    "static", "type", "crate", "super", "move", "ref", "as", "in", "true", "false",
];

const KEYWORDS_JS_TS: &[&str] = &[
    "function",
    "const",
    "let",
    "var",
    "class",
    "interface",
    "type",
    "import",
    "export",
    "from",
    "return",
    "if",
    "else",
    "for",
    "while",
    "async",
    "await",
    "new",
    "this",
    "extends",
    "implements",
    "enum",
    "default",
    "switch",
    "case",
    "break",
    "continue",
    "try",
    "catch",
    "throw",
    "true",
    "false",
    "null",
    "undefined",
    "void",
    "of",
    "in",
];

const KEYWORDS_CSHARP: &[&str] = &[
    "using",
    "namespace",
    "class",
    "interface",
    "public",
    "private",
    "protected",
    "internal",
    "static",
    "void",
    "var",
    "new",
    "return",
    "if",
    "else",
    "for",
    "foreach",
    "while",
    "async",
    "await",
    "override",
    "virtual",
    "abstract",
    "sealed",
    "readonly",
    "const",
    "true",
    "false",
    "null",
    "this",
    "base",
    "get",
    "set",
    "string",
    "int",
    "bool",
    "double",
    "float",
    "decimal",
    "Task",
    "List",
    "Dictionary",
];

fn keywords_for_lang(lang: &str) -> &'static [&'static str] {
    match lang {
        "rust" | "rs" => KEYWORDS_RUST,
        "javascript" | "js" | "typescript" | "ts" | "tsx" | "jsx" => KEYWORDS_JS_TS,
        "csharp" | "cs" | "c#" => KEYWORDS_CSHARP,
        _ => KEYWORDS_JS_TS, // Fallback razonable
    }
}

/// Colorea una linea de diff (unified format) con background sutil.
pub(super) fn highlight_diff_line(line: &str, theme: &ColorTheme) -> Span<'static> {
    let style = if line.starts_with("+++") || line.starts_with("---") {
        Style::default().fg(theme.blue).add_modifier(Modifier::BOLD)
    } else if line.starts_with('+') {
        Style::default().fg(theme.green).bg(theme.surface_positive)
    } else if line.starts_with('-') {
        Style::default().fg(theme.red).bg(theme.surface_negative)
    } else if line.starts_with("@@") {
        Style::default().fg(theme.cyan)
    } else {
        Style::default().fg(theme.text_secondary)
    };
    Span::styled(line.to_string(), style)
}

/// Coloreado basico de una linea de codigo. Sin dependencias externas.
/// Retorna spans owned ('static) para compatibilidad con el renderer.
pub(super) fn highlight_code_line(
    line: &str,
    lang: &str,
    theme: &ColorTheme,
) -> Vec<Span<'static>> {
    let trimmed = line.trim_start();

    // Comentarios de linea completa
    if trimmed.starts_with("//") || trimmed.starts_with('#') {
        return vec![Span::styled(line.to_string(), Style::default().fg(theme.text_dim))];
    }

    let keywords = keywords_for_lang(lang);
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut chars = line.char_indices().peekable();
    let mut segment_start = 0;

    while let Some(&(i, c)) = chars.peek() {
        // Strings
        if c == '"' || c == '\'' {
            if i > segment_start {
                highlight_segment(&mut spans, &line[segment_start..i], keywords, theme);
            }
            let quote = c;
            chars.next();
            let str_start = i;
            while let Some(&(j, sc)) = chars.peek() {
                chars.next();
                if sc == quote && (j == 0 || line.as_bytes().get(j - 1) != Some(&b'\\')) {
                    break;
                }
            }
            let str_end = chars.peek().map(|&(j, _)| j).unwrap_or(line.len());
            spans.push(Span::styled(
                line[str_start..str_end].to_string(),
                Style::default().fg(theme.green),
            ));
            segment_start = str_end;
            continue;
        }

        // Numbers (digit at word boundary)
        if c.is_ascii_digit()
            && (i == 0
                || !line.as_bytes().get(i - 1).map(|b| b.is_ascii_alphanumeric()).unwrap_or(false))
        {
            if i > segment_start {
                highlight_segment(&mut spans, &line[segment_start..i], keywords, theme);
            }
            let num_start = i;
            chars.next();
            while let Some(&(_, nc)) = chars.peek() {
                if nc.is_ascii_digit() || nc == '.' || nc == 'x' || nc == '_' {
                    chars.next();
                } else {
                    break;
                }
            }
            let num_end = chars.peek().map(|&(j, _)| j).unwrap_or(line.len());
            spans.push(Span::styled(
                line[num_start..num_end].to_string(),
                Style::default().fg(theme.yellow),
            ));
            segment_start = num_end;
            continue;
        }

        chars.next();
    }

    if segment_start < line.len() {
        highlight_segment(&mut spans, &line[segment_start..], keywords, theme);
    }

    if spans.is_empty() {
        spans.push(Span::styled(line.to_string(), Style::default().fg(theme.green)));
    }

    spans
}

/// Colorea un segmento de texto identificando keywords y PascalCase types.
fn highlight_segment(
    spans: &mut Vec<Span<'static>>,
    text: &str,
    keywords: &[&str],
    theme: &ColorTheme,
) {
    let mut last = 0;
    for (i, c) in text.char_indices() {
        if !c.is_alphanumeric() && c != '_' {
            if i > last {
                push_word_span(spans, &text[last..i], keywords, theme);
            }
            spans.push(Span::styled(c.to_string(), Style::default().fg(theme.green)));
            last = i + c.len_utf8();
        }
    }
    if last < text.len() {
        push_word_span(spans, &text[last..], keywords, theme);
    }
}

fn push_word_span(
    spans: &mut Vec<Span<'static>>,
    word: &str,
    keywords: &[&str],
    theme: &ColorTheme,
) {
    let style = if keywords.contains(&word) {
        Style::default().fg(theme.purple).add_modifier(Modifier::BOLD)
    } else if is_type_name(word) {
        Style::default().fg(theme.cyan)
    } else {
        Style::default().fg(theme.green)
    };
    spans.push(Span::styled(word.to_string(), style));
}

/// Detecta nombres PascalCase como posibles tipos/clases.
fn is_type_name(word: &str) -> bool {
    let mut chars = word.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return false,
    };
    // Debe empezar con mayuscula y contener al menos una minuscula
    first.is_uppercase() && chars.any(|c| c.is_lowercase())
}

/// Renders the lines inside a code block, applying syntax highlighting or diff coloring.
pub(super) fn render_code_block_lines(
    lines: &mut Vec<ratatui::text::Line<'static>>,
    text: &str,
    code_lang: &str,
    theme: &ColorTheme,
) {
    let is_diff = code_lang == "diff";
    for line in text.lines() {
        let mut spans = vec![Span::styled("  │ ", Style::default().fg(theme.border))];
        if is_diff {
            spans.push(highlight_diff_line(line, theme));
        } else {
            spans.extend(highlight_code_line(line, code_lang, theme));
        }
        lines.push(ratatui::text::Line::from(spans));
    }
}
