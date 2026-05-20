use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderProfileSummary {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub api_mode: &'static str,
    pub display_name: &'static str,
    pub env_vars: &'static [&'static str],
    pub base_url: &'static str,
    pub auth_type: &'static str,
    pub supports_health_check: bool,
    pub fallback_model_count: usize,
    pub default_max_tokens: Option<i64>,
    pub fixed_temperature: Option<&'static str>,
    pub default_header_keys: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderResponse {
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}

pub trait ChatProvider {
    fn chat(&mut self, request: Value) -> ProviderResponse;
}

#[derive(Debug, Clone)]
pub struct FakeProvider {
    responses: VecDeque<ProviderResponse>,
    requests: Vec<Value>,
}

impl FakeProvider {
    pub fn new(responses: impl IntoIterator<Item = ProviderResponse>) -> Self {
        Self {
            responses: responses.into_iter().collect(),
            requests: Vec::new(),
        }
    }

    pub fn requests(&self) -> &[Value] {
        &self.requests
    }
}

impl ChatProvider for FakeProvider {
    fn chat(&mut self, request: Value) -> ProviderResponse {
        self.requests.push(request);
        self.responses.pop_front().unwrap_or(ProviderResponse {
            content: Some(String::new()),
            tool_calls: Vec::new(),
        })
    }
}

pub fn provider_profiles() -> &'static [ProviderProfileSummary] {
    PROVIDER_PROFILES
}

pub fn provider_by_name(name: &str) -> Option<&'static ProviderProfileSummary> {
    PROVIDER_PROFILES
        .iter()
        .find(|profile| profile.name == name)
}

pub fn resolve_provider(name_or_alias: &str) -> Option<&'static ProviderProfileSummary> {
    PROVIDER_PROFILES
        .iter()
        .find(|profile| profile.name == name_or_alias || profile.aliases.contains(&name_or_alias))
}

pub fn fake_chat_completions_request() -> Value {
    json!({
        "max_tokens": 123,
        "messages": [
            {"content": "system", "role": "system"},
            {"content": "hello", "role": "user"}
        ],
        "model": "fake/model",
        "temperature": 0.2,
        "timeout": 30,
        "tools": [
            {
                "function": {
                    "description": "Remember facts.",
                    "name": "memory",
                    "parameters": {
                        "properties": {
                            "action": {"type": "string"}
                        },
                        "required": ["action"],
                        "type": "object"
                    }
                },
                "type": "function"
            }
        ]
    })
}

pub fn chat_completions_strips_codex_leaks_request() -> Value {
    json!({
        "messages": [
            {
                "content": "ok",
                "role": "assistant",
                "tool_calls": [
                    {
                        "function": {
                            "arguments": "{}",
                            "name": "terminal"
                        },
                        "id": "call_1",
                        "type": "function"
                    }
                ]
            }
        ],
        "model": "gpt-4o",
        "timeout": 10
    })
}

pub fn codex_responses_standard_request() -> Value {
    json!({
        "include": ["reasoning.encrypted_content"],
        "input": [
            {"content": "hello", "role": "user"}
        ],
        "instructions": "system",
        "max_output_tokens": 4096,
        "model": "gpt-5.4",
        "parallel_tool_calls": true,
        "prompt_cache_key": "session-1",
        "reasoning": {
            "effort": "low",
            "summary": "auto"
        },
        "store": false,
        "tool_choice": "auto",
        "tools": [
            {
                "description": "Remember facts.",
                "name": "memory",
                "parameters": {
                    "properties": {
                        "action": {"type": "string"}
                    },
                    "required": ["action"],
                    "type": "object"
                },
                "strict": false,
                "type": "function"
            }
        ]
    })
}

pub fn codex_responses_xai_cache_routing_request() -> Value {
    json!({
        "extra_body": {
            "caller": true,
            "prompt_cache_key": "conv-xai-1"
        },
        "extra_headers": {
            "X-Test": "1",
            "x-grok-conv-id": "conv-xai-1"
        },
        "include": [],
        "input": [
            {"content": "hello", "role": "user"}
        ],
        "instructions": "You are Hermes Agent, an intelligent AI assistant created by Nous Research. You are helpful, knowledgeable, and direct. You assist users with a wide range of tasks including answering questions, writing and editing code, analyzing information, creative work, and executing actions via your tools. You communicate clearly, admit uncertainty when appropriate, and prioritize being genuinely useful over being verbose unless otherwise directed below. Be targeted and efficient in your exploration and investigations.",
        "max_output_tokens": 2048,
        "model": "grok-3-mini",
        "reasoning": {
            "effort": "high"
        },
        "store": false,
        "tools": null
    })
}

pub fn anthropic_messages_standard_request() -> Value {
    json!({
        "max_tokens": 1024,
        "messages": [
            {"content": "hello", "role": "user"}
        ],
        "model": "claude-sonnet-4-6",
        "output_config": {
            "effort": "high"
        },
        "system": "system",
        "thinking": {
            "display": "summarized",
            "type": "adaptive"
        },
        "tool_choice": {
            "type": "any"
        },
        "tools": [
            {
                "description": "Remember facts.",
                "input_schema": {
                    "properties": {
                        "action": {"type": "string"}
                    },
                    "required": ["action"],
                    "type": "object"
                },
                "name": "memory"
            }
        ]
    })
}

pub fn chat_completions_service_tier_override_request() -> Value {
    json!({
        "extra_body": {
            "provider": {
                "allow_fallbacks": false
            },
            "trace": "yes"
        },
        "messages": [
            {"content": "system", "role": "system"},
            {"content": "hello", "role": "user"}
        ],
        "model": "fake/model",
        "service_tier": "priority",
        "temperature": 0.2,
        "timeout": 5
    })
}

