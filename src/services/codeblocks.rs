//! Code block detection and extraction from AI responses.
//!
//! Detects fenced code blocks (```lang path) in assistant messages
//! and extracts them as actionable items the user can apply to files.

/// A detected code block with optional file path.
#[derive(Debug, Clone)]
pub struct CodeBlock {
    /// Language hint (e.g., "rust", "typescript").
    pub lang: String,
    /// File path if detected (e.g., "src/main.rs").
    pub file_path: Option<String>,
    /// The code content (without fences).
    pub content: String,
    /// Line index in the message where the block starts.
    #[expect(dead_code, reason = "used for future code block highlighting in UI")]
    pub start_line: usize,
}

/// Extract code blocks from a markdown message.
/// Recognizes patterns like:
///   ```rust src/main.rs
///   ```rust
///   // src/main.rs
///   content...
///   ```
pub fn extract_code_blocks(text: &str) -> Vec<CodeBlock> {
    let mut blocks = Vec::new();
    let mut in_block = false;
    let mut current_lang = String::new();
    let mut current_path: Option<String> = None;
    let mut current_content = String::new();
    let mut start_line = 0;

    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();

        if !in_block && trimmed.starts_with("```") {
            in_block = true;
            start_line = i;
            current_content.clear();

            let meta = trimmed.trim_start_matches('`');
            let parts: Vec<&str> = meta.split_whitespace().collect();

            current_lang = parts.first().unwrap_or(&"").to_string();
            current_path =
                parts.get(1).and_then(
                    |p| {
                        if looks_like_path(p) {
                            Some(p.to_string())
                        } else {
                            None
                        }
                    },
                );
        } else if in_block && trimmed == "```" {
            in_block = false;

            // Try to detect path from first comment line if not found in fence
            if current_path.is_none() {
                current_path = detect_path_from_comment(&current_content);
            }

            if !current_content.is_empty() {
                blocks.push(CodeBlock {
                    lang: current_lang.clone(),
                    file_path: current_path.take(),
                    content: current_content.clone(),
                    start_line,
                });
            }
        } else if in_block {
            if !current_content.is_empty() {
                current_content.push('\n');
            }
            current_content.push_str(line);
        }
    }

    blocks
}

/// Check if a string looks like a file path.
fn looks_like_path(s: &str) -> bool {
    // Must contain a dot or slash, not be a known non-path token
    (s.contains('/') || s.contains('.'))
        && !s.starts_with("http")
        && !s.starts_with("--")
        && s.len() < 200
}

/// Try to detect a file path from a comment in the first line of code.
/// Patterns: `// src/main.rs`, `# config.py`, `<!-- index.html -->`
fn detect_path_from_comment(content: &str) -> Option<String> {
    let first_line = content.lines().next()?.trim();

    // // path or # path
    let candidate = if let Some(rest) = first_line.strip_prefix("//") {
        rest.trim()
    } else if let Some(rest) = first_line.strip_prefix('#') {
        // Only if it looks like a comment, not a heading
        let rest = rest.trim();
        if rest.starts_with(' ') || !rest.contains(' ') {
            rest.trim()
        } else {
            return None;
        }
    } else {
        return None;
    };

    if looks_like_path(candidate) && !candidate.contains(' ') {
        Some(candidate.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_should_find_block_with_path_in_fence() {
        let text = "```rust src/main.rs\nfn main() {}\n```";
        let blocks = extract_code_blocks(text);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].lang, "rust");
        assert_eq!(blocks[0].file_path.as_deref(), Some("src/main.rs"));
        assert_eq!(blocks[0].content, "fn main() {}");
    }

    #[test]
    fn extract_should_find_block_with_path_in_comment() {
        let text = "```rust\n// src/lib.rs\nfn hello() {}\n```";
        let blocks = extract_code_blocks(text);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].file_path.as_deref(), Some("src/lib.rs"));
    }

    #[test]
    fn extract_should_handle_block_without_path() {
        let text = "```rust\nfn hello() {}\n```";
        let blocks = extract_code_blocks(text);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].file_path, None);
    }

    #[test]
    fn extract_should_find_multiple_blocks() {
        let text = "```rust\nfn a() {}\n```\ntext\n```ts src/app.ts\nconst x = 1;\n```";
        let blocks = extract_code_blocks(text);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[1].file_path.as_deref(), Some("src/app.ts"));
    }
}
