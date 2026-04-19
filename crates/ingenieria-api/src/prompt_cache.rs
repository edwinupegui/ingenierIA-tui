//! Prompt caching para Anthropic Claude.
//!
//! Anthropic permite hasta 4 cache breakpoints (`cache_control: ephemeral`).
//! Estrategia de ingenierIA TUI:
//!   1. **Breakpoint 1**: System prompt completo (estable: stack, MCP, ingenierIA docs).
//!   2. **Breakpoint 2**: Ultimo bloque de tools (estable mientras no se descubran nuevas).
//!
//! Esto cobra ~25% extra al crear el cache pero ahorra ~90% en cada turno
//! posterior cuando el contenido cacheado se reusa.
//!
//! Tokens minimos para que la cache aplique: 1024 (Sonnet/Opus) o 2048 (Haiku).
//! Por debajo del minimo, la API ignora silenciosamente el `cache_control`.

use serde_json::{json, Value};

/// Tokens minimos estimados (chars/4) bajo los cuales no aplicamos cache.
/// La API valida el minimo real; este es un guard cliente para no contaminar
/// requests cortos con marcadores que no agregan valor.
const MIN_CACHE_CHARS: usize = 4_096; // ~1024 tokens

/// Aplica `cache_control` al system prompt si supera el minimo de tokens.
///
/// Convierte el formato string del system en bloque estructurado:
/// ```ignore
/// "system": [{
///     "type": "text",
///     "text": "...",
///     "cache_control": { "type": "ephemeral" }
/// }]
/// ```
pub fn cache_system_prompt(system: &str) -> Option<Value> {
    if system.len() < MIN_CACHE_CHARS {
        return None;
    }
    Some(json!([{
        "type": "text",
        "text": system,
        "cache_control": { "type": "ephemeral" }
    }]))
}

/// Aplica `cache_control` al ultimo tool definition de la lista.
///
/// Anthropic cachea desde el inicio del array hasta el primer breakpoint,
/// asi que marcar el ultimo cubre todas las definitions.
/// No hace nada si la lista esta vacia o tiene una sola tool.
pub fn cache_tool_definitions(tools: &mut [Value]) {
    let Some(last) = tools.last_mut() else {
        return;
    };
    if let Some(obj) = last.as_object_mut() {
        obj.insert("cache_control".to_string(), json!({ "type": "ephemeral" }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_system_not_cached() {
        assert!(cache_system_prompt("hola mundo").is_none());
    }

    #[test]
    fn large_system_cached() {
        let big = "x".repeat(MIN_CACHE_CHARS + 1);
        let value = cache_system_prompt(&big).expect("debe cachear");
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn cache_tools_marks_last() {
        let mut tools = vec![json!({"name": "a"}), json!({"name": "b"})];
        cache_tool_definitions(&mut tools);
        assert!(tools[0].get("cache_control").is_none());
        assert_eq!(tools[1]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn cache_tools_empty_is_noop() {
        let mut tools: Vec<Value> = Vec::new();
        cache_tool_definitions(&mut tools);
        assert!(tools.is_empty());
    }
}
