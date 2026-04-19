//! Tabla de precios por modelo para calculo de costos.
//!
//! Los precios se expresan en USD por millon de tokens.
//! Anthropic publica precios oficiales en https://www.anthropic.com/pricing.
//! Copilot se factura por suscripcion (sin costo por token), pero registramos
//! tarifas equivalentes para mostrar el ahorro relativo en el panel.

/// Precios por millon de tokens para un modelo determinado.
///
/// `cache_write` se cobra una sola vez al crear el bloque cacheado.
/// `cache_read` aplica a tokens recuperados de cache (~10% del input price).
#[derive(Debug, Clone, Copy)]
pub struct ModelPricing {
    pub display_name: &'static str,
    pub input: f64,
    pub output: f64,
    pub cache_write: f64,
    pub cache_read: f64,
}

impl ModelPricing {
    pub const fn new(
        display_name: &'static str,
        input: f64,
        output: f64,
        cache_write: f64,
        cache_read: f64,
    ) -> Self {
        Self { display_name, input, output, cache_write, cache_read }
    }
}

// ── Tabla de precios ────────────────────────────────────────────────────────

/// Sonnet 4 / Sonnet 4.6: $3 in / $15 out (cache: $3.75 write, $0.30 read).
const SONNET_4: ModelPricing = ModelPricing::new("Claude Sonnet 4", 3.0, 15.0, 3.75, 0.30);

/// Haiku 4.5: $1 in / $5 out (cache: $1.25 write, $0.10 read).
const HAIKU_4: ModelPricing = ModelPricing::new("Claude Haiku 4.5", 1.0, 5.0, 1.25, 0.10);

/// Opus 4 / Opus 4.6: $15 in / $75 out (cache: $18.75 write, $1.50 read).
const OPUS_4: ModelPricing = ModelPricing::new("Claude Opus 4", 15.0, 75.0, 18.75, 1.50);

/// Default Claude (cuando no se reconoce el id) — usar Sonnet como referencia segura.
const DEFAULT_CLAUDE: ModelPricing = SONNET_4;

/// Copilot (subscripcion mensual, no se cobra por token). Tarifa de referencia.
const COPILOT: ModelPricing = ModelPricing::new("GitHub Copilot", 0.0, 0.0, 0.0, 0.0);

/// Resuelve el precio para un identificador de modelo.
///
/// El matching es por substring del id (case-insensitive), de mas especifico
/// a mas generico. Modelos desconocidos caen a `DEFAULT_CLAUDE`.
pub fn pricing_for(model: &str) -> ModelPricing {
    let m = model.to_ascii_lowercase();
    if m.contains("opus") {
        OPUS_4
    } else if m.contains("haiku") {
        HAIKU_4
    } else if m.contains("sonnet") {
        SONNET_4
    } else if m.contains("gpt") || m.contains("copilot") || m.contains("o1") || m.contains("gemini")
    {
        COPILOT
    } else {
        DEFAULT_CLAUDE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pricing_resolves_sonnet() {
        let p = pricing_for("claude-sonnet-4-20250514");
        assert_eq!(p.input, 3.0);
        assert_eq!(p.output, 15.0);
    }

    #[test]
    fn pricing_resolves_haiku() {
        let p = pricing_for("claude-haiku-4-5-20251001");
        assert_eq!(p.input, 1.0);
        assert_eq!(p.cache_read, 0.10);
    }

    #[test]
    fn pricing_resolves_opus() {
        let p = pricing_for("claude-opus-4-20250514");
        assert_eq!(p.output, 75.0);
    }

    #[test]
    fn pricing_resolves_copilot() {
        let p = pricing_for("gpt-4o");
        assert_eq!(p.input, 0.0);
    }

    #[test]
    fn pricing_unknown_falls_back_to_default() {
        let p = pricing_for("some-unknown-model");
        assert_eq!(p.input, DEFAULT_CLAUDE.input);
    }
}
