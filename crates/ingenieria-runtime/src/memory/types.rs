//! Tipos de memoria persistente (E15).
//!
//! 4 categorias inspiradas en claude-code `src/memdir/memoryTypes.ts`:
//! - `user`: rol/preferencias del usuario
//! - `feedback`: correcciones y confirmaciones ("no hagas X", "si sigue asi")
//! - `project`: contexto de trabajo en curso (quien hace que, por que)
//! - `reference`: punteros a sistemas externos (Linear, Grafana, etc.)

use serde::{Deserialize, Serialize};

/// Categoria de memoria. Serializa como snake_case en frontmatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    User,
    Feedback,
    Project,
    Reference,
}

impl MemoryType {
    /// Todas las variantes en orden canonico.
    #[allow(dead_code, reason = "API publica reservada para UI de filtros/autocomplete")]
    pub fn all() -> &'static [MemoryType] {
        &[Self::User, Self::Feedback, Self::Project, Self::Reference]
    }

    /// Label para UI y frontmatter (`user`, `feedback`, `project`, `reference`).
    pub fn label(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Feedback => "feedback",
            Self::Project => "project",
            Self::Reference => "reference",
        }
    }

    /// Parse lenient (case-insensitive, acepta variantes conocidas).
    pub fn from_label(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "user" => Some(Self::User),
            "feedback" => Some(Self::Feedback),
            "project" => Some(Self::Project),
            "reference" => Some(Self::Reference),
            _ => None,
        }
    }
}

/// Frontmatter parsed del `.md`. Formato YAML-like minimo (3 campos).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub memory_type: MemoryType,
}

/// Entrada de memoria en disco.
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    /// Nombre del archivo, e.g. `user_role.md`.
    pub filename: String,
    pub frontmatter: MemoryFrontmatter,
    /// Cuerpo del markdown tras el frontmatter. No se muestra en el indice
    /// pero se carga al leer la memoria completa.
    #[allow(dead_code, reason = "consumido por /memory-show y futuras UIs de edicion")]
    pub body: String,
}

impl MemoryEntry {
    /// Linea del indice `MEMORY.md`:
    /// `- [Title](file.md) — description`
    pub fn index_line(&self) -> String {
        format!(
            "- [{}]({}) — {}",
            self.frontmatter.name, self.filename, self.frontmatter.description
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_label_is_case_insensitive() {
        assert_eq!(MemoryType::from_label("USER"), Some(MemoryType::User));
        assert_eq!(MemoryType::from_label(" Project "), Some(MemoryType::Project));
        assert_eq!(MemoryType::from_label("unknown"), None);
    }

    #[test]
    fn all_types_roundtrip_label() {
        for t in MemoryType::all() {
            assert_eq!(MemoryType::from_label(t.label()), Some(*t));
        }
    }

    #[test]
    fn index_line_format() {
        let entry = MemoryEntry {
            filename: "user_role.md".into(),
            frontmatter: MemoryFrontmatter {
                name: "My Role".into(),
                description: "senior engineer".into(),
                memory_type: MemoryType::User,
            },
            body: String::new(),
        };
        assert_eq!(entry.index_line(), "- [My Role](user_role.md) — senior engineer");
    }
}
