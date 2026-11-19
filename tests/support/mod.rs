//! Shared JSON Schema subset checker for the schema and property tests.

use serde_json::Value;

/// Validates `v` against `schema`, appending one message per violation.
pub fn validate(schema: &Value, v: &Value, path: &str, errors: &mut Vec<String>) {
    if let Some(alts) = schema.get("oneOf").and_then(Value::as_array) {
        let ok = alts.iter().filter(|alt| {
            let mut e = Vec::new();
            validate(alt, v, path, &mut e);
            e.is_empty()
        });
        if ok.count() != 1 {
            errors.push(format!("{path}: matches no oneOf branch"));
        }
        return;
    }
    if let Some(t) = schema.get("type") {
        let types: Vec<&str> = match t {
            Value::String(s) => vec![s.as_str()],
            Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
            _ => vec![],
        };
        let fits = types.iter().any(|t| match *t {
            "object" => v.is_object(),
            "array" => v.is_array(),
            "string" => v.is_string(),
            "integer" => v.is_u64() || v.is_i64(),
            "boolean" => v.is_boolean(),
            "null" => v.is_null(),
            _ => false,
        });
        if !fits {
            errors.push(format!("{path}: expected {types:?}, got {v}"));
            return;
        }
    }
    if let Some(allowed) = schema.get("enum").and_then(Value::as_array)
        && !allowed.contains(v)
    {
        errors.push(format!("{path}: {v} is not one of {allowed:?}"));
    }
    if let Some(obj) = v.as_object() {
        let props = schema.get("properties").and_then(Value::as_object);
        for key in schema
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !obj.contains_key(key) {
                errors.push(format!("{path}: missing `{key}`"));
            }
        }
        for (k, child) in obj {
            match props.and_then(|p| p.get(k)) {
                Some(s) => validate(s, child, &format!("{path}.{k}"), errors),
                None if schema.get("additionalProperties") == Some(&Value::Bool(false)) => {
                    errors.push(format!("{path}: undeclared field `{k}`"))
                }
                None => {}
            }
        }
    }
    if let (Some(items), Some(arr)) = (schema.get("items"), v.as_array()) {
        for (i, child) in arr.iter().enumerate() {
            validate(items, child, &format!("{path}[{i}]"), errors);
        }
    }
}
