use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde_json::{json, Map, Value};

#[derive(Debug, Clone)]
pub struct McpTool {
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct ToolFilter {
    pub include: BTreeSet<String>,
    pub exclude: BTreeSet<String>,
    pub resources: bool,
    pub prompts: bool,
}

impl ToolFilter {
    pub fn allow_all() -> Self {
        Self {
            resources: true,
            prompts: true,
            ..Self::default()
        }
    }

    pub fn from_config(config: &Value) -> Self {
        let tools = config.get("tools").unwrap_or(&Value::Null);
        Self {
            include: normalize_name_filter(tools.get("include")),
            exclude: normalize_name_filter(tools.get("exclude")),
            resources: parse_boolish(tools.get("resources"), true),
            prompts: parse_boolish(tools.get("prompts"), true),
        }
    }
}

pub fn sanitize_name_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub fn registered_tools(
    server_name: &str,
    tools: &[McpTool],
    filter: &ToolFilter,
    advertises_resources: bool,
    advertises_prompts: bool,
) -> Vec<String> {
    let safe_server = sanitize_name_component(server_name);
    let mut registered = Vec::new();

    for tool in tools {
        let should_register = if !filter.include.is_empty() {
            filter.include.contains(&tool.name)
        } else if !filter.exclude.is_empty() {
            !filter.exclude.contains(&tool.name)
        } else {
            true
        };

        if should_register {
            registered.push(format!(
                "mcp_{safe_server}_{}",
                sanitize_name_component(&tool.name)
            ));
        }
    }

    if filter.resources && advertises_resources {
        registered.push(format!("mcp_{safe_server}_list_resources"));
        registered.push(format!("mcp_{safe_server}_read_resource"));
    }
    if filter.prompts && advertises_prompts {
        registered.push(format!("mcp_{safe_server}_list_prompts"));
        registered.push(format!("mcp_{safe_server}_get_prompt"));
    }

    registered
}

pub fn normalize_input_schema(schema: Option<Value>) -> Value {
    let Some(schema) = schema else {
        return json!({"properties": {}, "type": "object"});
    };
    let mut normalized = rewrite_local_refs(schema);
    strip_nullable_unions(&mut normalized);
    repair_object_shape(&mut normalized);
    if !normalized.is_object() {
        return json!({"properties": {}, "type": "object"});
    }
    if normalized.get("type") == Some(&json!("object")) && normalized.get("properties").is_none() {
        normalized["properties"] = json!({});
    }
    normalized
}

pub fn build_safe_env(
    current_env: &BTreeMap<String, String>,
    user_env: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for (key, value) in current_env {
        if SAFE_ENV_KEYS.contains(&key.as_str()) || key.starts_with("XDG_") {
            env.insert(key.clone(), value.clone());
        }
    }
    for (key, value) in user_env {
        env.insert(key.clone(), value.clone());
    }
    env
}

pub fn sanitize_error(text: &str) -> String {
    let mut out = String::new();
    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];
        if let Some(end) = credential_match_len(rest) {
            out.push_str("[REDACTED]");
            index += end;
        } else {
            let ch = rest.chars().next().unwrap();
            out.push(ch);
            index += ch.len_utf8();
        }
    }
    out
}

pub fn validate_remote_mcp_url(server_name: &str, url: &Value) -> Result<String, String> {
    let Some(text) = url.as_str() else {
        return Err(format!(
            "Invalid MCP URL for '{server_name}': expected a string, got {}",
            python_type_name(url)
        ));
    };
    let stripped = text.trim();
    if stripped.is_empty() {
        return Err(format!("Invalid MCP URL for '{server_name}': empty url"));
    }

    let Some((scheme, rest)) = stripped.split_once("://") else {
        return Err(format!(
            "Invalid MCP URL for '{server_name}': scheme must be http or https, got '' ('{stripped}')"
        ));
    };
    let scheme_lower = scheme.to_ascii_lowercase();
    if !matches!(scheme_lower.as_str(), "http" | "https") {
        return Err(format!(
            "Invalid MCP URL for '{server_name}': scheme must be http or https, got '{scheme_lower}' ('{stripped}')"
        ));
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() {
        return Err(format!(
            "Invalid MCP URL for '{server_name}': missing host ('{stripped}')"
        ));
    }
    let host = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);
    let hostname = if host.starts_with('[') {
        host.strip_prefix('[')
            .and_then(|value| value.split_once(']').map(|(name, _)| name))
            .unwrap_or("")
    } else {
        host.split(':').next().unwrap_or("")
    };
    if hostname.is_empty() {
        return Err(format!(
            "Invalid MCP URL for '{server_name}': missing hostname ('{stripped}')"
        ));
    }
    Ok(stripped.to_string())
}

pub fn safe_numeric_i64(value: &Value, default: i64, minimum: i64) -> i64 {
    let coerced = match value {
        Value::Number(number) => {
            if let Some(int) = number.as_i64() {
                Some(int)
            } else {
                number
                    .as_f64()
                    .filter(|value| value.is_finite())
                    .map(|value| value as i64)
            }
        }
        Value::String(text) => text.parse::<i64>().ok(),
        Value::Bool(value) => Some(i64::from(*value)),
        _ => None,
    };
    coerced.unwrap_or(default).max(minimum)
}