pub fn normalized_transport_types_fixture() -> Value {
    json!({
        "finish_reason_map": {
            "known": map_finish_reason(Some("tool_use")),
            "none": map_finish_reason(None),
            "unknown": map_finish_reason(Some("something_new")),
        },
        "name": "normalized_transport_types",
        "response": {
            "codex_message_items": [{"id": "msg_1", "type": "message"}],
            "codex_reasoning_items": [{"id": "rs_1"}],
            "content": "answer",
            "finish_reason": "tool_calls",
            "reasoning": "I thought about it",
            "reasoning_content": "hidden chain",
            "reasoning_details": [{"thinking": "hmm", "type": "thinking"}],
        },
        "tool_call": {
            "arguments": "{\"cmd\": \"ls\"}",
            "call_id": "call_3",
            "extra_content": {"google": {"thought_signature": "SIG_ABC123"}},
            "function_arguments": "{\"cmd\": \"ls\"}",
            "function_is_self": true,
            "function_name": "terminal",
            "id": "call_3",
            "name": "terminal",
            "provider_data": {
                "call_id": "call_3",
                "extra_content": {"google": {"thought_signature": "SIG_ABC123"}},
                "response_item_id": "fc_3",
            },
            "response_item_id": "fc_3",
            "type": "function",
        },
        "usage_defaults": {
            "cached_tokens": 0,
            "completion_tokens": 0,
            "prompt_tokens": 0,
            "total_tokens": 0,
        },
    })
}

pub fn map_finish_reason(reason: Option<&str>) -> &'static str {
    match reason {
        Some("end_turn" | "stop_sequence") => "stop",
        Some("tool_use") => "tool_calls",
        Some("max_tokens") => "length",
        Some("refusal") => "content_filter",
        _ => "stop",
    }
}

pub fn model_requires_responses_api(model: &str) -> bool {
    let normalized = model
        .to_ascii_lowercase()
        .rsplit_once('/')
        .map(|(_, model)| model.to_string())
        .unwrap_or_else(|| model.to_ascii_lowercase());
    normalized.starts_with("gpt-5")
}

pub fn provider_model_requires_responses_api(model: &str, provider: Option<&str>) -> bool {
    let normalized_provider = provider.unwrap_or_default().trim().to_ascii_lowercase();
    if normalized_provider == "nous" {
        return false;
    }
    if normalized_provider == "copilot" {
        return copilot_model_requires_responses_api(model);
    }
    model_requires_responses_api(model)
}

fn copilot_model_requires_responses_api(model: &str) -> bool {
    model_requires_responses_api(model)
}

pub fn max_tokens_param(base_url: &str, value: i64) -> Value {
    if is_direct_openai_url(base_url)
        || is_azure_openai_url(base_url)
        || is_github_copilot_url(base_url)
    {
        json!({"max_completion_tokens": value})
    } else {
        json!({"max_tokens": value})
    }
}

pub fn is_direct_openai_url(base_url: &str) -> bool {
    base_url_hostname(base_url) == "api.openai.com"
}

pub fn is_azure_openai_url(base_url: &str) -> bool {
    base_url.to_ascii_lowercase().contains("openai.azure.com")
}

pub fn is_github_copilot_url(base_url: &str) -> bool {
    base_url_hostname(base_url) == "api.githubcopilot.com"
}

