//! Tracking acumulado de costos para la sesion de chat.
//!
//! Usa la tabla de precios `services::chat::pricing` para soportar multiples
//! modelos (Sonnet/Haiku/Opus/Copilot) y calcula ahorros derivados de
//! prompt caching de Anthropic (cache_read tokens cuestan ~10% del input price).

use crate::services::chat::pricing::{pricing_for, ModelPricing};

/// Budget por defecto para una sesion individual ($0.50).
pub const DEFAULT_SESSION_BUDGET: f64 = 0.50;

/// Estado acumulado de tokens y costo para la sesion actual.
///
/// Todos los conteos son acumulativos en la sesion (reset al cargar/clear chat).
#[derive(Debug, Clone)]
pub struct CostState {
    /// Tokens de input "frescos" (no servidos desde cache).
    pub total_input: u32,
    /// Tokens de output generados por el modelo.
    pub total_output: u32,
    /// Tokens enviados a crear cache (prompt_cache_creation).
    pub cache_creation_input: u32,
    /// Tokens leidos desde cache (prompt_cache_read).
    pub cache_read_input: u32,
    /// Cantidad de turnos completados (incrementa con cada Usage event).
    pub turn_count: u32,
    /// Cantidad de tool calls ejecutados.
    pub tool_calls: u32,
    /// Modelo activo (resuelve precios). `None` hasta el primer turno.
    pub model: Option<String>,
    /// Budget objetivo para la sesion ($).
    pub session_budget: f64,
}

impl Default for CostState {
    fn default() -> Self {
        Self {
            total_input: 0,
            total_output: 0,
            cache_creation_input: 0,
            cache_read_input: 0,
            turn_count: 0,
            tool_calls: 0,
            model: None,
            session_budget: DEFAULT_SESSION_BUDGET,
        }
    }
}

impl CostState {
    /// Registra los tokens de un turno y el modelo usado.
    pub fn add_usage(
        &mut self,
        input: u32,
        output: u32,
        cache_creation: u32,
        cache_read: u32,
        model: &str,
    ) {
        self.total_input += input;
        self.total_output += output;
        self.cache_creation_input += cache_creation;
        self.cache_read_input += cache_read;
        self.turn_count += 1;
        if self.model.as_deref() != Some(model) {
            self.model = Some(model.to_string());
        }
    }

    pub fn add_tool_call(&mut self) {
        self.tool_calls += 1;
    }

    fn pricing(&self) -> ModelPricing {
        pricing_for(self.model.as_deref().unwrap_or(""))
    }

    pub fn input_cost(&self) -> f64 {
        let p = self.pricing();
        (self.total_input as f64 / 1_000_000.0) * p.input
    }

    pub fn output_cost(&self) -> f64 {
        let p = self.pricing();
        (self.total_output as f64 / 1_000_000.0) * p.output
    }

    pub fn cache_write_cost(&self) -> f64 {
        let p = self.pricing();
        (self.cache_creation_input as f64 / 1_000_000.0) * p.cache_write
    }

    pub fn cache_read_cost(&self) -> f64 {
        let p = self.pricing();
        (self.cache_read_input as f64 / 1_000_000.0) * p.cache_read
    }

    pub fn total_cost(&self) -> f64 {
        self.input_cost() + self.output_cost() + self.cache_write_cost() + self.cache_read_cost()
    }

    /// Costo "sin cache" estimado: que habriamos pagado si todos los cache_read
    /// se hubieran cobrado al precio normal de input.
    pub fn baseline_cost(&self) -> f64 {
        let p = self.pricing();
        let total_in = self.total_input + self.cache_creation_input + self.cache_read_input;
        let baseline_input = (total_in as f64 / 1_000_000.0) * p.input;
        baseline_input + self.output_cost()
    }

    /// Ahorro absoluto en USD gracias al cache.
    pub fn cache_savings(&self) -> f64 {
        (self.baseline_cost() - self.total_cost()).max(0.0)
    }

    /// Hit ratio del cache (0.0 - 100.0).
    pub fn cache_hit_ratio(&self) -> f64 {
        let total_in = self.total_input + self.cache_creation_input + self.cache_read_input;
        if total_in == 0 {
            return 0.0;
        }
        (self.cache_read_input as f64 / total_in as f64) * 100.0
    }

    pub fn total_tokens(&self) -> u32 {
        self.total_input + self.total_output + self.cache_creation_input + self.cache_read_input
    }

    pub fn cost_display(&self) -> String {
        format_money(self.total_cost())
    }

