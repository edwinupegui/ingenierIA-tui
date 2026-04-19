//! Fork de sesiones: duplica el JSONL del padre y marca la nueva sesion con
//! un entry `Fork { parent_session_id, label }`.
//!
//! El fork preserva todo el historial hasta el punto de fork. Cambios
//! posteriores en cualquier rama no se ven entre si.

use std::path::PathBuf;

use super::entry::{SessionEntry, TimedEntry};
use super::meta::SessionMeta;
use super::store;

/// Informacion resultado de un fork exitoso.
#[derive(Debug, Clone)]
pub struct ForkInfo {
    pub child_id: String,
    /// Id del padre. Se expone para logs/telemetry aunque el handler actual
    /// solo consume `child_id` + `label`.
    #[allow(dead_code, reason = "expuesto para logs y futuros handlers de fork tree")]
    pub parent_id: String,
    pub label: String,
}

/// Forkea una sesion existente creando una nueva con `new_id`. Copia todas
/// las partes rotadas y el archivo activo del padre, anade un entry `Fork`
/// y escribe un meta hijo con `parent_id` relleno.
pub fn fork_session(
    parent_id: &str,
    new_id: String,
    label: String,
    parent_meta: Option<&SessionMeta>,
) -> anyhow::Result<ForkInfo> {
    fork_session_truncated(parent_id, new_id, label, parent_meta, None)
}

/// Variante que forkea copiando solo los primeros `keep_entries` entries del
/// padre (si `Some`). Pasar `None` copia todos — equivale a `fork_session`.
/// Error si el padre esta vacio o si `keep_entries` excede el total.
pub fn fork_session_truncated(
    parent_id: &str,
    new_id: String,
    label: String,
    parent_meta: Option<&SessionMeta>,
    keep_entries: Option<usize>,
) -> anyhow::Result<ForkInfo> {
    let src_entries = store::load_all_entries(parent_id);
    if src_entries.is_empty() {
        anyhow::bail!("sesion padre '{parent_id}' no tiene entradas");
    }
    let slice: &[TimedEntry] = if let Some(n) = keep_entries {
        if n == 0 {
            anyhow::bail!("fork truncado con 0 entries = sesion vacia");
        }
        if n > src_entries.len() {
            anyhow::bail!(
                "fork truncado pide {n} entries pero el padre tiene {}",
                src_entries.len()
            );
        }
        &src_entries[..n]
    } else {
        &src_entries[..]
    };

    // Copiar entries al hijo.
    for entry in slice {
        store::append_entry(&new_id, entry)?;
    }

    // Marcar el fork.
    let fork_entry = TimedEntry::now(SessionEntry::Fork {
        parent_session_id: parent_id.to_string(),
        label: label.clone(),
    });
    store::append_entry(&new_id, &fork_entry)?;

    // Escribir meta hijo. Para titulo usamos los entries efectivamente
    // copiados (`slice`), no todos los del padre: si truncamos a antes del
    // primer user message, el fallback deberia reflejarlo.
    let (title, factory, model) = parent_meta
        .map(|m| (m.title.clone(), m.factory.clone(), m.model.clone()))
        .unwrap_or_else(|| (derive_title_from_entries(slice), "?".to_string(), "?".to_string()));

    let mut child_meta = SessionMeta::new(new_id.clone(), title, factory, model);
    child_meta.parent_id = Some(parent_id.to_string());
    child_meta.fork_label = Some(label.clone());
    if let Some(parent) = parent_meta {
        child_meta.total_input_tokens = parent.total_input_tokens;
        child_meta.total_output_tokens = parent.total_output_tokens;
        child_meta.total_cost = parent.total_cost;
        child_meta.mode = parent.mode.clone();
        child_meta.turn_count = parent.turn_count;
        child_meta.message_count = parent.message_count;
    }
    let meta_path =
        store::meta_path(&new_id).ok_or_else(|| anyhow::anyhow!("No config dir for meta"))?;
    child_meta.save(&meta_path)?;

    Ok(ForkInfo { child_id: new_id, parent_id: parent_id.to_string(), label })
}

/// Formato de exportacion soportado por `export_session_as`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Jsonl,
    Markdown,
    Csv,
}

