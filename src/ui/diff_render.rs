//! Diff visual rendering (E41).
//!
//! Helpers puros que generan un diff line-by-line entre dos strings usando
//! LCS (Longest Common Subsequence) clasico con DP O(n·m). Sirve para el
//! permission modal (antes de aplicar Edit) y para el tool result expanded
//! (despues de Edit/Write). Sin dependencias externas.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use super::diff_word::{
    compute_word_diff, lines_similar_enough, render_word_added, render_word_removed,
};
use super::theme::{dim, green, red, white};

/// Operacion de diff sobre una linea.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOp {
    /// Linea presente en ambos lados (contexto).
    Context(String),
    /// Linea removida del lado viejo.
    Removed(String),
    /// Linea agregada en el lado nuevo.
    Added(String),
}

/// Configuracion de rendering. `max_lines` es el cap total de lineas
/// emitidas; al exceder se agrega un "… N mas" al final.
#[derive(Debug, Clone)]
pub struct DiffRenderOpts {
    pub max_lines: usize,
    pub indent: &'static str,
    /// Numero de lineas de contexto alrededor de un hunk. 0 = sin contexto.
    pub context: usize,
    /// File path hint for syntax highlighting (optional).
    pub file_path: Option<String>,
}

impl Default for DiffRenderOpts {
    fn default() -> Self {
        Self { max_lines: 20, indent: "  ", context: 2, file_path: None }
    }
}

/// Calcula el diff line-by-line entre `old` y `new` usando LCS clasico.
///
/// Complejidad O(n·m). Apto para bloques Edit de pocos KB. Para textos
/// enormes (>1MB) el caller deberia pre-truncar.
pub fn compute_diff(old: &str, new: &str) -> Vec<DiffOp> {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let n = a.len();
    let m = b.len();

    // Early-out: ambos vacios o identicos.
    if n == 0 && m == 0 {
        return Vec::new();
    }
    if a == b {
        return a.into_iter().map(|l| DiffOp::Context(l.to_string())).collect();
    }

    // DP table: lcs[i][j] = LCS(a[..i], b[..j]).
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in 0..n {
        for j in 0..m {
            lcs[i + 1][j + 1] =
                if a[i] == b[j] { lcs[i][j] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }

    // Backtrack para producir la secuencia.
    let mut ops = Vec::with_capacity(n + m);
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && a[i - 1] == b[j - 1] {
            ops.push(DiffOp::Context(a[i - 1].to_string()));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || lcs[i][j - 1] >= lcs[i - 1][j]) {
            ops.push(DiffOp::Added(b[j - 1].to_string()));
            j -= 1;
        } else {
            ops.push(DiffOp::Removed(a[i - 1].to_string()));
            i -= 1;
        }
    }
    ops.reverse();
    ops
}

/// Filtra el diff mostrando solo hunks con `context` lineas alrededor de
/// cambios. Si no hay cambios retorna vec vacio.
fn collapse_context(ops: &[DiffOp], context: usize) -> Vec<DiffOp> {
    if context == 0 {
        return ops.iter().filter(|op| !matches!(op, DiffOp::Context(_))).cloned().collect();
    }
    // Marca indices "interesantes": los cambios + `context` lineas de cada lado.
    let mut keep = vec![false; ops.len()];
    for (idx, op) in ops.iter().enumerate() {
        if !matches!(op, DiffOp::Context(_)) {
            let start = idx.saturating_sub(context);
            let end = (idx + context + 1).min(ops.len());
            for item in keep.iter_mut().take(end).skip(start) {
                *item = true;
            }
        }
    }
    ops.iter().zip(keep.iter()).filter(|(_, k)| **k).map(|(op, _)| op.clone()).collect()
}

/// Renderiza el diff como lineas ratatui con colores del tema.
/// If `opts.file_path` is set, applies syntax highlighting via syntect.
pub fn render_diff_lines(old: &str, new: &str, opts: DiffRenderOpts) -> Vec<Line<'static>> {
    let ops = compute_diff(old, new);
    let filtered = collapse_context(&ops, opts.context);

    if filtered.is_empty() {
        return vec![Line::from(vec![
            Span::styled(opts.indent, Style::default()),
            Span::styled("(sin cambios)", Style::default().fg(dim())),
        ])];
    }

    // Detect syntax for highlighting (best-effort, falls back to plain).
    let syntax_name = opts.file_path.as_deref().and_then(super::diff_syntax::detect_syntax);

    let total = filtered.len();
    let mut out = Vec::with_capacity(opts.max_lines.min(total) + 1);
    let mut i = 0;
    while i < filtered.len() && out.len() < opts.max_lines {
        // Detect adjacent Removed+Added pair for word-level diff.
        if let DiffOp::Removed(old_line) = &filtered[i] {
            if i + 1 < filtered.len() {
                if let DiffOp::Added(new_line) = &filtered[i + 1] {
                    if lines_similar_enough(old_line, new_line) {
                        let (old_ops, new_ops) = compute_word_diff(old_line, new_line);
                        out.push(render_word_removed(&old_ops, opts.indent));
                        if out.len() < opts.max_lines {
                            out.push(render_word_added(&new_ops, opts.indent));
                        }
                        i += 2;
                        continue;
                    }
                }
            }
        }
        out.push(op_to_line_highlighted(&filtered[i], opts.indent, syntax_name));
        i += 1;
    }
    if total > opts.max_lines && out.len() >= opts.max_lines {
        let rest = total - i;
        if rest > 0 {
            out.push(Line::from(vec![
                Span::styled(opts.indent, Style::default()),
                Span::styled(format!("… {rest} lineas mas"), Style::default().fg(dim())),
            ]));
        }
    }
    out
}

