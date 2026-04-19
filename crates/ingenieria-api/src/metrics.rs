//! Chat performance metrics (E34).
//!
//! Captura **TTFT** (time-to-first-token), **OTPS** (output tokens per second)
//! y timing por turno / por tool. Diseñado para overhead <0.1ms por evento.
//!
//! Los tokens se aproximan heuristicamente como `chars / 4` (mismo criterio
//! que `ChatState::estimated_context_tokens`). Esto evita tokenizer real y
//! mantiene el colector independiente del provider.
//!
//! Flujo tipico de un turno del usuario:
//! 1. `on_turn_start()` — cuando el usuario envia un mensaje.
//! 2. `on_first_delta()` + `on_delta(len)` — al recibir texto del AI.
//! 3. `on_tool_call(id)` + `on_tool_end(id, name, success)` — por cada tool.
//! 4. `on_turn_end()` — cuando `ChatStreamDone` cierra el turno (sin mas tool rounds).
//!
//! Un turno puede contener multiples `on_turn_start`→`on_turn_end`? No: un
//! solo par. Pero puede incluir multiples ChatStreamDone (uno por round de
//! tools). El colector trata esos eventos intermedios como subrequests y solo
//! finaliza el turno cuando el caller llama explicitamente `on_turn_end`.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Heuristica: 1 token ~ 4 caracteres (promedio sobre codigo + prosa en EN/ES).
pub const CHARS_PER_TOKEN: f64 = 4.0;

/// Umbral para mostrar breakdown del turno en notify (segundos).
pub const TURN_BREAKDOWN_MIN_SECS: u64 = 5;

/// Duracion de un tool call individual.
#[derive(Debug, Clone)]
#[allow(dead_code, reason = "tool_name/success consumidos por tests + futuros widgets per-tool")]
pub struct ToolTiming {
    pub tool_name: String,
    pub duration: Duration,
    pub success: bool,
}

/// Resumen de un turno completado.
#[derive(Debug, Clone, Default)]
#[allow(dead_code, reason = "response_chars consumido por tests + exportable a audit")]
pub struct CompletedTurn {
    pub ttft: Option<Duration>,
    pub otps: Option<f64>,
    pub total_duration: Duration,
    pub tool_count: usize,
    pub tool_duration_total: Duration,
    pub response_chars: usize,
}

impl CompletedTurn {
    /// Tiempo activo del turno (total menos pausas por tools).
    pub fn active_duration(&self) -> Duration {
        self.total_duration.saturating_sub(self.tool_duration_total)
    }
}

/// Aggregates de toda la sesion.
#[derive(Debug, Clone, Default)]
pub struct SessionAggregates {
    pub turns: usize,
    pub ttft_median_ms: Option<u64>,
    pub ttft_p95_ms: Option<u64>,
    pub otps_median: Option<f64>,
    pub otps_p95: Option<f64>,
    pub total_tools: usize,
    pub avg_turn_secs: Option<f64>,
}

/// Colector principal de metricas por chat. Vive en `ChatState.metrics`.
#[derive(Debug, Default)]
pub struct ChatMetrics {
    // ── Turno activo ─────────────────────────────────────────────────────────
    pub turn_start: Option<Instant>,
    pub first_token_at: Option<Instant>,
    pub last_token_at: Option<Instant>,
    pub response_chars: usize,
    /// Tool calls en vuelo (id → momento de inicio).
    pub tool_starts: HashMap<String, Instant>,
    /// Tool timings finalizados dentro del turno actual.
    pub turn_tools: Vec<ToolTiming>,

    // ── Ultimo turno cerrado ─────────────────────────────────────────────────
    pub last_turn: Option<CompletedTurn>,

    // ── Aggregates de sesion ────────────────────────────────────────────────
    pub completed_turns: Vec<CompletedTurn>,
}