impl ExportFormat {
    /// Infiere el formato a partir de la extension del destino. Default: Jsonl.
    pub fn from_path(dest: &std::path::Path) -> Self {
        match dest.extension().and_then(|s| s.to_str()).map(str::to_ascii_lowercase).as_deref() {
            Some("md") | Some("markdown") => Self::Markdown,
            Some("csv") => Self::Csv,
            _ => Self::Jsonl,
        }
    }
}

/// Exporta la sesion a `dest` en formato JSONL (legacy, preserva el shape
/// exacto del almacenamiento). Wrapper de `export_session_as` para
/// compatibilidad con callers existentes.
pub fn export_session(id: &str, dest: &PathBuf) -> anyhow::Result<usize> {
    export_session_as(id, dest, ExportFormat::Jsonl)
}

/// Exporta la sesion en el formato solicitado. Sobreescribe el archivo.
pub fn export_session_as(id: &str, dest: &PathBuf, format: ExportFormat) -> anyhow::Result<usize> {
    let entries = store::load_all_entries(id);
    if entries.is_empty() {
        anyhow::bail!("sesion '{id}' no tiene entradas para exportar");
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let out = match format {
        ExportFormat::Jsonl => entries_to_jsonl(&entries)?,
        ExportFormat::Markdown => entries_to_markdown(&entries),
        ExportFormat::Csv => entries_to_csv(&entries),
    };
    std::fs::write(dest, out)?;
    Ok(entries.len())
}

fn entries_to_jsonl(entries: &[TimedEntry]) -> anyhow::Result<String> {
    let mut out = String::new();
    for entry in entries {
        out.push_str(&serde_json::to_string(entry)?);
        out.push('\n');
    }
    Ok(out)
}

fn entries_to_markdown(entries: &[TimedEntry]) -> String {
    let mut out = String::new();
    out.push_str("# Session export\n\n");
    for entry in entries {
        let ts = &entry.timestamp;
        match &entry.entry {
            SessionEntry::UserMessage { content } => {
                out.push_str(&format!("## 🧑 User · {ts}\n\n{content}\n\n"));
            }
            SessionEntry::AssistantMessage { content, tool_calls } => {
                out.push_str(&format!("## 🤖 Assistant · {ts}\n\n"));
                if !content.trim().is_empty() {
                    out.push_str(content);
                    out.push_str("\n\n");
                }
                for tc in tool_calls {
                    out.push_str(&format!(
                        "- tool `{}` (id `{}`): `{}`\n",
                        tc.name, tc.id, tc.arguments
                    ));
                }
                if !tool_calls.is_empty() {
                    out.push('\n');
                }
            }
            SessionEntry::ToolResult { tool_call_id, content } => {
                out.push_str(&format!(
                    "## 🛠️ Tool result · {ts}\n\n_call `{tool_call_id}`_\n\n```\n{content}\n```\n\n"
                ));
            }
            SessionEntry::SystemMessage { content } => {
                out.push_str(&format!("## ⚙️ System · {ts}\n\n{content}\n\n"));
            }
            SessionEntry::Fork { parent_session_id, label } => {
                out.push_str(&format!(
                    "> Forked from `{parent_session_id}` as `{label}` at {ts}\n\n"
                ));
            }
            SessionEntry::MetaSnapshot { turn_count, message_count, total_cost, .. } => {
                out.push_str(&format!(
                    "> _meta · {ts}_ — turns={turn_count} msgs={message_count} cost=${total_cost:.4}\n\n"
                ));
            }
        }
    }
    out
}

fn entries_to_csv(entries: &[TimedEntry]) -> String {
    let mut out = String::from("timestamp,role,content\n");
    for entry in entries {
        let (role, content) = match &entry.entry {
            SessionEntry::UserMessage { content } => ("user", content.clone()),
            SessionEntry::AssistantMessage { content, tool_calls } => {
                let mut body = content.clone();
                if !tool_calls.is_empty() {
                    for tc in tool_calls {
                        body.push_str(&format!("\n[tool {}({})]", tc.name, tc.arguments));
                    }
                }
                ("assistant", body)
            }
            SessionEntry::ToolResult { tool_call_id, content } => {
                ("tool", format!("[call {tool_call_id}] {content}"))
            }
            SessionEntry::SystemMessage { content } => ("system", content.clone()),
            SessionEntry::Fork { parent_session_id, label } => {
                ("fork", format!("parent={parent_session_id} label={label}"))
            }
            SessionEntry::MetaSnapshot { .. } => continue, // excluir snapshots en CSV
        };
        out.push_str(&csv_quote(&entry.timestamp));
        out.push(',');
        out.push_str(&csv_quote(role));
        out.push(',');
        out.push_str(&csv_quote(&content));
        out.push('\n');
    }
    out
}

/// CSV quoting RFC 4180: envuelve en `"..."` y escapa `"` como `""`.
fn csv_quote(s: &str) -> String {
    let escaped = s.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

fn derive_title_from_entries(entries: &[TimedEntry]) -> String {
    entries
        .iter()
        .find_map(|e| match &e.entry {
            SessionEntry::UserMessage { content } => Some(super::title_from_content(content)),
            _ => None,
        })
        .unwrap_or_else(|| "Fork".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_tmp_home() -> (tempfile::TempDir, std::sync::MutexGuard<'static, ()>) {
        let guard = crate::TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", tmp.path());
            std::env::set_var("HOME", tmp.path());
        }
        (tmp, guard)
    }

    #[test]
    fn fork_copies_parent_entries_and_appends_marker() {
        let (_tmp, _g) = setup_tmp_home();
        let parent_id = format!("fork-parent-{}", std::process::id());
        let child_id = format!("fork-child-{}", std::process::id());

        let e1 = TimedEntry::with_timestamp(
            "2026-04-13T10:00:00Z".into(),
            SessionEntry::UserMessage { content: "pregunta inicial".into() },
        );
        let e2 = TimedEntry::with_timestamp(
            "2026-04-13T10:00:01Z".into(),
            SessionEntry::AssistantMessage { content: "respuesta".into(), tool_calls: vec![] },
        );
        store::append_entry(&parent_id, &e1).unwrap();
        store::append_entry(&parent_id, &e2).unwrap();

        let info = fork_session(&parent_id, child_id.clone(), "probar opus".into(), None).unwrap();
        assert_eq!(info.parent_id, parent_id);
        assert_eq!(info.label, "probar opus");

        let child_entries = store::load_all_entries(&child_id);
        // 2 del padre + marker de fork
        assert_eq!(child_entries.len(), 3);
        assert!(matches!(child_entries.last().unwrap().entry, SessionEntry::Fork { .. }));

        let _ = store::delete_session(&parent_id);
        let _ = store::delete_session(&child_id);
    }

    #[test]
    fn fork_truncated_keeps_only_first_n_entries() {
        let (_tmp, _g) = setup_tmp_home();
        let parent_id = format!("fork-trunc-parent-{}", std::process::id());
        let child_id = format!("fork-trunc-child-{}", std::process::id());

        for (i, content) in ["q1", "a1", "q2", "a2"].iter().enumerate() {
            let entry = if i % 2 == 0 {
                SessionEntry::UserMessage { content: (*content).into() }
            } else {
                SessionEntry::AssistantMessage { content: (*content).into(), tool_calls: vec![] }
            };
            let timed = TimedEntry::with_timestamp(format!("t{i}"), entry);
            store::append_entry(&parent_id, &timed).unwrap();
        }
        // Truncar a 2 entries → solo q1 + a1 en el child, mas el fork marker.
        let info =
            fork_session_truncated(&parent_id, child_id.clone(), "half".into(), None, Some(2))
                .unwrap();
        assert_eq!(info.label, "half");
        let child_entries = store::load_all_entries(&child_id);
        assert_eq!(child_entries.len(), 3, "2 copiadas + 1 fork marker");
        match &child_entries[0].entry {
            SessionEntry::UserMessage { content } => assert_eq!(content, "q1"),
            _ => panic!("entry 0 debe ser q1"),
        }
        assert!(matches!(child_entries[2].entry, SessionEntry::Fork { .. }));

        let _ = store::delete_session(&parent_id);
        let _ = store::delete_session(&child_id);
    }

    #[test]
    fn fork_truncated_rejects_zero_or_overflow() {
        let (_tmp, _g) = setup_tmp_home();
        let parent_id = format!("fork-trunc-bad-parent-{}", std::process::id());
        let child_id = format!("fork-trunc-bad-child-{}", std::process::id());
        store::append_entry(
            &parent_id,
            &TimedEntry::with_timestamp(
                "t".into(),
                SessionEntry::UserMessage { content: "x".into() },
            ),
        )
        .unwrap();
        let zero =
            fork_session_truncated(&parent_id, format!("{child_id}-z"), "l".into(), None, Some(0));
        assert!(zero.is_err());
        let overflow = fork_session_truncated(
            &parent_id,
            format!("{child_id}-o"),
            "l".into(),
            None,
            Some(999),
        );
        assert!(overflow.is_err());
        let _ = store::delete_session(&parent_id);
    }

    #[test]
    fn fork_fails_when_parent_empty() {
        let (_tmp, _g) = setup_tmp_home();
        let parent_id = format!("fork-empty-parent-{}", std::process::id());
        let child_id = format!("fork-empty-child-{}", std::process::id());
        let err = fork_session(&parent_id, child_id, "x".into(), None);
        assert!(err.is_err());
    }

    #[test]
    fn export_writes_concatenated_jsonl() {
        let (_tmp, _g) = setup_tmp_home();
        let id = format!("export-{}", std::process::id());
        let e = TimedEntry::with_timestamp(
            "t".into(),
            SessionEntry::UserMessage { content: "hi".into() },
        );
        store::append_entry(&id, &e).unwrap();

        let dest = std::env::temp_dir().join(format!("export-dest-{}.jsonl", std::process::id()));
        let count = export_session(&id, &dest).unwrap();
        assert_eq!(count, 1);
        let content = std::fs::read_to_string(&dest).unwrap();
        assert!(content.contains("user_message"));
        let _ = std::fs::remove_file(&dest);
        let _ = store::delete_session(&id);
    }

    #[test]
    fn export_markdown_renders_roles_and_content() {
        let (_tmp, _g) = setup_tmp_home();
        let id = format!("export-md-{}", std::process::id());
        store::append_entry(
            &id,
            &TimedEntry::with_timestamp(
                "2026-04-17T00:00:00Z".into(),
                SessionEntry::UserMessage { content: "hola mundo".into() },
            ),
        )
        .unwrap();
        store::append_entry(
            &id,
            &TimedEntry::with_timestamp(
                "2026-04-17T00:00:01Z".into(),
                SessionEntry::AssistantMessage { content: "respuesta".into(), tool_calls: vec![] },
            ),
        )
        .unwrap();

        let dest = std::env::temp_dir().join(format!("export-md-{}.md", std::process::id()));
        let count = export_session_as(&id, &dest, ExportFormat::Markdown).unwrap();
        assert_eq!(count, 2);
        let content = std::fs::read_to_string(&dest).unwrap();
        assert!(content.contains("# Session export"));
        assert!(content.contains("🧑 User"));
        assert!(content.contains("hola mundo"));
        assert!(content.contains("🤖 Assistant"));
        assert!(content.contains("respuesta"));
        let _ = std::fs::remove_file(&dest);
        let _ = store::delete_session(&id);
    }

    #[test]
    fn export_csv_has_header_and_quotes_embedded_commas() {
        let (_tmp, _g) = setup_tmp_home();
        let id = format!("export-csv-{}", std::process::id());
        store::append_entry(
            &id,
            &TimedEntry::with_timestamp(
                "2026-04-17T00:00:00Z".into(),
                SessionEntry::UserMessage { content: "a,b \"c\"".into() },
            ),
        )
        .unwrap();

        let dest = std::env::temp_dir().join(format!("export-csv-{}.csv", std::process::id()));
        export_session_as(&id, &dest, ExportFormat::Csv).unwrap();
        let content = std::fs::read_to_string(&dest).unwrap();
        assert!(content.starts_with("timestamp,role,content\n"));
        // Comillas escapadas como "" y comas dentro de campo envuelto.
        assert!(content.contains("\"a,b \"\"c\"\"\""));
        let _ = std::fs::remove_file(&dest);
        let _ = store::delete_session(&id);
    }

    #[test]
    fn export_format_inferred_from_extension() {
        assert_eq!(ExportFormat::from_path(std::path::Path::new("x.md")), ExportFormat::Markdown);
        assert_eq!(
            ExportFormat::from_path(std::path::Path::new("x.markdown")),
            ExportFormat::Markdown
        );
        assert_eq!(ExportFormat::from_path(std::path::Path::new("x.csv")), ExportFormat::Csv);
        assert_eq!(ExportFormat::from_path(std::path::Path::new("x.jsonl")), ExportFormat::Jsonl);
        // Desconocida → Jsonl default.
        assert_eq!(ExportFormat::from_path(std::path::Path::new("x.txt")), ExportFormat::Jsonl);
    }
}
