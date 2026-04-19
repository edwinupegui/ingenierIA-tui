// ── Paste Handler — Clasificacion y referencias compactas para pastes grandes ─

/// Pastes above this threshold get a compact placeholder in the input.
pub const LARGE_PASTE_THRESHOLD: usize = 5120; // 5 KB

/// Stored reference for a large paste. Expanded back on send.
pub struct PastedRef {
    pub id: usize,
    pub full_text: String,
    pub line_count: usize,
}

pub struct PasteClassification {
    pub is_large: bool,
    pub line_count: usize,
}

/// Classify a paste by size and line count.
pub fn classify(text: &str) -> PasteClassification {
    PasteClassification {
        is_large: text.len() >= LARGE_PASTE_THRESHOLD,
        line_count: text.lines().count().max(1),
    }
}

/// Build the placeholder shown in the input for a large paste.
pub fn make_placeholder(id: usize, line_count: usize) -> String {
    format!("[Pegado #{id} +{line_count} lineas]")
}

/// Replace all paste placeholders with their full text before sending.
pub fn expand_placeholders(input: &str, refs: &[PastedRef]) -> String {
    let mut result = input.to_string();
    for r in refs {
        let placeholder = make_placeholder(r.id, r.line_count);
        if result.contains(&placeholder) {
            result = result.replace(&placeholder, &r.full_text);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_small() {
        let text = "hello world";
        let c = classify(text);
        assert!(!c.is_large);
        assert_eq!(c.line_count, 1);
    }

    #[test]
    fn classify_large() {
        let text = "x".repeat(LARGE_PASTE_THRESHOLD + 1);
        let c = classify(&text);
        assert!(c.is_large);
    }

    #[test]
    fn placeholder_format() {
        assert_eq!(make_placeholder(1, 47), "[Pegado #1 +47 lineas]");
        assert_eq!(make_placeholder(3, 1), "[Pegado #3 +1 lineas]");
    }

    #[test]
    fn expand_single() {
        let refs = vec![PastedRef { id: 1, full_text: "BIG TEXT".to_string(), line_count: 5 }];
        let input = "check this [Pegado #1 +5 lineas] please";
        let result = expand_placeholders(input, &refs);
        assert_eq!(result, "check this BIG TEXT please");
    }

    #[test]
    fn expand_multiple() {
        let refs = vec![
            PastedRef { id: 1, full_text: "AAA".to_string(), line_count: 2 },
            PastedRef { id: 2, full_text: "BBB".to_string(), line_count: 3 },
        ];
        let input = "[Pegado #1 +2 lineas] and [Pegado #2 +3 lineas]";
        let result = expand_placeholders(input, &refs);
        assert_eq!(result, "AAA and BBB");
    }

    #[test]
    fn expand_no_refs() {
        let result = expand_placeholders("plain text", &[]);
        assert_eq!(result, "plain text");
    }
}
