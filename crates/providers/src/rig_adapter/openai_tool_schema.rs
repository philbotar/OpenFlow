//! `OpenAI Responses` tool-schema compatibility.

use serde_json::Value;

/// Preserve open-object tool schemas by opting only incompatible function tools
/// out of `OpenAI` strict mode.
///
/// Rig 0.39 enables strict mode for every Responses function tool. Strict mode
/// rejects map-like schemas such as MCP `dict[str, Any]` parameters because
/// their objects intentionally allow additional properties. Closing those
/// objects would change the tool contract, so keep the schema and relax only
/// that tool.
pub(super) fn relax_incompatible_responses_tools(root: &mut Value) -> bool {
    let Some(tools) = root.get_mut("tools").and_then(Value::as_array_mut) else {
        return false;
    };

    let mut changed = false;
    for tool in tools {
        let is_strict_function = tool.get("type").and_then(Value::as_str) == Some("function")
            && tool.get("strict").and_then(Value::as_bool) == Some(true);
        if !is_strict_function
            || !tool
                .get("parameters")
                .is_some_and(contains_open_object_schema)
        {
            continue;
        }
        let Some(tool) = tool.as_object_mut() else {
            continue;
        };
        tool.insert("strict".to_string(), Value::Bool(false));
        changed = true;
    }
    changed
}

fn contains_open_object_schema(schema: &Value) -> bool {
    let Value::Object(object) = schema else {
        return false;
    };

    let object_type = match object.get("type") {
        Some(Value::String(kind)) => kind == "object",
        Some(Value::Array(kinds)) => kinds.iter().any(|kind| kind.as_str() == Some("object")),
        _ => false,
    };
    let is_object_schema = object_type
        || object.contains_key("properties")
        || object.contains_key("patternProperties")
        || object.contains_key("additionalProperties");
    if is_object_schema && object.get("additionalProperties") != Some(&Value::Bool(false)) {
        return true;
    }

    object.values().any(|value| match value {
        Value::Object(_) => contains_open_object_schema(value),
        Value::Array(values) => values.iter().any(contains_open_object_schema),
        _ => false,
    })
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "unit tests assert fixed JSON tool shapes"
)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn relaxes_nested_open_object_without_changing_parameters() {
        let parameters = json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "apply": {
                    "anyOf": [{
                        "type": "array",
                        "items": {
                            "type": "object",
                            "additionalProperties": true
                        }
                    }, {
                        "type": "null"
                    }]
                }
            },
            "required": ["apply"]
        });
        let mut request = json!({
            "tools": [{
                "type": "function",
                "name": "mcp_7_massive_call__api",
                "strict": true,
                "parameters": parameters
            }]
        });

        assert!(relax_incompatible_responses_tools(&mut request));
        assert_eq!(request["tools"][0]["strict"], false);
        assert_eq!(request["tools"][0]["parameters"], parameters);
    }

    #[test]
    fn keeps_closed_object_tool_strict() {
        let mut request = json!({
            "tools": [{
                "type": "function",
                "name": "read",
                "strict": true,
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {"path": {"type": "string"}},
                    "required": ["path"]
                }
            }]
        });

        assert!(!relax_incompatible_responses_tools(&mut request));
        assert_eq!(request["tools"][0]["strict"], true);
    }
}