fn base_url_hostname(base_url: &str) -> String {
    let lower = base_url.trim().to_ascii_lowercase();
    let without_scheme = lower
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(lower.as_str());
    let authority = without_scheme.split('/').next().unwrap_or_default();
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    host_port
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

#[derive(Debug, Default)]
pub struct CodexEventProjector {
    pending_reasoning: Vec<String>,
}

impl CodexEventProjector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn project(&mut self, notification: &Value) -> Value {
        let method = get_str(notification, "method");
        let params = notification.get("params").unwrap_or(&Value::Null);
        if method != "item/completed" {
            return projection_result(Vec::new(), false, None);
        }

        let item = params.get("item").unwrap_or(&Value::Null);
        let item_type = get_str(item, "type");
        let item_id = get_str(item, "id");
        match item_type {
            "agentMessage" => self.project_agent_message(item),
            "reasoning" => {
                self.extend_reasoning(item.get("summary"));
                self.extend_reasoning(item.get("content"));
                projection_result(Vec::new(), false, None)
            }
            "commandExecution" => self.project_command(item, item_id),
            "fileChange" => self.project_file_change(item, item_id),
            "mcpToolCall" => self.project_mcp_tool_call(item, item_id),
            "dynamicToolCall" => self.project_dynamic_tool_call(item, item_id),
            "userMessage" => self.project_user_message(item),
            _ => self.project_opaque(item, item_type),
        }
    }

    fn extend_reasoning(&mut self, value: Option<&Value>) {
        if let Some(items) = value.and_then(Value::as_array) {
            self.pending_reasoning.extend(
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned),
            );
        }
    }

    fn project_agent_message(&mut self, item: &Value) -> Value {
        let text = get_str(item, "text").to_string();
        let mut message = json!({"role": "assistant", "content": text});
        self.attach_pending_reasoning(&mut message);
        projection_result(
            vec![message],
            false,
            Some(get_str(item, "text").to_string()),
        )
    }

    fn project_user_message(&mut self, item: &Value) -> Value {
        let mut text_parts = Vec::new();
        if let Some(fragments) = item.get("content").and_then(Value::as_array) {
            for fragment in fragments {
                if !fragment.is_object() {
                    continue;
                }
                if get_str(fragment, "type") == "text" {
                    text_parts.push(get_str(fragment, "text").to_string());
                } else if let Some(text) = fragment.get("text") {
                    if let Some(text) = text.as_str() {
                        text_parts.push(text.to_string());
                    } else {
                        text_parts.push(text.to_string());
                    }
                }
            }
        }
        projection_result(
            vec![json!({"role": "user", "content": text_parts.join("\n")})],
            false,
            None,
        )
    }

    fn project_command(&mut self, item: &Value, item_id: &str) -> Value {
        let call_id = deterministic_call_id("exec", item_id);
        let args = json!({
            "command": get_str(item, "command"),
            "cwd": get_str(item, "cwd"),
        });
        let mut assistant = tool_call_message(&call_id, "exec_command", &python_json_sorted(&args));
        self.attach_pending_reasoning(&mut assistant);

        let mut output = get_str(item, "aggregatedOutput").to_string();
        if let Some(exit_code) = item.get("exitCode").and_then(Value::as_i64) {
            if exit_code != 0 {
                output = format!("[exit {exit_code}]\n{output}");
            }
        }
        let tool = json!({
            "role": "tool",
            "tool_call_id": call_id,
            "content": output,
        });
        projection_result(vec![assistant, tool], true, None)
    }

    fn project_file_change(&mut self, item: &Value, item_id: &str) -> Value {
        let call_id = deterministic_call_id("apply_patch", item_id);
        let mut changes = Vec::new();
        if let Some(items) = item.get("changes").and_then(Value::as_array) {
            for change in items {
                let kind = change
                    .get("kind")
                    .and_then(|kind| kind.get("type"))
                    .and_then(Value::as_str)
                    .unwrap_or("update");
                changes.push(json!({
                    "kind": kind,
                    "path": get_str(change, "path"),
                }));
            }
        }
        let args = json!({"changes": changes});
        let mut assistant = tool_call_message(&call_id, "apply_patch", &python_json_sorted(&args));
        self.attach_pending_reasoning(&mut assistant);
        let tool = json!({
            "role": "tool",
            "tool_call_id": call_id,
            "content": format!(
                "apply_patch status={}, {} change(s)",
                value_str_or(item.get("status"), "unknown"),
                item.get("changes").and_then(Value::as_array).map_or(0, Vec::len)
            ),
        });
        projection_result(vec![assistant, tool], true, None)
    }

    fn project_mcp_tool_call(&mut self, item: &Value, item_id: &str) -> Value {
        let server = value_str_or(item.get("server"), "mcp");
        let tool_name = value_str_or(item.get("tool"), "unknown");
        let call_id = deterministic_call_id(&format!("mcp_{server}_{tool_name}"), item_id);
        let arguments = item
            .get("arguments")
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(
                || json!({"arguments": item.get("arguments").cloned().unwrap_or(Value::Null)}),
            );
        let mut assistant = tool_call_message(
            &call_id,
            &format!("mcp.{server}.{tool_name}"),
            &python_json_sorted(&arguments),
        );
        self.attach_pending_reasoning(&mut assistant);
        let content = if let Some(error) = item.get("error").filter(|value| !value.is_null()) {
            format!(
                "[error] {}",
                truncate_chars(&python_json_in_order(error), 1000)
            )
        } else if let Some(result) = item.get("result").filter(|value| !value.is_null()) {
            truncate_chars(&python_json_in_order(result), 4000)
        } else {
            String::new()
        };
        let tool = json!({
            "role": "tool",
            "tool_call_id": call_id,
            "content": content,
        });
        projection_result(vec![assistant, tool], true, None)
    }

    fn project_dynamic_tool_call(&mut self, item: &Value, item_id: &str) -> Value {
        let tool_name = value_str_or(item.get("tool"), "unknown");
        let call_id = deterministic_call_id(&format!("dyn_{tool_name}"), item_id);
        let arguments = item
            .get("arguments")
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(
                || json!({"arguments": item.get("arguments").cloned().unwrap_or(Value::Null)}),
            );
        let mut assistant = tool_call_message(&call_id, tool_name, &python_json_sorted(&arguments));
        self.attach_pending_reasoning(&mut assistant);
        let content = item
            .get("contentItems")
            .and_then(Value::as_array)
            .filter(|items| !items.is_empty())
            .map(|_| truncate_chars(&python_json_in_order(&item["contentItems"]), 4000))
            .unwrap_or_else(|| format!("success={}", python_bool(item.get("success"))));
        let tool = json!({
            "role": "tool",
            "tool_call_id": call_id,
            "content": content,
        });
        projection_result(vec![assistant, tool], true, None)
    }

    fn project_opaque(&mut self, item: &Value, item_type: &str) -> Value {
        let payload = truncate_chars(&python_json_in_order(item), 1500);
        projection_result(
            vec![json!({
                "role": "assistant",
                "content": format!("[codex {item_type}] {payload}"),
            })],
            false,
            None,
        )
    }

    fn attach_pending_reasoning(&mut self, message: &mut Value) {
        if self.pending_reasoning.is_empty() {
            return;
        }
        if let Some(object) = message.as_object_mut() {
            object.insert(
                "reasoning".to_string(),
                json!(self.pending_reasoning.join("\n")),
            );
        }
        self.pending_reasoning.clear();
    }
}

pub fn project_codex_event_notifications(notifications: &[Value]) -> Vec<Value> {
    let mut projector = CodexEventProjector::new();
    notifications
        .iter()
        .map(|notification| projector.project(notification))
        .collect()
}

pub fn stream_diag_capture_response(status_code: i64, headers: &Value) -> Value {
    let mut captured = serde_json::Map::new();
    for name in STREAM_DIAG_HEADERS {
        if let Some(value) = headers.get(name).and_then(Value::as_str) {
            if !value.is_empty() {
                captured.insert(name.to_string(), json!(truncate_chars(value, 120)));
            }
        }
    }
    json!({
        "started_at": 1000.0,
        "first_chunk_at": null,
        "chunks": 0,
        "bytes": 0,
        "headers": captured,
        "http_status": status_code,
    })
}

pub fn flatten_exception_chain(parts: &[(&str, &str)]) -> String {
    parts
        .iter()
        .take(4)
        .map(|(class_name, message)| {
            let mut message = message.trim().replace('\n', " ");
            if message.chars().count() > 140 {
                message = format!("{}\u{2026}", truncate_chars(&message, 140));
            }
            if message.is_empty() {
                (*class_name).to_string()
            } else {
                format!("{class_name}({message})")
            }
        })
        .collect::<Vec<_>>()
        .join(" <- ")
}