fn rewrite_local_refs(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut out = Map::new();
            for (key, value) in object {
                let out_key = if key == "definitions" { "$defs" } else { &key };
                out.insert(out_key.to_string(), rewrite_local_refs(value));
            }
            if let Some(Value::String(reference)) = out.get_mut("$ref") {
                if let Some(rest) = reference.strip_prefix("#/definitions/") {
                    *reference = format!("#/$defs/{rest}");
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(rewrite_local_refs).collect()),
        other => other,
    }
}

fn strip_nullable_unions(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if let Some(Value::Array(any_of)) = object.get("anyOf") {
                let non_null = any_of
                    .iter()
                    .find(|item| item.get("type") != Some(&json!("null")))
                    .cloned();
                if let Some(mut replacement) = non_null {
                    if let Value::Object(replacement_obj) = &mut replacement {
                        if object.get("default").is_some_and(Value::is_null) {
                            replacement_obj.insert("default".to_string(), Value::Null);
                        }
                        replacement_obj.insert("nullable".to_string(), json!(true));
                    }
                    *value = replacement;
                    strip_nullable_unions(value);
                    return;
                }
            }
            for item in object.values_mut() {
                strip_nullable_unions(item);
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_nullable_unions(item);
            }
        }
        _ => {}
    }
}

fn repair_object_shape(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for item in object.values_mut() {
                repair_object_shape(item);
            }
            let object_shaped =
                object.get("properties").is_some() || object.get("required").is_some();
            if object.get("type").is_none_or(Value::is_null) && object_shaped {
                object.insert("type".to_string(), json!("object"));
            }
            if object.get("type") == Some(&json!("object")) {
                if !object.get("properties").is_some_and(Value::is_object) {
                    object.insert("properties".to_string(), json!({}));
                }
                let property_names = object
                    .get("properties")
                    .and_then(Value::as_object)
                    .map(|properties| properties.keys().cloned().collect::<BTreeSet<_>>())
                    .unwrap_or_default();
                let valid_required =
                    object
                        .get("required")
                        .and_then(Value::as_array)
                        .map(|required| {
                            required
                                .iter()
                                .filter_map(Value::as_str)
                                .filter(|name| property_names.contains(*name))
                                .map(|name| json!(name))
                                .collect::<Vec<_>>()
                        });
                if let Some(valid_required) = valid_required {
                    if valid_required.is_empty() {
                        object.remove("required");
                    } else {
                        object.insert("required".to_string(), Value::Array(valid_required));
                    }
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                repair_object_shape(item);
            }
        }
        _ => {}
    }
}

fn normalize_name_filter(value: Option<&Value>) -> BTreeSet<String> {
    match value {
        None | Some(Value::Null) => BTreeSet::new(),
        Some(Value::String(text)) => BTreeSet::from([text.clone()]),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| match item {
                Value::String(text) => text.clone(),
                Value::Null => "None".to_string(),
                other => other.to_string(),
            })
            .collect(),
        _ => BTreeSet::new(),
    }
}

fn parse_boolish(value: Option<&Value>, default: bool) -> bool {
    match value {
        None | Some(Value::Null) => default,
        Some(Value::Bool(value)) => *value,
        Some(Value::String(text)) => match text.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => true,
            "false" | "0" | "no" | "off" => false,
            _ => default,
        },
        _ => default,
    }
}

const SAFE_ENV_KEYS: &[&str] = &[
    "PATH", "HOME", "USER", "LANG", "LC_ALL", "TERM", "SHELL", "TMPDIR",
];

fn credential_match_len(text: &str) -> Option<usize> {
    if let Some(rest) = text.strip_prefix("ghp_") {
        return secret_suffix_len(rest, true).map(|len| "ghp_".len() + len);
    }
    if let Some(rest) = text.strip_prefix("sk-") {
        return secret_suffix_len(rest, true).map(|len| "sk-".len() + len);
    }
    if let Some(rest) = text.strip_prefix("Bearer") {
        let whitespace_len = rest
            .chars()
            .take_while(|ch| ch.is_whitespace())
            .map(char::len_utf8)
            .sum::<usize>();
        if whitespace_len == 0 {
            return None;
        }
        let token = &rest[whitespace_len..];
        let token_len = token
            .chars()
            .take_while(|ch| !ch.is_whitespace())
            .map(char::len_utf8)
            .sum::<usize>();
        return (token_len > 0).then_some("Bearer".len() + whitespace_len + token_len);
    }
    for prefix in ["token=", "key=", "API_KEY=", "password="] {
        if let Some(rest) = text.strip_prefix(prefix) {
            return secret_suffix_len(rest, false).map(|len| prefix.len() + len);
        }
    }
    None
}

fn secret_suffix_len(text: &str, alnum_underscore_only: bool) -> Option<usize> {
    let mut len = 0;
    let mut count = 0;
    for ch in text.chars() {
        let allowed = if alnum_underscore_only {
            ch.is_ascii_alphanumeric() || ch == '_'
        } else {
            !ch.is_whitespace() && !matches!(ch, '&' | ',' | ';' | '"' | '\'')
        };
        if !allowed || count >= 255 {
            break;
        }
        len += ch.len_utf8();
        count += 1;
    }
    (count > 0).then_some(len)
}

fn python_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(number) if number.is_i64() || number.is_u64() => "int",
        Value::Number(_) => "float",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}
