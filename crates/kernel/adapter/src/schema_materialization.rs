use serde_json::Value;

pub(crate) fn schema_properties(
    schema: &serde_json::Map<String, Value>,
) -> Option<&serde_json::Map<String, Value>> {
    schema.get("properties").and_then(Value::as_object)
}

pub(crate) fn schema_required_fields(schema: &serde_json::Map<String, Value>) -> Vec<&str> {
    match schema.get("required") {
        Some(Value::Array(fields)) => fields.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn schema_property_to_context_type(schema: &Value) -> Result<&'static str, String> {
    let Some(object) = schema.as_object() else {
        return Err("field schema must be an object".to_string());
    };

    let Some(ty_value) = object.get("type") else {
        return Err("field schema is missing 'type'".to_string());
    };

    match ty_value {
        Value::String(ty) => map_schema_type_to_context_type(ty, object),
        Value::Array(_) => {
            Err("union field types are not supported for context materialization".to_string())
        }
        _ => Err("field schema 'type' must be a string".to_string()),
    }
}

fn map_schema_type_to_context_type(
    ty: &str,
    schema: &serde_json::Map<String, Value>,
) -> Result<&'static str, String> {
    match ty {
        "number" | "integer" => Ok("Number"),
        "boolean" => Ok("Bool"),
        "string" => Ok("String"),
        "array" => {
            let Some(items) = schema.get("items") else {
                return Err("array field requires an 'items' schema".to_string());
            };
            if array_items_are_numbers(items) {
                Ok("Series")
            } else {
                Err(
                    "array field items must be number or integer to materialize as Series"
                        .to_string(),
                )
            }
        }
        other => Err(format!("unsupported field type '{other}'")),
    }
}

fn array_items_are_numbers(items: &Value) -> bool {
    let Some(object) = items.as_object() else {
        return false;
    };
    match object.get("type") {
        Some(Value::String(ty)) => ty == "number" || ty == "integer",
        _ => false,
    }
}
