use std::fs;
use std::path::Path;

// Re-export domain types for backward compatibility
pub use crate::domain::init_types::{InitClient, InitFileResult, ProjectType};

// Re-export file generation functions from sibling module
pub use crate::services::init_gen::run_init;

// ── Detección de proyecto ────────────────────────────────────────────────────

pub fn detect_project_type(dir: &Path) -> ProjectType {
    let is_net = has_file_with_ext(dir, ".sln")
        || has_file_with_ext(dir, ".csproj")
        || dir.join("Program.cs").exists();

    let is_ang = dir.join("angular.json").exists() || has_angular_in_package_json(dir);

    let is_nest = dir.join("nest-cli.json").exists() || has_nestjs_in_package_json(dir);

    // Multiple frameworks detected → Full Stack (Orchestrator)
    let detected_count = is_net as u8 + is_ang as u8 + is_nest as u8;
    if detected_count > 1 {
        return ProjectType::FullStack;
    }
    match (is_net, is_ang, is_nest) {
        (true, _, _) => ProjectType::Net,
        (_, true, _) => ProjectType::Ang,
        (_, _, true) => ProjectType::Nest,
        _ => ProjectType::Unknown,
    }
}

fn has_file_with_ext(dir: &Path, ext: &str) -> bool {
    fs::read_dir(dir)
        .ok()
        .map(|entries| {
            entries.filter_map(|e| e.ok()).any(|e| e.file_name().to_string_lossy().ends_with(ext))
        })
        .unwrap_or(false)
}

fn has_angular_in_package_json(dir: &Path) -> bool {
    let pkg_path = dir.join("package.json");
    let Ok(content) = fs::read_to_string(pkg_path) else {
        return false;
    };
    content.contains("@angular/core")
}

fn has_nestjs_in_package_json(dir: &Path) -> bool {
    let pkg_path = dir.join("package.json");
    let Ok(content) = fs::read_to_string(pkg_path) else {
        return false;
    };
    content.contains("@nestjs/core")
}

// Note: Extended tech detection has moved to autoskill_map.rs.
// This module retains detect_project_type() for factory selection.
