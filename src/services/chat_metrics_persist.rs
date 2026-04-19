//! Persistencia JSONL de turnos de chat (P3.8).
//!
//! Cada `CompletedTurn` de `ingenieria_api::metrics` se serializa a una línea
//! JSON y se hace append a `~/.local/share/ingenieria-tui/metrics/<session>.jsonl`.
//! Fire-and-forget desde un `tokio::spawn`: errores de I/O se loguean con
//! `tracing::warn` y no interrumpen el chat.
//!
//! Formato de cada línea:
//! ```json
//! {
//!   "session_id": "sess-abc",
//!   "completed_at_epoch_s": 1729168200,
//!   "ttft_ms": 312,
//!   "otps": 87.3,
//!   "total_duration_ms": 4521,
//!   "tool_count": 3,
//!   "tool_duration_ms": 1202,
//!   "response_chars": 1820
//! }
//! ```
use std::path::PathBuf;

use ingenieria_api::metrics::CompletedTurn;

/// Path del directorio donde se persisten los jsonl por sesión.
pub fn metrics_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("ingenieria-tui").join("metrics"))
}

/// Path del jsonl para una sesión dada.
pub fn jsonl_path(session_id: &str) -> Option<PathBuf> {
    metrics_dir().map(|d| d.join(format!("{session_id}.jsonl")))
}

/// Append best-effort de un CompletedTurn a disco. Silencioso en error.
pub fn append_turn_async(session_id: String, turn: CompletedTurn) {
    tokio::spawn(async move {
        if let Err(e) = append_turn_blocking(&session_id, &turn) {
            tracing::warn!(error = %e, session = %session_id, "metrics append failed");
        }
    });
}

fn append_turn_blocking(session_id: &str, turn: &CompletedTurn) -> std::io::Result<()> {
    use std::io::Write;
    let Some(path) = jsonl_path(session_id) else {
        return Ok(()); // data_local_dir unavailable: skip silently
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let entry = serialize_turn(session_id, turn);
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(file, "{entry}")?;
    Ok(())
}

fn serialize_turn(session_id: &str, turn: &CompletedTurn) -> String {
    let epoch_s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ttft_ms = turn.ttft.map(|d| d.as_millis() as u64);
    let value = serde_json::json!({
        "session_id": session_id,
        "completed_at_epoch_s": epoch_s,
        "ttft_ms": ttft_ms,
        "otps": turn.otps,
        "total_duration_ms": turn.total_duration.as_millis() as u64,
        "tool_count": turn.tool_count,
        "tool_duration_ms": turn.tool_duration_total.as_millis() as u64,
        "response_chars": turn.response_chars,
    });
    serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string())
}

/// Cuenta cuántos turnos se han persistido en todos los jsonl del directorio
/// de metrics. Útil para `/metrics --history`.
#[allow(dead_code, reason = "consumido por /metrics --history (extensión futura)")]
pub fn total_persisted_turns() -> usize {
    let Some(dir) = metrics_dir() else {
        return 0;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return 0;
    };
    let mut total = 0usize;
    for entry in entries.flatten() {
        if entry.path().extension().and_then(|e| e.to_str()) == Some("jsonl") {
            if let Ok(text) = std::fs::read_to_string(entry.path()) {
                total += text.lines().filter(|l| !l.trim().is_empty()).count();
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn sample_turn() -> CompletedTurn {
        CompletedTurn {
            ttft: Some(Duration::from_millis(300)),
            otps: Some(85.0),
            total_duration: Duration::from_millis(4500),
            tool_count: 2,
            tool_duration_total: Duration::from_millis(1200),
            response_chars: 1500,
        }
    }

    #[test]
    fn serialize_turn_contains_all_fields() {
        let s = serialize_turn("test-sess", &sample_turn());
        assert!(s.contains("\"session_id\":\"test-sess\""));
        assert!(s.contains("\"ttft_ms\":300"));
        assert!(s.contains("\"total_duration_ms\":4500"));
        assert!(s.contains("\"tool_count\":2"));
        assert!(s.contains("\"response_chars\":1500"));
    }

    #[test]
    fn serialize_turn_handles_missing_ttft() {
        let mut turn = sample_turn();
        turn.ttft = None;
        turn.otps = None;
        let s = serialize_turn("x", &turn);
        assert!(s.contains("\"ttft_ms\":null"));
        assert!(s.contains("\"otps\":null"));
    }
}