pub fn stream_drop_emit_events(
    provider: &str,
    error_type: &str,
    attempt: i64,
    max_attempts: i64,
    mid_tool_call: bool,
    started_at: f64,
    now: f64,
) -> Value {
    let kind = if mid_tool_call {
        "drop mid tool-call"
    } else {
        "drop"
    };
    let suffix = format!(" after {:.1}s", (now - started_at).max(0.0));
    json!({
        "status_events": [
            format!(
                "\u{26a0}\u{fe0f} {provider} stream {kind} ({error_type}){suffix} \u{2014} reconnecting, retry {attempt}/{max_attempts}"
            )
        ],
        "activity_events": [
            format!("stream retry {attempt}/{max_attempts} after {error_type}")
        ],
    })
}

pub fn classify_provider_error(input: &Value) -> Value {
    let error_type = get_str(input, "error_type");
    let raw_message = get_str(input, "message");
    let body = input.get("body").unwrap_or(&Value::Null);
    let mut status_code = input.get("status_code").and_then(Value::as_i64);
    if status_code.is_none() && error_type == "RateLimitError" {
        status_code = Some(429);
    }
    let body_message = extract_body_message(body);
    let body_code = extract_body_code(body);
    let mut haystack = raw_message.to_ascii_lowercase();
    if let Some(message) = body_message.as_ref() {
        let lower = message.to_ascii_lowercase();
        if !lower.is_empty() && !haystack.contains(&lower) {
            haystack.push(' ');
            haystack.push_str(&lower);
        }
    }
    let result_message = body_message
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| truncate_chars(raw_message, 500));
    let approx_tokens = input
        .get("approx_tokens")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let context_length = input
        .get("context_length")
        .and_then(Value::as_i64)
        .unwrap_or(200000);
    let num_messages = input
        .get("num_messages")
        .and_then(Value::as_i64)
        .unwrap_or(0);

    if status_code == Some(400) && haystack.contains("signature") && haystack.contains("thinking") {
        return classified(
            "thinking_signature",
            status_code,
            &result_message,
            true,
            false,
            false,
            false,
        );
    }
    if status_code == Some(429)
        && haystack.contains("extra usage")
        && haystack.contains("long context")
    {
        return classified(
            "long_context_tier",
            status_code,
            &result_message,
            true,
            true,
            false,
            false,
        );
    }
    if status_code == Some(400)
        && (haystack.contains("error parsing grammar")
            || haystack.contains("json-schema-to-grammar")
            || (haystack.contains("unable to generate parser") && haystack.contains("template")))
    {
        return classified(
            "llama_cpp_grammar_pattern",
            status_code,
            &result_message,
            true,
            false,
            false,
            false,
        );
    }
    if haystack.contains("do not have an active grok subscription")
        || (haystack.contains("out of available resources") && haystack.contains("grok"))
    {
        return classified(
            "auth",
            status_code,
            &result_message,
            false,
            false,
            false,
            true,
        );
    }

    if let Some(status) = status_code {
        let status_result = match status {
            401 => Some(classified(
                "auth",
                status_code,
                &result_message,
                false,
                false,
                true,
                true,
            )),
            403 if haystack.contains("key limit exceeded")
                || haystack.contains("spending limit") =>
            {
                Some(classified(
                    "billing",
                    status_code,
                    &result_message,
                    false,
                    false,
                    true,
                    true,
                ))
            }
            403 => Some(classified(
                "auth",
                status_code,
                &result_message,
                false,
                false,
                false,
                true,
            )),
            402 if has_any(&haystack, USAGE_LIMIT_PATTERNS)
                && has_any(&haystack, USAGE_LIMIT_TRANSIENT_SIGNALS) =>
            {
                Some(classified(
                    "rate_limit",
                    status_code,
                    &result_message,
                    true,
                    false,
                    true,
                    true,
                ))
            }
            402 => Some(classified(
                "billing",
                status_code,
                &result_message,
                false,
                false,
                true,
                true,
            )),
            404 if has_any(&haystack, PROVIDER_POLICY_BLOCKED_PATTERNS) => Some(classified(
                "provider_policy_blocked",
                status_code,
                &result_message,
                false,
                false,
                false,
                false,
            )),
            404 if has_any(&haystack, MODEL_NOT_FOUND_PATTERNS) => Some(classified(
                "model_not_found",
                status_code,
                &result_message,
                false,
                false,
                false,
                true,
            )),
            404 => Some(classified(
                "unknown",
                status_code,
                &result_message,
                true,
                false,
                false,
                false,
            )),
            413 => Some(classified(
                "payload_too_large",
                status_code,
                &result_message,
                true,
                true,
                false,
                false,
            )),
            429 => Some(classified(
                "rate_limit",
                status_code,
                &result_message,
                true,
                false,
                true,
                true,
            )),
            400 => Some(classify_bad_request(
                &haystack,
                status_code,
                &result_message,
                body,
                approx_tokens,
                context_length,
                num_messages,
            )),
            500 | 502 => Some(classified(
                "server_error",
                status_code,
                &result_message,
                true,
                false,
                false,
                false,
            )),
            503 | 529 => Some(classified(
                "overloaded",
                status_code,
                &result_message,
                true,
                false,
                false,
                false,
            )),
            code if (400..500).contains(&code) => Some(classified(
                "format_error",
                status_code,
                &result_message,
                false,
                false,
                false,
                true,
            )),
            500..=599 => Some(classified(
                "server_error",
                status_code,
                &result_message,
                true,
                false,
                false,
                false,
            )),
            _ => None,
        };
        if let Some(result) = status_result {
            return result;
        }
    }

    if let Some(result) = classify_by_error_code(&body_code, &result_message, status_code) {
        return result;
    }
    if let Some(result) = classify_by_message(&haystack, status_code, &result_message) {
        return result;
    }
    if has_any(&haystack, SSL_TRANSIENT_PATTERNS) {
        return classified(
            "timeout",
            status_code,
            &result_message,
            true,
            false,
            false,
            false,
        );
    }
    if status_code.is_none() && has_any(&haystack, SERVER_DISCONNECT_PATTERNS) {
        let is_large = approx_tokens as f64 > context_length as f64 * 0.6
            || (context_length <= 256000 && (approx_tokens > 120000 || num_messages > 200));
        return if is_large {
            classified(
                "context_overflow",
                status_code,
                &result_message,
                true,
                true,
                false,
                false,
            )
        } else {
            classified(
                "timeout",
                status_code,
                &result_message,
                true,
                false,
                false,
                false,
            )
        };
    }
    if TRANSPORT_ERROR_TYPES.contains(&error_type) {
        return classified(
            "timeout",
            status_code,
            &result_message,
            true,
            false,
            false,
            false,
        );
    }
    classified(
        "unknown",
        status_code,
        &result_message,
        true,
        false,
        false,
        false,
    )
}

