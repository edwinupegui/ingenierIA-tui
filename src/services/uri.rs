//! ingenierIA URI parser and navigation.
//!
//! URI format: `ingenieria://type/factory/name`
//! Examples:
//!   - `ingenieria://skill/net/add-feature`
//!   - `ingenieria://policy/ang/naming-conventions`
//!   - `ingenieria://adr/nest/001-api-versioning`

/// Parsed ingenierIA URI components.
#[derive(Debug, Clone, PartialEq)]
pub struct IngenieriaUri {
    pub doc_type: String,
    pub factory: String,
    pub name: String,
}

impl IngenieriaUri {
    /// Format back to URI string.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn to_uri(&self) -> String {
        format!("ingenieria://{}/{}/{}", self.doc_type, self.factory, self.name)
    }
}

/// Parse a `ingenieria://type/factory/name` URI.
/// Returns None if the URI doesn't match the expected format.
pub fn parse(uri: &str) -> Option<IngenieriaUri> {
    let rest = uri.strip_prefix("ingenieria://")?;
    let parts: Vec<&str> = rest.splitn(3, '/').collect();

    if parts.len() < 3 {
        return None;
    }

    let doc_type = parts[0].trim();
    let factory = parts[1].trim();
    let name = parts[2].trim();

    if doc_type.is_empty() || factory.is_empty() || name.is_empty() {
        return None;
    }

    Some(IngenieriaUri {
        doc_type: doc_type.to_string(),
        factory: factory.to_string(),
        name: name.to_string(),
    })
}

/// Extract all ingenieria:// URIs from a text string.
#[cfg_attr(not(test), allow(dead_code))]
pub fn extract_uris(text: &str) -> Vec<IngenieriaUri> {
    text.split_whitespace()
        .filter(|word| word.starts_with("ingenieria://"))
        .filter_map(|word| {
            // Strip trailing punctuation
            let clean = word.trim_end_matches([',', '.', ')', ']']);
            parse(clean)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_should_extract_valid_uri() {
        let uri = parse("ingenieria://skill/net/add-feature");
        assert_eq!(
            uri,
            Some(IngenieriaUri {
                doc_type: "skill".into(),
                factory: "net".into(),
                name: "add-feature".into(),
            })
        );
    }

    #[test]
    fn parse_should_reject_invalid_uri() {
        assert_eq!(parse("http://example.com"), None);
        assert_eq!(parse("ingenieria://"), None);
        assert_eq!(parse("ingenieria://skill"), None);
        assert_eq!(parse("ingenieria://skill/net"), None);
    }

    #[test]
    fn extract_should_find_uris_in_text() {
        let text = "Revisa ingenieria://policy/net/naming y ingenieria://adr/net/001-api.";
        let uris = extract_uris(text);
        assert_eq!(uris.len(), 2);
        assert_eq!(uris[0].name, "naming");
        assert_eq!(uris[1].name, "001-api");
    }

    #[test]
    fn to_uri_should_roundtrip() {
        let original = "ingenieria://skill/net/add-feature";
        let parsed = parse(original).unwrap();
        assert_eq!(parsed.to_uri(), original);
    }
}
