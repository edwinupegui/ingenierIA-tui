//! Toast notification severity level.

use crate::failure::Severity;

/// Severity level for toast notifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl From<Severity> for ToastLevel {
    fn from(sev: Severity) -> Self {
        match sev {
            Severity::Info => ToastLevel::Info,
            Severity::Warning => ToastLevel::Warning,
            Severity::Error | Severity::Critical => ToastLevel::Error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_maps_to_toast_level() {
        assert_eq!(ToastLevel::from(Severity::Critical), ToastLevel::Error);
        assert_eq!(ToastLevel::from(Severity::Warning), ToastLevel::Warning);
        assert_eq!(ToastLevel::from(Severity::Info), ToastLevel::Info);
    }
}
