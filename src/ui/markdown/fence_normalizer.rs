//! Fence normalization — auto-upgrade outer fences for nested code blocks.
//!
//! When AI generates code blocks that contain other code blocks, the inner
//! fences can prematurely close the outer one. This normalizer detects
//! nesting and upgrades outer fences to use more backticks.
//!
//! Example:
//!   Input:  ```rust\n  ```toml\n  [pkg]\n  ```\n```
//!   Output: ````rust\n  ```toml\n  [pkg]\n  ```\n````

/// Normalize fences so nested code blocks render correctly.
///
/// Scans for ``` fence opens and ensures the outermost fence uses more
/// backticks than any inner fence. Returns content with fixed fences.
pub fn normalize_fences(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::with_capacity(content.len() + 32);
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();

        if let Some(outer_ticks) = opening_fence(trimmed) {
            // Scan ahead: find the max inner fence and the LAST closing fence
            let (max_inner, close_idx) = scan_block(&lines, i + 1, outer_ticks);

            if max_inner >= outer_ticks {
                let new_ticks = max_inner + 1;
                let new_fence = "`".repeat(new_ticks);
                let suffix = &trimmed[outer_ticks..]; // language tag
                let ws = &line[..line.len() - trimmed.len()];

                // Write upgraded opening fence
                result.push_str(ws);
                result.push_str(&new_fence);
                result.push_str(suffix);
                result.push('\n');

                // Copy inner lines verbatim
                for inner_line in &lines[i + 1..close_idx] {
                    result.push_str(inner_line);
                    result.push('\n');
                }

                // Write upgraded closing fence
                let close_line = lines[close_idx];
                let close_ws = &close_line[..close_line.len() - close_line.trim_start().len()];
                result.push_str(close_ws);
                result.push_str(&new_fence);
                result.push('\n');
                i = close_idx + 1;
            } else {
                result.push_str(line);
                result.push('\n');
                i += 1;
            }
        } else {
            result.push_str(line);
            result.push('\n');
            i += 1;
        }
    }

    // Remove trailing newline if original didn't have one
    if !content.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }
    result
}

/// Check if line is an opening fence (3+ backticks, may have language tag).
/// Returns tick count if it's a fence opening.
fn opening_fence(line: &str) -> Option<usize> {
    let count = line.chars().take_while(|&c| c == '`').count();
    // Must be 3+ ticks. An opening fence may have a language tag after.
    if count >= 3 {
        Some(count)
    } else {
        None
    }
}

/// Check if a line is a closing fence (only backticks, no tag).
fn is_closing_fence(line: &str, min_ticks: usize) -> bool {
    let ticks = line.chars().take_while(|&c| c == '`').count();
    ticks >= min_ticks && line[ticks..].trim().is_empty()
}

/// Scan the block after an opening fence. Returns (max_inner_ticks, close_line_idx).
///
/// The closing fence is the LAST line with `outer_ticks`+ backticks and empty rest,
/// since AI-generated content may have inner fences at the same tick count.
fn scan_block(lines: &[&str], start: usize, outer_ticks: usize) -> (usize, usize) {
    let mut max_inner = 0;
    let mut last_close = start; // default: line after open (degenerate case)

    for (offset, line) in lines[start..].iter().enumerate() {
        let trimmed = line.trim_start();
        if is_closing_fence(trimmed, outer_ticks) {
            last_close = start + offset;
        } else if let Some(count) = opening_fence(trimmed) {
            max_inner = max_inner.max(count);
        }
    }

    (max_inner, last_close)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_nesting_unchanged() {
        let input = "```rust\nfn main() {}\n```";
        assert_eq!(normalize_fences(input), input);
    }

    #[test]
    fn nested_fences_upgraded() {
        let input = "```markdown\n```rust\ncode\n```\n```";
        let output = normalize_fences(input);
        assert!(output.starts_with("````markdown"), "got: {output}");
        assert!(output.ends_with("````"), "got: {output}");
    }

    #[test]
    fn deeply_nested() {
        let input = "```md\n```rust\n````inner\ndeep\n````\n```\n```";
        let output = normalize_fences(input);
        // Outer should be upgraded beyond the max inner (4 ticks)
        assert!(output.starts_with("`````md"), "got: {output}");
    }

    #[test]
    fn preserves_language_tag() {
        let input = "```rust\n```toml\n[pkg]\n```\n```";
        let output = normalize_fences(input);
        assert!(output.contains("rust"));
    }

    #[test]
    fn no_fences_passthrough() {
        let input = "Just plain text\nwith lines";
        assert_eq!(normalize_fences(input), input);
    }
}
