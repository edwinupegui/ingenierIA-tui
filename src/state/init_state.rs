// ── Init project ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum InitStep {
    SelectType,
    SelectClient,
    Confirm,
    Running,
    Done,
}

pub struct InitState {
    pub step: InitStep,
    pub detected_type: crate::services::init::ProjectType,
    pub type_cursor: usize,
    pub client_cursor: usize,
    pub project_dir: String,
    pub results: Vec<crate::services::init::InitFileResult>,
    pub error: Option<String>,
}

impl InitState {
    pub fn new() -> Self {
        Self {
            step: InitStep::SelectType,
            detected_type: crate::services::init::ProjectType::Unknown,
            type_cursor: 0,
            client_cursor: 0,
            project_dir: String::new(),
            results: Vec::new(),
            error: None,
        }
    }
}
