/// Domain types for the `/doctor` diagnostic command.
/// Each `DoctorCheck` represents one diagnostic with a semaphore status.

#[derive(Debug, Clone, PartialEq)]
pub enum CheckStatus {
    Green,
    Yellow,
    Red,
}

impl CheckStatus {
    pub fn glyph(&self) -> &'static str {
        match self {
            Self::Green => "✓",
            Self::Yellow => "⚠",
            Self::Red => "✗",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Green => "OK",
            Self::Yellow => "Warning",
            Self::Red => "Error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DoctorCheck {
    pub name: &'static str,
    pub status: CheckStatus,
    pub detail: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DoctorReport {
    pub checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    /// Overall status: worst of all checks.
    pub fn overall(&self) -> CheckStatus {
        if self.checks.iter().any(|c| c.status == CheckStatus::Red) {
            CheckStatus::Red
        } else if self.checks.iter().any(|c| c.status == CheckStatus::Yellow) {
            CheckStatus::Yellow
        } else {
            CheckStatus::Green
        }
    }
}
