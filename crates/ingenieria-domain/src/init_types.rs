/// Project type detected from the filesystem structure.
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectType {
    Net,
    Ang,
    Nest,
    FullStack,
    Unknown,
}

impl ProjectType {
    pub fn label(&self) -> &'static str {
        match self {
            ProjectType::Net => ".NET",
            ProjectType::Ang => "Angular",
            ProjectType::Nest => "NestJS",
            ProjectType::FullStack => "Full Stack (Orchestrator)",
            ProjectType::Unknown => "unknown",
        }
    }

    /// Selectable project types (excludes Unknown — resolved by factory fallback).
    pub const ALL: &[ProjectType] =
        &[ProjectType::Net, ProjectType::Ang, ProjectType::Nest, ProjectType::FullStack];
}

/// AI client option for project initialization.
#[derive(Debug, Clone, PartialEq)]
pub enum InitClient {
    Claude,
    Copilot,
    Both,
}

impl InitClient {
    pub fn label(&self) -> &'static str {
        match self {
            InitClient::Claude => "Claude Code",
            InitClient::Copilot => "GitHub Copilot",
            InitClient::Both => "Ambos (Claude + Copilot)",
        }
    }

    pub const ALL: &[InitClient] = &[InitClient::Claude, InitClient::Copilot, InitClient::Both];
}

/// Result of creating a single file during init.
#[derive(Debug, Clone)]
pub struct InitFileResult {
    pub path: String,
    pub created: bool,
    pub description: String,
}