    /// Verifica si el costo cruzo el umbral del budget de sesion.
    pub fn budget_warning(&self) -> Option<&'static str> {
        let cost = self.total_cost();
        let budget = self.session_budget.max(0.001);
        if cost >= budget {
            Some("Budget alcanzado")
        } else if cost >= budget * 0.8 {
            Some("80% del budget")
        } else {
            None
        }
    }

    /// Porcentaje del budget consumido (0.0 - 100+).
    pub fn budget_percent(&self) -> f64 {
        let budget = self.session_budget.max(0.001);
        (self.total_cost() / budget) * 100.0
    }

    pub fn tokens_display(&self) -> String {
        format_tokens(self.total_tokens())
    }

    pub fn model_display_name(&self) -> &'static str {
        self.pricing().display_name
    }
}

/// Formatea un valor monetario con 2 o 4 decimales segun magnitud.
pub fn format_money(value: f64) -> String {
    if value < 0.01 {
        format!("${value:.4}")
    } else {
        format!("${value:.2}")
    }
}

/// Formatea un conteo de tokens en estilo compacto (1.2K / 3.4M).
pub fn format_tokens(total: u32) -> String {
    if total >= 1_000_000 {
        format!("{:.1}M", total as f64 / 1_000_000.0)
    } else if total >= 1_000 {
        format!("{:.1}K", total as f64 / 1_000.0)
    } else {
        format!("{total}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sonnet_state() -> CostState {
        CostState { model: Some("claude-sonnet-4-20250514".to_string()), ..CostState::default() }
    }

    #[test]
    fn default_session_budget_is_set() {
        let c = CostState::default();
        assert_eq!(c.session_budget, DEFAULT_SESSION_BUDGET);
        assert!(c.model.is_none());
    }

    #[test]
    fn add_usage_accumulates_all_token_kinds() {
        let mut c = CostState::default();
        c.add_usage(100, 200, 50, 800, "claude-sonnet-4-20250514");
        assert_eq!(c.total_input, 100);
        assert_eq!(c.total_output, 200);
        assert_eq!(c.cache_creation_input, 50);
        assert_eq!(c.cache_read_input, 800);
        assert_eq!(c.turn_count, 1);
        assert_eq!(c.model.as_deref(), Some("claude-sonnet-4-20250514"));
    }

    #[test]
    fn cost_breakdown_matches_pricing_table() {
        let mut c = sonnet_state();
        c.total_input = 1_000_000;
        c.total_output = 1_000_000;
        c.cache_creation_input = 1_000_000;
        c.cache_read_input = 1_000_000;
        // Sonnet: 3 + 15 + 3.75 + 0.30 = 22.05
        let total = c.total_cost();
        assert!((total - 22.05).abs() < 0.001, "expected 22.05 got {total}");
    }

    #[test]
    fn cache_savings_positive_when_cache_used() {
        let mut c = sonnet_state();
        c.total_input = 100;
        c.cache_read_input = 1_000_000;
        c.total_output = 0;
        // baseline: (100 + 1M) * 3 / 1M = 3.0003
        // total: 100*3/1M + 1M*0.30/1M = 0.0003 + 0.30 = 0.3003
        // savings: ~2.70
        let savings = c.cache_savings();
        assert!(savings > 2.6 && savings < 2.8, "got {savings}");
    }

    #[test]
    fn cache_hit_ratio_handles_empty() {
        let c = CostState::default();
        assert_eq!(c.cache_hit_ratio(), 0.0);
    }

    #[test]
    fn cache_hit_ratio_computes_percent() {
        let mut c = sonnet_state();
        c.total_input = 100;
        c.cache_creation_input = 100;
        c.cache_read_input = 800;
        // 800 / 1000 = 80%
        assert!((c.cache_hit_ratio() - 80.0).abs() < 0.01);
    }

    #[test]
    fn budget_warning_triggers_at_thresholds() {
        let mut c = sonnet_state();
        c.session_budget = 0.10;
        c.total_output = 7_000; // 7000 * 15 / 1M = 0.105 (over)
        assert_eq!(c.budget_warning(), Some("Budget alcanzado"));

        let mut c = sonnet_state();
        c.session_budget = 1.0;
        c.total_output = 60_000; // 60000 * 15 / 1M = 0.9 (90%)
        assert_eq!(c.budget_warning(), Some("80% del budget"));

        let mut c = sonnet_state();
        c.session_budget = 1.0;
        c.total_output = 1_000; // 0.015 -> under
        assert_eq!(c.budget_warning(), None);
    }

    #[test]
    fn format_tokens_compact_formats() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1_500), "1.5K");
        assert_eq!(format_tokens(2_500_000), "2.5M");
    }

    #[test]
    fn format_money_uses_4_decimals_when_small() {
        assert_eq!(format_money(0.0001), "$0.0001");
        assert_eq!(format_money(1.234), "$1.23");
    }
}