impl ChatMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Limpia todo (para `/clear`). No se llama hoy porque `/clear` recrea
    /// `ChatState::new()` completo, pero se mantiene para snapshots parciales.
    #[allow(dead_code, reason = "API publica para callers externos + tests")]
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    // ── Eventos del turno ────────────────────────────────────────────────────

    /// Marca el inicio de un turno (usuario envia mensaje).
    pub fn on_turn_start(&mut self) {
        let now = Instant::now();
        self.turn_start = Some(now);
        self.first_token_at = None;
        self.last_token_at = None;
        self.response_chars = 0;
        self.tool_starts.clear();
        self.turn_tools.clear();
    }

    /// Captura el primer delta del turno (TTFT). Idempotente: si ya hay
    /// `first_token_at`, solo actualiza `last_token_at`.
    pub fn on_first_delta(&mut self) {
        let now = Instant::now();
        if self.first_token_at.is_none() {
            self.first_token_at = Some(now);
        }
        self.last_token_at = Some(now);
    }

    /// Acumula chars recibidos en el turno (aproximacion de tokens output).
    pub fn on_delta(&mut self, chars: usize) {
        self.on_first_delta();
        self.response_chars = self.response_chars.saturating_add(chars);
    }

    /// Registra un tool call en vuelo.
    pub fn on_tool_call(&mut self, id: String) {
        self.tool_starts.insert(id, Instant::now());
    }

    /// Cierra un tool call y retorna la duracion (para popular
    /// `ToolCall::duration_ms`). Retorna `None` si no habia start registrado.
    pub fn on_tool_end(&mut self, id: &str, tool_name: String, success: bool) -> Option<Duration> {
        let start = self.tool_starts.remove(id)?;
        let duration = start.elapsed();
        self.turn_tools.push(ToolTiming { tool_name, duration, success });
        Some(duration)
    }

    /// Cierra el turno actual. Agrega a completed_turns y retorna el resumen.
    pub fn on_turn_end(&mut self) -> Option<CompletedTurn> {
        let start = self.turn_start.take()?;
        let total_duration = start.elapsed();
        let ttft = self.first_token_at.map(|t| t - start);
        let otps = self.current_otps();
        let tool_duration_total: Duration = self.turn_tools.iter().map(|t| t.duration).sum();

        let turn = CompletedTurn {
            ttft,
            otps,
            total_duration,
            tool_count: self.turn_tools.len(),
            tool_duration_total,
            response_chars: self.response_chars,
        };
        self.last_turn = Some(turn.clone());
        self.completed_turns.push(turn.clone());

        // Limpiar estado por-turno
        self.first_token_at = None;
        self.last_token_at = None;
        self.response_chars = 0;
        self.tool_starts.clear();
        self.turn_tools.clear();

        Some(turn)
    }

    // ── Consultas en vivo ────────────────────────────────────────────────────

    /// OTPS actual basado en last_token_at - first_token_at.
    pub fn current_otps(&self) -> Option<f64> {
        let first = self.first_token_at?;
        let last = self.last_token_at?;
        let elapsed = last.saturating_duration_since(first).as_secs_f64();
        if elapsed < 0.05 || self.response_chars == 0 {
            return None;
        }
        let tokens = self.response_chars as f64 / CHARS_PER_TOKEN;
        Some(tokens / elapsed)
    }

    /// TTFT acumulado del turno actual.
    pub fn current_ttft(&self) -> Option<Duration> {
        let start = self.turn_start?;
        let first = self.first_token_at?;
        Some(first - start)
    }

    /// Duracion del turno en curso.
    #[allow(dead_code, reason = "consumido por tests + futuro widget de status")]
    pub fn current_turn_elapsed(&self) -> Option<Duration> {
        self.turn_start.map(|t| t.elapsed())
    }

    /// Numero de turnos finalizados en la sesion.
    #[allow(dead_code, reason = "consumido por tests + futuro /metrics widget")]
    pub fn turn_count(&self) -> usize {
        self.completed_turns.len()
    }

    // ── Aggregates ───────────────────────────────────────────────────────────

    pub fn aggregates(&self) -> SessionAggregates {
        let turns = self.completed_turns.len();
        if turns == 0 {
            return SessionAggregates::default();
        }

        let mut ttfts: Vec<u64> = self
            .completed_turns
            .iter()
            .filter_map(|t| t.ttft.map(|d| d.as_millis() as u64))
            .collect();
        let mut otps: Vec<f64> = self.completed_turns.iter().filter_map(|t| t.otps).collect();

        ttfts.sort_unstable();
        otps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let ttft_median_ms = percentile_u64(&ttfts, 50);
        let ttft_p95_ms = percentile_u64(&ttfts, 95);
        let otps_median = percentile_f64(&otps, 50);
        let otps_p95 = percentile_f64(&otps, 95);

        let total_tools: usize = self.completed_turns.iter().map(|t| t.tool_count).sum();
        let total_secs: f64 =
            self.completed_turns.iter().map(|t| t.total_duration.as_secs_f64()).sum();
        let avg_turn_secs = Some(total_secs / turns as f64);

        SessionAggregates {
            turns,
            ttft_median_ms,
            ttft_p95_ms,
            otps_median,
            otps_p95,
            total_tools,
            avg_turn_secs,
        }
    }
}

