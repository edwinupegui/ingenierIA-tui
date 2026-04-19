//! Terminal hyperlinks (OSC 8) and clipboard (OSC 52) utilities.
//!
//! OSC 8: `\x1b]8;params;URI\x07TEXT\x1b]8;;\x07` — clickable hyperlinks.
//! OSC 52: `\x1b]52;c;BASE64\x07` — write to clipboard without arboard.
//!
//! Not all terminals support these; detection is best-effort via env vars.

#![allow(dead_code)]

use std::io::Write;

/// Detect whether the current terminal likely supports OSC 8 hyperlinks.
///
/// Checks TERM_PROGRAM and known terminal identifiers.
pub fn terminal_supports_hyperlinks() -> bool {
    if let Ok(prog) = std::env::var("TERM_PROGRAM") {
        let lower = prog.to_lowercase();
        return matches!(
            lower.as_str(),
            "iterm.app" | "iterm2" | "wezterm" | "hyper" | "mintty" | "contour" | "rio" | "ghostty"
        );
    }
    // kitty announces itself via KITTY_WINDOW_ID
    if std::env::var("KITTY_WINDOW_ID").is_ok() {
        return true;
    }
    // WT_SESSION indicates Windows Terminal
    if std::env::var("WT_SESSION").is_ok() {
        return true;
    }
    false
}

/// Format a string as an OSC 8 hyperlink (for use in raw terminal output).
///
/// Returns `text` unchanged if hyperlinks are not supported.
pub fn osc8_link(url: &str, text: &str) -> String {
    if terminal_supports_hyperlinks() {
        format!("\x1b]8;;{url}\x07{text}\x1b]8;;\x07")
    } else {
        text.to_string()
    }
}

/// Write text to the system clipboard using OSC 52 escape sequence.
///
/// This works over SSH and in terminals that support OSC 52 (most modern ones).
/// Falls back silently if writing fails.
pub fn osc52_copy(text: &str) -> std::io::Result<()> {
    let encoded = simple_base64_encode(text.as_bytes());
    let mut stdout = std::io::stdout().lock();
    write!(stdout, "\x1b]52;c;{encoded}\x07")?;
    stdout.flush()
}

/// Minimal base64 encoder (avoids adding base64 crate dependency).
fn simple_base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Simple URL detection: finds http/https/ingenieria:// URLs in text.
///
/// Returns (start_byte, end_byte, url) tuples.
pub fn detect_urls(text: &str) -> Vec<(usize, usize, &str)> {
    let prefixes = ["https://", "http://", "ingenieria://"];
    let mut results = Vec::new();
    let mut search_from = 0;

    while search_from < text.len() {
        let remaining = &text[search_from..];
        let mut earliest: Option<(usize, &str)> = None;

        for prefix in &prefixes {
            if let Some(pos) = remaining.find(prefix) {
                if earliest.is_none() || pos < earliest.unwrap().0 {
                    earliest = Some((pos, prefix));
                }
            }
        }

        let Some((rel_pos, _)) = earliest else {
            break;
        };

        let abs_start = search_from + rel_pos;
        // URL ends at whitespace, ), ], or end of string
        let url_slice = &text[abs_start..];
        let end_offset = url_slice
            .find(|c: char| c.is_whitespace() || c == ')' || c == ']' || c == '>' || c == '"')
            .unwrap_or(url_slice.len());
        let abs_end = abs_start + end_offset;

        results.push((abs_start, abs_end, &text[abs_start..abs_end]));
        search_from = abs_end;
    }

    results
}
