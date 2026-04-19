/// Config validation with Levenshtein-based typo detection.
///
/// Validates `~/.config/ingenieria-tui/config.json` for unknown keys
/// and suggests corrections using string similarity.
use std::path::PathBuf;

/// Known valid keys for `config.json`.
const VALID_CONFIG_KEYS: &[&str] =
    &["server_url", "developer", "provider", "model", "default_factory", "last_sync_date"];

/// Known valid keys for `keybindings.json`.
const VALID_KB_KEYS: &[&str] =
    &["toggle_sidebar", "search", "command_palette", "copy", "factory_switch"];

#[derive(Debug, Clone)]
pub struct ConfigWarning {
    pub file: &'static str,
    pub key: String,
    pub suggestion: Option<String>,
}

impl std::fmt::Display for ConfigWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref s) = self.suggestion {
            write!(f, "{}: unknown key '{}' — did you mean '{s}'?", self.file, self.key)
        } else {
            write!(f, "{}: unknown key '{}'", self.file, self.key)
        }
    }
}

/// Validate config files and return any warnings (unknown keys with suggestions).
pub fn validate_config_files() -> Vec<ConfigWarning> {
    let mut warnings = Vec::new();
    if let Some(dir) = dirs::config_dir().map(|d| d.join("ingenieria-tui")) {
        check_json_keys(&dir.join("config.json"), "config.json", VALID_CONFIG_KEYS, &mut warnings);
        check_json_keys(
            &dir.join("keybindings.json"),
            "keybindings.json",
            VALID_KB_KEYS,
            &mut warnings,
        );
    }
    warnings
}

fn check_json_keys(
    path: &PathBuf,
    file_label: &'static str,
    valid: &[&str],
    out: &mut Vec<ConfigWarning>,
) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let map: serde_json::Map<String, serde_json::Value> = match serde_json::from_str(&content) {
        Ok(serde_json::Value::Object(m)) => m,
        _ => return,
    };
    for key in map.keys() {
        if !valid.contains(&key.as_str()) {
            let suggestion = find_closest(key, valid);
            out.push(ConfigWarning { file: file_label, key: key.clone(), suggestion });
        }
    }
}

/// Find the closest valid key using Levenshtein distance (max distance 3).
fn find_closest(input: &str, candidates: &[&str]) -> Option<String> {
    let mut best: Option<(&str, usize)> = None;
    for &candidate in candidates {
        let dist = strsim::levenshtein(input, candidate);
        if dist <= 3 && (best.is_none() || dist < best.as_ref().map_or(usize::MAX, |b| b.1)) {
            best = Some((candidate, dist));
        }
    }
    best.map(|(s, _)| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_closest_exact_match() {
        assert_eq!(find_closest("server_url", VALID_CONFIG_KEYS), Some("server_url".into()));
    }

    #[test]
    fn find_closest_typo() {
        assert_eq!(find_closest("servr_url", VALID_CONFIG_KEYS), Some("server_url".into()));
        assert_eq!(find_closest("develper", VALID_CONFIG_KEYS), Some("developer".into()));
    }

    #[test]
    fn find_closest_no_match() {
        assert_eq!(find_closest("completely_wrong_key_name", VALID_CONFIG_KEYS), None);
    }
}
