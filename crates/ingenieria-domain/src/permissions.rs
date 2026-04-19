//! Permission-related types shared across crates.

/// Permission level assigned to a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPermission {
    /// Read-only operations, always allowed.
    Safe,
    /// Write operations, require user confirmation.
    Ask,
    /// Potentially dangerous operations, require explicit approval.
    Dangerous,
}

/// Result of bash command validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BashValidation {
    /// Command is safe to execute without prompting.
    Allow,
    /// Command needs user confirmation (show reasons).
    Warn { reasons: Vec<String> },
    /// Command is blocked outright.
    Block { reasons: Vec<String> },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bash_validation_warn_carries_reasons() {
        let v = BashValidation::Warn { reasons: vec!["destructive".into()] };
        if let BashValidation::Warn { reasons } = v {
            assert_eq!(reasons.len(), 1);
        } else {
            panic!("expected Warn");
        }
    }
}