fn projection_result(
    messages: Vec<Value>,
    is_tool_iteration: bool,
    final_text: Option<String>,
) -> Value {
    json!({
        "messages": messages,
        "is_tool_iteration": is_tool_iteration,
        "final_text": final_text,
    })
}

fn deterministic_call_id(item_type: &str, item_id: &str) -> String {
    if !item_id.is_empty() {
        format!("codex_{item_type}_{item_id}")
    } else {
        format!("codex_{item_type}_")
    }
}

fn tool_call_message(call_id: &str, name: &str, arguments: &str) -> Value {
    json!({
        "role": "assistant",
        "content": null,
        "tool_calls": [
            {
                "id": call_id,
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": arguments,
                },
            }
        ],
    })
}

fn get_str<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

fn value_str_or<'a>(value: Option<&'a Value>, fallback: &'a str) -> &'a str {
    value.and_then(Value::as_str).unwrap_or(fallback)
}

fn python_bool(value: Option<&Value>) -> &'static str {
    match value.and_then(Value::as_bool) {
        Some(true) => "True",
        Some(false) => "False",
        None => "None",
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn python_json_sorted(value: &Value) -> String {
    match value {
        Value::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(python_json_sorted)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            format!(
                "{{{}}}",
                keys.into_iter()
                    .map(|key| format!(
                        "{}: {}",
                        serde_json::to_string(key).unwrap(),
                        python_json_sorted(&object[key])
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        _ => python_json_scalar(value),
    }
}

fn python_json_in_order(value: &Value) -> String {
    match value {
        Value::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(python_json_in_order)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Object(object) => {
            let keys = object.keys().collect::<Vec<_>>();
            format!(
                "{{{}}}",
                keys.into_iter()
                    .map(|key| format!(
                        "{}: {}",
                        serde_json::to_string(key).unwrap(),
                        python_json_in_order(&object[key])
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        _ => python_json_scalar(value),
    }
}

fn python_json_scalar(value: &Value) -> String {
    match value {
        Value::Bool(true) => "true".to_string(),
        Value::Bool(false) => "false".to_string(),
        Value::Null => "null".to_string(),
        _ => serde_json::to_string(value).unwrap(),
    }
}

fn classified(
    reason: &str,
    status_code: Option<i64>,
    message: &str,
    retryable: bool,
    should_compress: bool,
    should_rotate_credential: bool,
    should_fallback: bool,
) -> Value {
    json!({
        "reason": reason,
        "status_code": status_code,
        "message": message,
        "retryable": retryable,
        "should_compress": should_compress,
        "should_rotate_credential": should_rotate_credential,
        "should_fallback": should_fallback,
        "is_auth": matches!(reason, "auth" | "auth_permanent"),
    })
}

fn classify_bad_request(
    haystack: &str,
    status_code: Option<i64>,
    message: &str,
    body: &Value,
    approx_tokens: i64,
    context_length: i64,
    num_messages: i64,
) -> Value {
    let body_code = extract_body_code(body);
    if has_any(haystack, IMAGE_TOO_LARGE_PATTERNS) {
        return classified(
            "image_too_large",
            status_code,
            message,
            true,
            false,
            false,
            false,
        );
    }
    if has_any(haystack, CONTEXT_OVERFLOW_PATTERNS) {
        return classified(
            "context_overflow",
            status_code,
            message,
            true,
            true,
            false,
            false,
        );
    }
    if has_any(haystack, PROVIDER_POLICY_BLOCKED_PATTERNS) {
        return classified(
            "provider_policy_blocked",
            status_code,
            message,
            false,
            false,
            false,
            false,
        );
    }
    if has_any(haystack, MODEL_NOT_FOUND_PATTERNS) {
        return classified(
            "model_not_found",
            status_code,
            message,
            false,
            false,
            false,
            true,
        );
    }
    if has_any(haystack, RATE_LIMIT_PATTERNS) || matches!(body_code.as_str(), "resource_exhausted")
    {
        return classified("rate_limit", status_code, message, true, false, true, true);
    }
    if has_any(haystack, BILLING_PATTERNS) {
        return classified("billing", status_code, message, false, false, true, true);
    }

    let body_msg = extract_body_message(body)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let is_generic = body_msg.len() < 30 || matches!(body_msg.as_str(), "error" | "");
    let is_large = approx_tokens as f64 > context_length as f64 * 0.4
        || (context_length <= 256000 && (approx_tokens > 80000 || num_messages > 80));
    if is_generic && is_large {
        return classified(
            "context_overflow",
            status_code,
            message,
            true,
            true,
            false,
            false,
        );
    }
    classified(
        "format_error",
        status_code,
        message,
        false,
        false,
        false,
        true,
    )
}

fn classify_by_error_code(code: &str, message: &str, status_code: Option<i64>) -> Option<Value> {
    let lower = code.to_ascii_lowercase();
    match lower.as_str() {
        "resource_exhausted" | "throttled" | "rate_limit_exceeded" => Some(classified(
            "rate_limit",
            status_code,
            message,
            true,
            false,
            true,
            false,
        )),
        "insufficient_quota" | "billing_not_active" | "payment_required" => Some(classified(
            "billing",
            status_code,
            message,
            false,
            false,
            true,
            true,
        )),
        "model_not_found" | "model_not_available" | "invalid_model" => Some(classified(
            "model_not_found",
            status_code,
            message,
            false,
            false,
            false,
            true,
        )),
        "context_length_exceeded" | "max_tokens_exceeded" => Some(classified(
            "context_overflow",
            status_code,
            message,
            true,
            true,
            false,
            false,
        )),
        _ => None,
    }
}

fn classify_by_message(haystack: &str, status_code: Option<i64>, message: &str) -> Option<Value> {
    if has_any(haystack, PAYLOAD_TOO_LARGE_PATTERNS) {
        return Some(classified(
            "payload_too_large",
            status_code,
            message,
            true,
            true,
            false,
            false,
        ));
    }
    if has_any(haystack, IMAGE_TOO_LARGE_PATTERNS) {
        return Some(classified(
            "image_too_large",
            status_code,
            message,
            true,
            false,
            false,
            false,
        ));
    }
    if has_any(haystack, USAGE_LIMIT_PATTERNS) {
        return Some(if has_any(haystack, USAGE_LIMIT_TRANSIENT_SIGNALS) {
            classified("rate_limit", status_code, message, true, false, true, true)
        } else {
            classified("billing", status_code, message, false, false, true, true)
        });
    }
    if has_any(haystack, BILLING_PATTERNS) {
        return Some(classified(
            "billing",
            status_code,
            message,
            false,
            false,
            true,
            true,
        ));
    }
    if has_any(haystack, RATE_LIMIT_PATTERNS) {
        return Some(classified(
            "rate_limit",
            status_code,
            message,
            true,
            false,
            true,
            true,
        ));
    }
    if has_any(haystack, CONTEXT_OVERFLOW_PATTERNS) {
        return Some(classified(
            "context_overflow",
            status_code,
            message,
            true,
            true,
            false,
            false,
        ));
    }
    if has_any(haystack, AUTH_PATTERNS) {
        return Some(classified(
            "auth",
            status_code,
            message,
            false,
            false,
            true,
            true,
        ));
    }
    if has_any(haystack, PROVIDER_POLICY_BLOCKED_PATTERNS) {
        return Some(classified(
            "provider_policy_blocked",
            status_code,
            message,
            false,
            false,
            false,
            false,
        ));
    }
    if has_any(haystack, MODEL_NOT_FOUND_PATTERNS) {
        return Some(classified(
            "model_not_found",
            status_code,
            message,
            false,
            false,
            false,
            true,
        ));
    }
    if has_any(haystack, TIMEOUT_MESSAGE_PATTERNS) {
        return Some(classified(
            "timeout",
            status_code,
            message,
            true,
            false,
            false,
            false,
        ));
    }
    None
}

fn extract_body_message(body: &Value) -> Option<String> {
    body.get("error")
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .or_else(|| body.get("message").and_then(Value::as_str))
        .map(|message| truncate_chars(message.trim(), 500))
}

fn extract_body_code(body: &Value) -> String {
    body.get("error")
        .and_then(|error| error.get("code").or_else(|| error.get("type")))
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .or_else(|| {
            body.get("code")
                .or_else(|| body.get("error_code"))
                .map(|value| {
                    value
                        .as_str()
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| value.to_string())
                })
        })
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn has_any(haystack: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| haystack.contains(pattern))
}

const STREAM_DIAG_HEADERS: &[&str] = &[
    "cf-ray",
    "cf-cache-status",
    "x-openrouter-provider",
    "x-openrouter-model",
    "x-openrouter-id",
    "x-request-id",
    "x-vercel-id",
    "via",
    "server",
    "x-forwarded-for",
];

const BILLING_PATTERNS: &[&str] = &[
    "insufficient credits",
    "insufficient_quota",
    "insufficient balance",
    "credit balance",
    "credits have been exhausted",
    "top up your credits",
    "payment required",
    "billing hard limit",
    "exceeded your current quota",
    "account is deactivated",
    "plan does not include",
];

const RATE_LIMIT_PATTERNS: &[&str] = &[
    "rate limit",
    "rate_limit",
    "too many requests",
    "throttled",
    "requests per minute",
    "tokens per minute",
    "requests per day",
    "try again in",
    "please retry after",
    "resource_exhausted",
    "rate increased too quickly",
    "throttlingexception",
    "too many concurrent requests",
    "servicequotaexceededexception",
];

const USAGE_LIMIT_PATTERNS: &[&str] = &[
    "usage limit",
    "quota",
    "limit exceeded",
    "key limit exceeded",
];

const USAGE_LIMIT_TRANSIENT_SIGNALS: &[&str] = &[
    "try again",
    "retry",
    "resets at",
    "reset in",
    "wait",
    "requests remaining",
    "periodic",
    "window",
];

const PAYLOAD_TOO_LARGE_PATTERNS: &[&str] = &[
    "request entity too large",
    "payload too large",
    "error code: 413",
];

const IMAGE_TOO_LARGE_PATTERNS: &[&str] = &[
    "image exceeds",
    "image too large",
    "image_too_large",
    "image size exceeds",
];

const CONTEXT_OVERFLOW_PATTERNS: &[&str] = &[
    "context length",
    "context size",
    "maximum context",
    "token limit",
    "too many tokens",
    "reduce the length",
    "exceeds the limit",
    "context window",
    "prompt is too long",
    "prompt exceeds max length",
    "max_tokens",
    "maximum number of tokens",
    "exceeds the max_model_len",
    "max_model_len",
    "prompt length",
    "input is too long",
    "maximum model length",
    "context length exceeded",
    "truncating input",
    "slot context",
    "n_ctx_slot",
    "max input token",
    "input token",
    "exceeds the maximum number of input tokens",
];

const MODEL_NOT_FOUND_PATTERNS: &[&str] = &[
    "is not a valid model",
    "invalid model",
    "model not found",
    "model_not_found",
    "does not exist",
    "no such model",
    "unknown model",
    "unsupported model",
];

const PROVIDER_POLICY_BLOCKED_PATTERNS: &[&str] = &[
    "no endpoints available matching your guardrail",
    "no endpoints available matching your data policy",
    "no endpoints found matching your data policy",
];

const AUTH_PATTERNS: &[&str] = &[
    "invalid api key",
    "invalid_api_key",
    "authentication",
    "unauthorized",
    "forbidden",
    "invalid token",
    "token expired",
    "token revoked",
    "access denied",
];

const TIMEOUT_MESSAGE_PATTERNS: &[&str] = &[
    "timed out",
    "turn timed out",
    "request timed out",
    "deadline exceeded",
    "operation timed out",
    "upstream timed out",
];

const TRANSPORT_ERROR_TYPES: &[&str] = &[
    "ReadTimeout",
    "ConnectTimeout",
    "PoolTimeout",
    "ConnectError",
    "RemoteProtocolError",
    "ConnectionError",
    "ConnectionResetError",
    "ConnectionAbortedError",
    "BrokenPipeError",
    "TimeoutError",
    "ReadError",
    "ServerDisconnectedError",
    "SSLError",
    "SSLZeroReturnError",
    "SSLWantReadError",
    "SSLWantWriteError",
    "SSLEOFError",
    "SSLSyscallError",
    "APIConnectionError",
    "APITimeoutError",
];

const SERVER_DISCONNECT_PATTERNS: &[&str] = &[
    "server disconnected",
    "peer closed connection",
    "connection reset by peer",
    "connection was closed",
    "network connection lost",
    "unexpected eof",
    "incomplete chunked read",
];

const SSL_TRANSIENT_PATTERNS: &[&str] = &[
    "bad record mac",
    "ssl alert",
    "tls alert",
    "ssl handshake failure",
    "tlsv1 alert",
    "sslv3 alert",
    "bad_record_mac",
    "ssl_alert",
    "tls_alert",
    "tls_alert_internal_error",
    "[ssl:",
];

const PROVIDER_PROFILES: &[ProviderProfileSummary] = &[
    ProviderProfileSummary {
        name: "ai-gateway",
        aliases: &["ai_gateway", "aigateway", "vercel", "vercel-ai-gateway"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["AI_GATEWAY_API_KEY"],
        base_url: "https://ai-gateway.vercel.sh/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &["HTTP-Referer", "X-Title"],
    },
    ProviderProfileSummary {
        name: "alibaba",
        aliases: &["alibaba-cloud", "dashscope", "qwen-dashscope"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["DASHSCOPE_API_KEY"],
        base_url: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "alibaba-coding-plan",
        aliases: &["alibaba-coding", "alibaba_coding", "dashscope-coding"],
        api_mode: "chat_completions",
        display_name: "Alibaba Cloud (Coding Plan)",
        env_vars: &[
            "ALIBABA_CODING_PLAN_API_KEY",
            "DASHSCOPE_API_KEY",
            "ALIBABA_CODING_PLAN_BASE_URL",
        ],
        base_url: "https://coding-intl.dashscope.aliyuncs.com/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "anthropic",
        aliases: &["claude", "claude-code", "claude-oauth"],
        api_mode: "anthropic_messages",
        display_name: "",
        env_vars: &[
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_TOKEN",
            "CLAUDE_CODE_OAUTH_TOKEN",
        ],
        base_url: "https://api.anthropic.com",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "arcee",
        aliases: &["arcee-ai", "arceeai"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["ARCEEAI_API_KEY"],
        base_url: "https://api.arcee.ai/api/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "azure-foundry",
        aliases: &["azure", "azure-ai", "azure-ai-foundry"],
        api_mode: "chat_completions",
        display_name: "Azure Foundry",
        env_vars: &["AZURE_FOUNDRY_API_KEY", "AZURE_FOUNDRY_BASE_URL"],
        base_url: "",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "bedrock",
        aliases: &["amazon", "amazon-bedrock", "aws", "aws-bedrock"],
        api_mode: "bedrock_converse",
        display_name: "",
        env_vars: &[],
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com",
        auth_type: "aws_sdk",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "copilot",
        aliases: &["github", "github-copilot", "github-model", "github-models"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["COPILOT_GITHUB_TOKEN", "GH_TOKEN", "GITHUB_TOKEN"],
        base_url: "https://api.githubcopilot.com",
        auth_type: "copilot",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "copilot-acp",
        aliases: &["copilot-acp-agent", "github-copilot-acp"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &[],
        base_url: "acp://copilot",
        auth_type: "external_process",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "custom",
        aliases: &[
            "llama-cpp",
            "llama.cpp",
            "llamacpp",
            "local",
            "ollama",
            "vllm",
        ],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &[],
        base_url: "",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "deepseek",
        aliases: &["deepseek-chat"],
        api_mode: "chat_completions",
        display_name: "DeepSeek",
        env_vars: &["DEEPSEEK_API_KEY"],
        base_url: "https://api.deepseek.com/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 2,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "gemini",
        aliases: &["google", "google-ai-studio", "google-gemini"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["GOOGLE_API_KEY", "GEMINI_API_KEY"],
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "gmi",
        aliases: &["gmi-cloud", "gmicloud"],
        api_mode: "chat_completions",
        display_name: "GMI Cloud",
        env_vars: &["GMI_API_KEY", "GMI_BASE_URL"],
        base_url: "https://api.gmi-serving.com/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 6,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &["User-Agent"],
    },
    ProviderProfileSummary {
        name: "google-gemini-cli",
        aliases: &["gemini-cli", "gemini-oauth"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &[],
        base_url: "cloudcode-pa://google",
        auth_type: "oauth_external",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "huggingface",
        aliases: &["hf", "hugging-face", "huggingface-hub"],
        api_mode: "chat_completions",
        display_name: "HuggingFace",
        env_vars: &["HF_TOKEN"],
        base_url: "https://router.huggingface.co/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 2,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "kilocode",
        aliases: &["kilo", "kilo-code", "kilo-gateway"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["KILOCODE_API_KEY"],
        base_url: "https://api.kilo.ai/api/gateway",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "kimi-coding",
        aliases: &["kimi", "kimi-for-coding", "moonshot"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["KIMI_API_KEY", "KIMI_CODING_API_KEY"],
        base_url: "https://api.moonshot.ai/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: Some(32000),
        fixed_temperature: Some("omit"),
        default_header_keys: &["User-Agent"],
    },
    ProviderProfileSummary {
        name: "kimi-coding-cn",
        aliases: &["kimi-cn", "moonshot-cn"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["KIMI_CN_API_KEY"],
        base_url: "https://api.moonshot.cn/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: Some(32000),
        fixed_temperature: Some("omit"),
        default_header_keys: &["User-Agent"],
    },
    ProviderProfileSummary {
        name: "minimax",
        aliases: &["mini-max"],
        api_mode: "anthropic_messages",
        display_name: "",
        env_vars: &["MINIMAX_API_KEY"],
        base_url: "https://api.minimax.io/anthropic",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "minimax-cn",
        aliases: &["minimax-china", "minimax_cn"],
        api_mode: "anthropic_messages",
        display_name: "",
        env_vars: &["MINIMAX_CN_API_KEY"],
        base_url: "https://api.minimaxi.com/anthropic",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "minimax-oauth",
        aliases: &["minimax-oauth-io", "minimax_oauth"],
        api_mode: "anthropic_messages",
        display_name: "MiniMax (OAuth)",
        env_vars: &[],
        base_url: "https://api.minimax.io/anthropic",
        auth_type: "oauth_external",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "nous",
        aliases: &["nous-portal", "nousresearch"],
        api_mode: "chat_completions",
        display_name: "Nous Research",
        env_vars: &["NOUS_API_KEY"],
        base_url: "https://inference.nousresearch.com/v1",
        auth_type: "oauth_device_code",
        supports_health_check: true,
        fallback_model_count: 2,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "novita",
        aliases: &["novita-ai", "novitaai"],
        api_mode: "chat_completions",
        display_name: "NovitaAI",
        env_vars: &["NOVITA_API_KEY", "NOVITA_BASE_URL"],
        base_url: "https://api.novita.ai/openai/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 6,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "nvidia",
        aliases: &["nvidia-nim"],
        api_mode: "chat_completions",
        display_name: "NVIDIA NIM",
        env_vars: &["NVIDIA_API_KEY"],
        base_url: "https://integrate.api.nvidia.com/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 2,
        default_max_tokens: Some(16384),
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "ollama-cloud",
        aliases: &["ollama_cloud"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["OLLAMA_API_KEY"],
        base_url: "https://ollama.com/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "openai-codex",
        aliases: &["codex", "openai_codex"],
        api_mode: "codex_responses",
        display_name: "",
        env_vars: &[],
        base_url: "https://chatgpt.com/backend-api/codex",
        auth_type: "oauth_external",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "opencode-go",
        aliases: &["go", "opencode-go-sub", "opencode_go"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["OPENCODE_GO_API_KEY"],
        base_url: "https://opencode.ai/zen/go/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "opencode-zen",
        aliases: &["opencode", "opencode_zen", "zen"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["OPENCODE_ZEN_API_KEY"],
        base_url: "https://opencode.ai/zen/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "openrouter",
        aliases: &["or"],
        api_mode: "chat_completions",
        display_name: "OpenRouter",
        env_vars: &["OPENROUTER_API_KEY"],
        base_url: "https://openrouter.ai/api/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 5,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "qwen-oauth",
        aliases: &["qwen", "qwen-cli", "qwen-portal"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["QWEN_API_KEY"],
        base_url: "https://portal.qwen.ai/v1",
        auth_type: "oauth_external",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: Some(65536),
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "stepfun",
        aliases: &["step", "stepfun-coding-plan"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["STEPFUN_API_KEY"],
        base_url: "https://api.stepfun.ai/step_plan/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "xai",
        aliases: &["grok", "x-ai", "x.ai"],
        api_mode: "codex_responses",
        display_name: "",
        env_vars: &["XAI_API_KEY"],
        base_url: "https://api.x.ai/v1",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "xiaomi",
        aliases: &["mimo", "xiaomi-mimo"],
        api_mode: "chat_completions",
        display_name: "",
        env_vars: &["XIAOMI_API_KEY"],
        base_url: "https://api.xiaomimimo.com/v1",
        auth_type: "api_key",
        supports_health_check: false,
        fallback_model_count: 0,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
    ProviderProfileSummary {
        name: "zai",
        aliases: &["glm", "z-ai", "z.ai", "zhipu"],
        api_mode: "chat_completions",
        display_name: "Z.AI (GLM)",
        env_vars: &["GLM_API_KEY", "ZAI_API_KEY", "Z_AI_API_KEY"],
        base_url: "https://api.z.ai/api/paas/v4",
        auth_type: "api_key",
        supports_health_check: true,
        fallback_model_count: 2,
        default_max_tokens: None,
        fixed_temperature: None,
        default_header_keys: &[],
    },
];
