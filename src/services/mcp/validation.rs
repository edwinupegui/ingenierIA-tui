//! Validacion liviana de tool inputs contra `inputSchema` MCP.
//!
//! **Alcance limitado**: solo type-check + required properties + enum. NO
//! implementa JSON Schema completo (usaria un crate como `jsonschema` para
//! eso). Para inputs mas complejos se delega al server MCP que rechazara
//! con error 400.

use serde_json::Value;

/// Valida `input` contra el `schema` (typicamente `inputSchema` de un
/// `McpToolInfo`). Retorna `Ok(())` si todo OK o `Err(mensaje)` si no.
///
/// Chequeos:
/// - Si `schema.type == "object"`: `input` debe ser objeto.
/// - Si hay `required: [...]`: cada clave debe estar presente.
/// - Si una propiedad tiene `type`: el valor debe matchear.
/// - Si una propiedad tiene `enum: [...]`: el valor debe estar en la lista.
pub fn validate_tool_input(input: &Value, schema: &Value) -> anyhow::Result<()> {
    // Schema vacio o no-objeto → asumir valido.
    let Value::Object(schema_obj) = schema else {
        return Ok(());
    };

    let schema_type = schema_obj.get("type").and_then(|v| v.as_str()).unwrap_or("object");
    if schema_type == "object" && !input.is_object() {
        anyhow::bail!("expected object, got {}", value_type(input));
    }

    if let Some(Value::Array(required)) = schema_obj.get("required") {
        let Value::Object(input_obj) = input else {
            anyhow::bail!("input must be object when required fields exist");
        };
        for req in required {
            if let Some(key) = req.as_str() {
                if !input_obj.contains_key(key) {
                    anyhow::bail!("missing required field '{key}'");
                }
            }
        }
    }

    if let (Value::Object(props), Value::Object(input_obj)) =
        (schema_obj.get("properties").unwrap_or(&Value::Null), input)
    {
        for (key, prop_schema) in props {
            if let Some(value) = input_obj.get(key) {
                validate_property(key, value, prop_schema)?;
            }
        }
    }

    Ok(())
}

fn validate_property(key: &str, value: &Value, schema: &Value) -> anyhow::Result<()> {
    let Value::Object(schema_obj) = schema else {
        return Ok(());
    };

    if let Some(expected_type) = schema_obj.get("type").and_then(|v| v.as_str()) {
        if !matches_type(value, expected_type) {
            anyhow::bail!("'{key}': expected {expected_type}, got {}", value_type(value));
        }
    }

    if let Some(Value::Array(enums)) = schema_obj.get("enum") {
        if !enums.iter().any(|e| e == value) {
            anyhow::bail!("'{key}': value not in enum");
        }
    }

    Ok(())
}

fn matches_type(value: &Value, expected: &str) -> bool {
    match expected {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.is_i64() || value.is_u64(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "null" => value.is_null(),
        _ => true, // Tipos no soportados → pasar
    }
}

fn value_type(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_schema_accepts_anything() {
        assert!(validate_tool_input(&json!({}), &json!({})).is_ok());
        assert!(validate_tool_input(&json!("foo"), &json!(null)).is_ok());
    }

    #[test]
    fn required_field_missing_rejected() {
        let schema = json!({
            "type": "object",
            "required": ["name"],
            "properties": {"name": {"type": "string"}}
        });
        let result = validate_tool_input(&json!({}), &schema);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing required"));
    }

    #[test]
    fn required_field_present_accepted() {
        let schema = json!({
            "type": "object",
            "required": ["name"],
            "properties": {"name": {"type": "string"}}
        });
        assert!(validate_tool_input(&json!({"name": "x"}), &schema).is_ok());
    }

    #[test]
    fn type_mismatch_rejected() {
        let schema = json!({
            "type": "object",
            "properties": {"count": {"type": "integer"}}
        });
        let result = validate_tool_input(&json!({"count": "not-a-number"}), &schema);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expected integer"));
    }

    #[test]
    fn enum_violation_rejected() {
        let schema = json!({
            "type": "object",
            "properties": {"color": {"enum": ["red", "blue"]}}
        });
        let result = validate_tool_input(&json!({"color": "green"}), &schema);
        assert!(result.is_err());
    }

    #[test]
    fn enum_match_accepted() {
        let schema = json!({
            "type": "object",
            "properties": {"color": {"enum": ["red", "blue"]}}
        });
        assert!(validate_tool_input(&json!({"color": "red"}), &schema).is_ok());
    }

    #[test]
    fn non_object_rejected_when_type_is_object() {
        let schema = json!({"type": "object"});
        let result = validate_tool_input(&json!("string"), &schema);
        assert!(result.is_err());
    }

    #[test]
    fn additional_props_ignored() {
        let schema = json!({
            "type": "object",
            "required": ["a"],
            "properties": {"a": {"type": "string"}}
        });
        assert!(validate_tool_input(&json!({"a": "x", "extra": 42}), &schema).is_ok());
    }
}