// ── Utilidades de percentiles ───────────────────────────────────────────────

/// Percentil simple sobre un slice ya ordenado. Retorna `None` si `sorted` vacio.
fn percentile_u64(sorted: &[u64], p: u32) -> Option<u64> {
    if sorted.is_empty() {
        return None;
    }
    let idx = (sorted.len() as f64 * (p as f64 / 100.0)) as usize;
    Some(sorted[idx.min(sorted.len() - 1)])
}

fn percentile_f64(sorted: &[f64], p: u32) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let idx = (sorted.len() as f64 * (p as f64 / 100.0)) as usize;
    Some(sorted[idx.min(sorted.len() - 1)])
}

// ── Formatters ──────────────────────────────────────────────────────────────

/// Formato compacto para status bar durante streaming.
/// Ejemplo: `42.3 tok/s · TTFT 1.2s` o `… · TTFT 1.2s` si OTPS aun no disponible.
pub fn format_streaming_status(metrics: &ChatMetrics) -> Option<String> {
    let ttft_str = metrics
        .current_ttft()
        .map(|d| format!("TTFT {:.1}s", d.as_secs_f64()))
        .unwrap_or_else(|| "TTFT …".to_string());
    let otps_str = match metrics.current_otps() {
        Some(rate) => format!("{rate:.1} tok/s"),
        None => "…".to_string(),
    };
    // Solo mostrar si el turno esta activo
    metrics.turn_start?;
    Some(format!("{otps_str} · {ttft_str}"))
}

/// Formato para mostrar al cerrar un turno largo (`>5s`).
/// Ejemplo: `Turno 12.4s (API 8.1s · 3 tools 3.2s)`
pub fn format_turn_summary(turn: &CompletedTurn) -> Option<String> {
    if turn.total_duration.as_secs() < TURN_BREAKDOWN_MIN_SECS {
        return None;
    }
    let total = turn.total_duration.as_secs_f64();
    let active = turn.active_duration().as_secs_f64();
    let tool_secs = turn.tool_duration_total.as_secs_f64();
    if turn.tool_count == 0 {
        return Some(format!("Turno {total:.1}s"));
    }
    Some(format!(
        "Turno {total:.1}s (API {active:.1}s · {n} tools {tool_secs:.1}s)",
        n = turn.tool_count
    ))
}

