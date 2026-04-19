//! Word-level (intra-line) diff highlighting (E41 completion).
//!
//! When a Removed+Added pair is "similar enough", we tokenize both lines
//! and run LCS on the tokens to highlight only the changed words.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use super::theme::{green, red, white};

/// Word-level token: either unchanged or changed text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum WordOp {
    Same(String),
    Changed(String),
}

/// Background colors for highlighting changed words.
const REMOVED_BG: ratatui::style::Color = ratatui::style::Color::Rgb(80, 20, 20);
const ADDED_BG: ratatui::style::Color = ratatui::style::Color::Rgb(20, 60, 20);

/// Splits a line into tokens at whitespace and punctuation boundaries.
fn tokenize_words(s: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start = 0;
    let mut in_word = false;
    for (i, ch) in s.char_indices() {
        let is_boundary = ch.is_whitespace()
            || matches!(ch, '(' | ')' | '{' | '}' | '[' | ']' | ',' | ';' | ':' | '.' | '"' | '\'');
        if !in_word && !is_boundary {
            start = i;
            in_word = true;
        } else if in_word && is_boundary {
            tokens.push(&s[start..i]);
            tokens.push(&s[i..i + ch.len_utf8()]);
            in_word = false;
        } else if !in_word {
            tokens.push(&s[i..i + ch.len_utf8()]);
        }
    }
    if in_word {
        tokens.push(&s[start..]);
    }
    tokens
}

/// Compute word-level diff between two lines using LCS on tokens.
pub(super) fn compute_word_diff(old: &str, new: &str) -> (Vec<WordOp>, Vec<WordOp>) {
    let a = tokenize_words(old);
    let b = tokenize_words(new);
    let n = a.len();
    let m = b.len();

    let mut lcs = vec![vec![0u16; m + 1]; n + 1];
    for i in 0..n {
        for j in 0..m {
            lcs[i + 1][j + 1] =
                if a[i] == b[j] { lcs[i][j] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }

    let mut old_ops: Vec<WordOp> = Vec::new();
    let mut new_ops: Vec<WordOp> = Vec::new();
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && a[i - 1] == b[j - 1] {
            old_ops.push(WordOp::Same(a[i - 1].to_string()));
            new_ops.push(WordOp::Same(b[j - 1].to_string()));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || lcs[i][j - 1] >= lcs[i - 1][j]) {
            new_ops.push(WordOp::Changed(b[j - 1].to_string()));
            j -= 1;
        } else {
            old_ops.push(WordOp::Changed(a[i - 1].to_string()));
            i -= 1;
        }
    }
    old_ops.reverse();
    new_ops.reverse();
    (old_ops, new_ops)
}

/// Render a removed line with word-level highlights.
pub(super) fn render_word_removed(ops: &[WordOp], indent: &'static str) -> Line<'static> {
    let mut spans = vec![
        Span::styled(indent, Style::default()),
        Span::styled("- ", Style::default().fg(red()).add_modifier(Modifier::BOLD)),
    ];
    for op in ops {
        match op {
            WordOp::Same(s) => spans.push(Span::styled(s.clone(), Style::default().fg(red()))),
            WordOp::Changed(s) => spans.push(Span::styled(
                s.clone(),
                Style::default().fg(red()).bg(REMOVED_BG).add_modifier(Modifier::BOLD),
            )),
        }
    }
    Line::from(spans)
}

/// Render an added line with word-level highlights.
pub(super) fn render_word_added(ops: &[WordOp], indent: &'static str) -> Line<'static> {
    let mut spans = vec![
        Span::styled(indent, Style::default()),
        Span::styled("+ ", Style::default().fg(green()).add_modifier(Modifier::BOLD)),
    ];
    for op in ops {
        match op {
            WordOp::Same(s) => spans.push(Span::styled(s.clone(), Style::default().fg(white()))),
            WordOp::Changed(s) => spans.push(Span::styled(
                s.clone(),
                Style::default().fg(green()).bg(ADDED_BG).add_modifier(Modifier::BOLD),
            )),
        }
    }
    Line::from(spans)
}

/// Check if two lines are "similar enough" to warrant word-level diff.
/// Heuristic: at least 30% of the shorter line's non-whitespace tokens match.
pub(super) fn lines_similar_enough(old: &str, new: &str) -> bool {
    let a: Vec<&str> = tokenize_words(old).into_iter().filter(|t| !t.trim().is_empty()).collect();
    let b: Vec<&str> = tokenize_words(new).into_iter().filter(|t| !t.trim().is_empty()).collect();
    if a.is_empty() || b.is_empty() {
        return false;
    }
    let common: usize = a.iter().filter(|t| b.contains(t)).count();
    let min_len = a.len().min(b.len());
    common * 100 / min_len >= 30
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_words_splits_on_whitespace_and_punctuation() {
        let tokens = tokenize_words("fn foo(bar: i32)");
        assert!(tokens.contains(&"fn"));
        assert!(tokens.contains(&"foo"));
        assert!(tokens.contains(&"bar"));
        assert!(tokens.contains(&"i32"));
    }

    #[test]
    fn tokenize_words_empty_string() {
        assert!(tokenize_words("").is_empty());
    }

    #[test]
    fn compute_word_diff_finds_changed_tokens() {
        let (old_ops, new_ops) = compute_word_diff("let x = 10;", "let x = 20;");
        assert!(old_ops.iter().any(|op| matches!(op, WordOp::Changed(s) if s == "10")));
        assert!(new_ops.iter().any(|op| matches!(op, WordOp::Changed(s) if s == "20")));
        assert!(old_ops.iter().any(|op| matches!(op, WordOp::Same(s) if s == "let")));
    }

    #[test]
    fn compute_word_diff_all_different() {
        let (old_ops, new_ops) = compute_word_diff("aaa", "zzz");
        assert!(old_ops.iter().all(|op| matches!(op, WordOp::Changed(_))));
        assert!(new_ops.iter().all(|op| matches!(op, WordOp::Changed(_))));
    }

    #[test]
    fn lines_similar_enough_detects_partial_match() {
        assert!(lines_similar_enough("let x = 10;", "let x = 20;"));
        assert!(!lines_similar_enough("completely different", "nothing alike here"));
    }

    #[test]
    fn lines_similar_empty_is_not_similar() {
        assert!(!lines_similar_enough("", "something"));
        assert!(!lines_similar_enough("something", ""));
    }
}