/// Render a single diff op with optional syntax highlighting.
fn op_to_line_highlighted(
    op: &DiffOp,
    indent: &'static str,
    syntax_name: Option<&str>,
) -> Line<'static> {
    if let Some(name) = syntax_name {
        match op {
            DiffOp::Removed(s) => {
                let prefix_style = Style::default().fg(red()).add_modifier(Modifier::BOLD);
                if let Some(line) =
                    super::diff_syntax::highlight_line(name, s, "- ", prefix_style, indent)
                {
                    return line;
                }
            }
            DiffOp::Added(s) => {
                let prefix_style = Style::default().fg(green()).add_modifier(Modifier::BOLD);
                if let Some(line) =
                    super::diff_syntax::highlight_line(name, s, "+ ", prefix_style, indent)
                {
                    return line;
                }
            }
            DiffOp::Context(s) => {
                let prefix_style = Style::default().fg(dim());
                if let Some(line) =
                    super::diff_syntax::highlight_line(name, s, "  ", prefix_style, indent)
                {
                    return line;
                }
            }
        }
    }
    // Fallback to plain coloring.
    op_to_line(op, indent)
}

fn op_to_line(op: &DiffOp, indent: &'static str) -> Line<'static> {
    match op {
        DiffOp::Context(s) => Line::from(vec![
            Span::styled(indent, Style::default()),
            Span::styled("  ", Style::default().fg(dim())),
            Span::styled(s.clone(), Style::default().fg(dim())),
        ]),
        DiffOp::Removed(s) => Line::from(vec![
            Span::styled(indent, Style::default()),
            Span::styled("- ", Style::default().fg(red()).add_modifier(Modifier::BOLD)),
            Span::styled(s.clone(), Style::default().fg(red())),
        ]),
        DiffOp::Added(s) => Line::from(vec![
            Span::styled(indent, Style::default()),
            Span::styled("+ ", Style::default().fg(green()).add_modifier(Modifier::BOLD)),
            Span::styled(s.clone(), Style::default().fg(white())),
        ]),
    }
}

/// Cuenta agregadas/removidas en un diff — util para headers tipo "+3 -1".
pub fn diff_stats(old: &str, new: &str) -> (usize, usize) {
    let ops = compute_diff(old, new);
    let added = ops.iter().filter(|o| matches!(o, DiffOp::Added(_))).count();
    let removed = ops.iter().filter(|o| matches!(o, DiffOp::Removed(_))).count();
    (added, removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_strings_produce_only_context() {
        let diff = compute_diff("a\nb\nc", "a\nb\nc");
        assert_eq!(diff.len(), 3);
        assert!(diff.iter().all(|op| matches!(op, DiffOp::Context(_))));
    }

    #[test]
    fn empty_strings_produce_empty_diff() {
        assert!(compute_diff("", "").is_empty());
    }

    #[test]
    fn all_added_when_old_empty() {
        let diff = compute_diff("", "x\ny");
        assert_eq!(diff.len(), 2);
        assert!(diff.iter().all(|op| matches!(op, DiffOp::Added(_))));
    }

    #[test]
    fn all_removed_when_new_empty() {
        let diff = compute_diff("x\ny", "");
        assert_eq!(diff.len(), 2);
        assert!(diff.iter().all(|op| matches!(op, DiffOp::Removed(_))));
    }

    #[test]
    fn single_line_replacement() {
        let diff = compute_diff("foo", "bar");
        assert_eq!(diff.len(), 2);
        assert!(matches!(&diff[0], DiffOp::Removed(s) if s == "foo"));
        assert!(matches!(&diff[1], DiffOp::Added(s) if s == "bar"));
    }

    #[test]
    fn mixed_diff_preserves_common_prefix() {
        let diff = compute_diff("a\nb\nc", "a\nX\nc");
        // Debe tener a (context), b (removed), X (added), c (context)
        assert_eq!(diff.len(), 4);
        assert!(matches!(&diff[0], DiffOp::Context(s) if s == "a"));
        assert!(matches!(&diff[3], DiffOp::Context(s) if s == "c"));
    }

    #[test]
    fn diff_stats_counts_correctly() {
        let (added, removed) = diff_stats("a\nb\nc", "a\nX\nY\nc");
        assert_eq!(added, 2); // X, Y
        assert_eq!(removed, 1); // b
    }

    #[test]
    fn collapse_context_zero_removes_context() {
        let ops = compute_diff("a\nb\nc", "a\nX\nc");
        let collapsed = collapse_context(&ops, 0);
        assert_eq!(collapsed.len(), 2); // solo b removed + X added
        assert!(collapsed.iter().all(|op| !matches!(op, DiffOp::Context(_))));
    }

    #[test]
    fn collapse_context_keeps_surrounding_lines() {
        let ops = compute_diff("a\nb\nc\nd\ne", "a\nb\nX\nd\ne");
        let collapsed = collapse_context(&ops, 1);
        // Cambio en c, contexto=1 → b y d se mantienen, pero a y e no.
        assert!(collapsed.len() >= 3);
        assert!(!collapsed.iter().any(|op| matches!(op, DiffOp::Context(s) if s == "a")));
    }

    #[test]
    fn render_respects_max_lines_cap() {
        let old = "1\n2\n3\n4\n5";
        let new = "A\nB\nC\nD\nE";
        let lines = render_diff_lines(
            old,
            new,
            DiffRenderOpts { file_path: None, max_lines: 3, indent: "", context: 0 },
        );
        // 3 lineas mostradas + 1 de "… N mas".
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn render_empty_diff_shows_hint() {
        let lines = render_diff_lines(
            "same",
            "same",
            DiffRenderOpts { file_path: None, max_lines: 20, indent: "  ", context: 0 },
        );
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn word_diff_applied_when_lines_similar() {
        let old = "fn hello() { return 1; }";
        let new = "fn hello() { return 2; }";
        let lines = render_diff_lines(
            old,
            new,
            DiffRenderOpts { file_path: None, max_lines: 20, indent: "", context: 2 },
        );
        assert_eq!(lines.len(), 2);
        // Word-level diff produces more than 3 spans per line.
        assert!(lines[0].spans.len() > 3);
        assert!(lines[1].spans.len() > 3);
    }

    #[test]
    fn line_diff_fallback_when_dissimilar() {
        let old = "completely unique content";
        let new = "nothing alike at all here";
        let lines = render_diff_lines(
            old,
            new,
            DiffRenderOpts { file_path: None, max_lines: 20, indent: "", context: 0 },
        );
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].spans.len(), 3); // indent + "- " + text
        assert_eq!(lines[1].spans.len(), 3); // indent + "+ " + text
    }
}