/// Resumen textual para `/metrics`.
pub fn format_session_summary(agg: &SessionAggregates) -> String {
    if agg.turns == 0 {
        return "## Chat Metrics\n\n_Aún no hay turnos completados en esta sesión._".to_string();
    }
    let ttft_med = agg.ttft_median_ms.map(|v| format!("{v} ms")).unwrap_or_else(|| "—".into());
    let ttft_p95 = agg.ttft_p95_ms.map(|v| format!("{v} ms")).unwrap_or_else(|| "—".into());
    let otps_med = agg.otps_median.map(|v| format!("{v:.1}")).unwrap_or_else(|| "—".into());
    let otps_p95 = agg.otps_p95.map(|v| format!("{v:.1}")).unwrap_or_else(|| "—".into());
    let avg = agg.avg_turn_secs.map(|v| format!("{v:.1}s")).unwrap_or_else(|| "—".into());
    format!(
        "## Chat Metrics\n\n\
         ```\n\
         Turnos completados : {turns}\n\
         Turno promedio     : {avg}\n\
         TTFT mediana       : {ttft_med}\n\
         TTFT p95           : {ttft_p95}\n\
         OTPS mediana       : {otps_med} tok/s\n\
         OTPS p95           : {otps_p95} tok/s\n\
         Tool calls totales : {tools}\n\
         ```\n",
        turns = agg.turns,
        avg = avg,
        ttft_med = ttft_med,
        ttft_p95 = ttft_p95,
        otps_med = otps_med,
        otps_p95 = otps_p95,
        tools = agg.total_tools,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn new_metrics_are_empty() {
        let m = ChatMetrics::new();
        assert!(m.turn_start.is_none());
        assert!(m.last_turn.is_none());
        assert_eq!(m.completed_turns.len(), 0);
        assert_eq!(m.turn_count(), 0);
    }

    #[test]
    fn turn_start_sets_timestamp() {
        let mut m = ChatMetrics::new();
        m.on_turn_start();
        assert!(m.turn_start.is_some());
        assert_eq!(m.response_chars, 0);
    }

    #[test]
    fn on_delta_captures_ttft_on_first_call() {
        let mut m = ChatMetrics::new();
        m.on_turn_start();
        sleep(Duration::from_millis(10));
        m.on_delta(100);
        assert!(m.first_token_at.is_some());
        assert!(m.current_ttft().is_some());
        let ttft = m.current_ttft().unwrap();
        assert!(ttft.as_millis() >= 5);
    }

    #[test]
    fn subsequent_deltas_do_not_reset_first_token() {
        let mut m = ChatMetrics::new();
        m.on_turn_start();
        m.on_delta(10);
        let first = m.first_token_at;
        sleep(Duration::from_millis(5));
        m.on_delta(20);
        assert_eq!(first, m.first_token_at);
        assert_eq!(m.response_chars, 30);
    }

    #[test]
    fn current_otps_returns_none_before_deltas() {
        let mut m = ChatMetrics::new();
        m.on_turn_start();
        assert!(m.current_otps().is_none());
    }

    #[test]
    fn current_otps_computes_from_chars() {
        let mut m = ChatMetrics::new();
        m.on_turn_start();
        m.on_delta(80); // ~20 tokens
        sleep(Duration::from_millis(200));
        m.on_delta(80); // otra tanda
        let otps = m.current_otps();
        assert!(otps.is_some());
        // 40 tokens en >= 200ms → OTPS razonable (<= 200)
        let value = otps.unwrap();
        assert!(value > 0.0 && value.is_finite(), "otps={value}");
    }

    #[test]
    fn tool_start_end_returns_duration() {
        let mut m = ChatMetrics::new();
        m.on_turn_start();
        m.on_tool_call("t1".into());
        sleep(Duration::from_millis(15));
        let dur = m.on_tool_end("t1", "bash".into(), true);
        assert!(dur.is_some());
        assert!(dur.unwrap().as_millis() >= 10);
        assert_eq!(m.turn_tools.len(), 1);
        assert_eq!(m.turn_tools[0].tool_name, "bash");
    }

    #[test]
    fn tool_end_without_start_returns_none() {
        let mut m = ChatMetrics::new();
        m.on_turn_start();
        assert!(m.on_tool_end("nope", "bash".into(), true).is_none());
    }

    #[test]
    fn turn_end_pushes_completed_turn() {
        let mut m = ChatMetrics::new();
        m.on_turn_start();
        m.on_delta(40);
        let summary = m.on_turn_end();
        assert!(summary.is_some());
        assert_eq!(m.completed_turns.len(), 1);
        assert!(m.last_turn.is_some());
        assert!(m.turn_start.is_none());
    }

    #[test]
    fn turn_end_without_start_is_noop() {
        let mut m = ChatMetrics::new();
        assert!(m.on_turn_end().is_none());
        assert_eq!(m.completed_turns.len(), 0);
    }

    #[test]
    fn aggregates_empty_when_no_turns() {
        let m = ChatMetrics::new();
        let agg = m.aggregates();
        assert_eq!(agg.turns, 0);
        assert!(agg.ttft_median_ms.is_none());
    }

    #[test]
    fn aggregates_summarize_multiple_turns() {
        let mut m = ChatMetrics::new();
        for i in 0..3 {
            m.on_turn_start();
            sleep(Duration::from_millis(5));
            m.on_delta(40 * (i + 1));
            sleep(Duration::from_millis(20));
            m.on_delta(20);
            m.on_turn_end();
        }
        let agg = m.aggregates();
        assert_eq!(agg.turns, 3);
        assert!(agg.ttft_median_ms.is_some());
        assert!(agg.avg_turn_secs.is_some());
    }

    #[test]
    fn reset_clears_all_state() {
        let mut m = ChatMetrics::new();
        m.on_turn_start();
        m.on_delta(50);
        m.on_turn_end();
        m.reset();
        assert_eq!(m.completed_turns.len(), 0);
        assert!(m.turn_start.is_none());
    }

    #[test]
    fn format_streaming_status_none_if_idle() {
        let m = ChatMetrics::new();
        assert!(format_streaming_status(&m).is_none());
    }

    #[test]
    fn format_streaming_status_returns_text_while_active() {
        let mut m = ChatMetrics::new();
        m.on_turn_start();
        let s = format_streaming_status(&m).expect("active turn should format");
        assert!(s.contains("TTFT"));
    }

    #[test]
    fn format_turn_summary_skips_short_turns() {
        let turn = CompletedTurn {
            ttft: Some(Duration::from_millis(100)),
            otps: Some(40.0),
            total_duration: Duration::from_secs(2),
            tool_count: 0,
            tool_duration_total: Duration::ZERO,
            response_chars: 100,
        };
        assert!(format_turn_summary(&turn).is_none());
    }

    #[test]
    fn format_turn_summary_includes_tools_when_present() {
        let turn = CompletedTurn {
            ttft: Some(Duration::from_millis(200)),
            otps: Some(40.0),
            total_duration: Duration::from_secs(10),
            tool_count: 2,
            tool_duration_total: Duration::from_secs(3),
            response_chars: 400,
        };
        let s = format_turn_summary(&turn).unwrap();
        assert!(s.contains("10.0s"));
        assert!(s.contains("2 tools"));
    }

    #[test]
    fn format_session_summary_handles_empty() {
        let agg = SessionAggregates::default();
        let s = format_session_summary(&agg);
        assert!(s.contains("Aún no hay"));
    }

    #[test]
    fn format_session_summary_shows_turns() {
        let mut m = ChatMetrics::new();
        m.on_turn_start();
        m.on_delta(40);
        sleep(Duration::from_millis(10));
        m.on_delta(60);
        m.on_turn_end();
        let agg = m.aggregates();
        let s = format_session_summary(&agg);
        assert!(s.contains("Turnos completados"));
        assert!(s.contains('1'));
    }

    #[test]
    fn percentile_handles_empty() {
        assert!(percentile_u64(&[], 50).is_none());
        assert!(percentile_f64(&[], 50).is_none());
    }

    #[test]
    fn percentile_exact_at_median() {
        assert_eq!(percentile_u64(&[10, 20, 30, 40, 50], 50), Some(30));
    }
}
