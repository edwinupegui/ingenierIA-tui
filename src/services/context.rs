//! Smart context collector for enriching AI conversations.
//!
//! Gathers git diff, recently modified files, and compilation errors
//! from the local project to give the AI better situational awareness.

use std::path::Path;
use std::process::Command;

/// Maximum bytes of git diff to include in context.
const MAX_DIFF_BYTES: usize = 12_000;
/// Maximum number of recent files to list.
const MAX_RECENT_FILES: usize = 20;
/// Maximum bytes of compiler output to include.
const MAX_COMPILER_BYTES: usize = 8_000;

/// Collected smart context from the local project.
pub struct SmartContext {
    pub git_diff: Option<String>,
    pub recent_files: Vec<String>,
    pub compiler_errors: Option<String>,
    pub git_branch: Option<String>,
}

impl SmartContext {
    /// Returns true if there is any meaningful context to inject.
    pub fn is_empty(&self) -> bool {
        self.git_diff.is_none()
            && self.recent_files.is_empty()
            && self.compiler_errors.is_none()
            && self.git_branch.is_none()
    }

    /// Format as markdown section for system prompt injection.
    pub fn to_markdown(&self) -> String {
        if self.is_empty() {
            return String::new();
        }

        let mut out = String::with_capacity(2048);
        out.push_str("## Contexto del proyecto local\n\n");

        if let Some(branch) = &self.git_branch {
            out.push_str(&format!("**Branch**: `{branch}`\n\n"));
        }

        if !self.recent_files.is_empty() {
            out.push_str("### Archivos modificados recientemente\n\n");
            for f in &self.recent_files {
                out.push_str(&format!("- `{f}`\n"));
            }
            out.push('\n');
        }

        if let Some(diff) = &self.git_diff {
            out.push_str("### Git diff (cambios sin commitear)\n\n```diff\n");
            out.push_str(diff);
            out.push_str("\n```\n\n");
        }

        if let Some(errors) = &self.compiler_errors {
            out.push_str("### Errores de compilacion\n\n```\n");
            out.push_str(errors);
            out.push_str("\n```\n\n");
        }

        out
    }
}

/// Collect all available smart context from the given project directory.
pub fn collect(project_dir: &Path) -> SmartContext {
    SmartContext {
        git_branch: git_branch(project_dir),
        git_diff: git_diff(project_dir),
        recent_files: recent_files(project_dir),
        compiler_errors: compiler_errors(project_dir),
    }
}

/// Get just the git diff (for /diff slash command).
pub fn git_diff_only(project_dir: &Path) -> Option<String> {
    git_diff(project_dir)
}

fn git_branch(dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch.is_empty() {
        None
    } else {
        Some(branch)
    }
}

fn git_diff(dir: &Path) -> Option<String> {
    // Staged + unstaged changes
    let output = Command::new("git")
        .args(["diff", "HEAD", "--stat", "--patch", "--no-color"])
        .current_dir(dir)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let diff = String::from_utf8_lossy(&output.stdout);
    let diff = diff.trim();

    if diff.is_empty() {
        return None;
    }

    Some(truncate_to_bytes(diff, MAX_DIFF_BYTES))
}

fn recent_files(dir: &Path) -> Vec<String> {
    // Files modified in the last 5 commits
    let output = Command::new("git")
        .args(["log", "--oneline", "--name-only", "-5", "--pretty=format:"])
        .current_dir(dir)
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut seen = std::collections::HashSet::new();
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| seen.insert(l.to_string()))
        .take(MAX_RECENT_FILES)
        .map(String::from)
        .collect()
}

fn compiler_errors(dir: &Path) -> Option<String> {
    let output = Command::new("cargo")
        .args(["check", "--message-format=short", "--color=never"])
        .current_dir(dir)
        .output()
        .ok()?;

    // Only include if there are actual errors (non-zero exit)
    if output.status.success() {
        return None;
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let errors: String = stderr
        .lines()
        .filter(|l| l.contains("error") || l.contains("Error"))
        .collect::<Vec<_>>()
        .join("\n");

    if errors.is_empty() {
        return None;
    }

    Some(truncate_to_bytes(&errors, MAX_COMPILER_BYTES))
}

fn truncate_to_bytes(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }

    // Find a safe UTF-8 boundary
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }

    let mut result = s[..end].to_string();
    result.push_str("\n... (truncado)");
    result
}
