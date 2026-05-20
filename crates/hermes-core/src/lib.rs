use hermes_provider::{ChatProvider, ToolCall};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

pub type ToolHandler = Box<dyn Fn(&Value) -> Value + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationResult {
    pub final_response: String,
    pub messages: Vec<Value>,
    pub api_call_count: usize,
    pub stopped_for_iteration_limit: bool,
}

pub struct AgentRuntime<P> {
    provider: P,
    max_iterations: usize,
    tools: BTreeMap<String, ToolHandler>,
}

#[derive(Debug)]
pub struct IterationBudget {
    max_total: usize,
    used: Mutex<usize>,
}

impl IterationBudget {
    pub fn new(max_total: usize) -> Self {
        Self {
            max_total,
            used: Mutex::new(0),
        }
    }

    pub fn consume(&self) -> bool {
        let mut used = self.used.lock().unwrap();
        if *used >= self.max_total {
            return false;
        }
        *used += 1;
        true
    }

    pub fn refund(&self) {
        let mut used = self.used.lock().unwrap();
        if *used > 0 {
            *used -= 1;
        }
    }

    pub fn used(&self) -> usize {
        *self.used.lock().unwrap()
    }

    pub fn remaining(&self) -> usize {
        self.max_total.saturating_sub(self.used())
    }
}

impl<P: ChatProvider> AgentRuntime<P> {
    pub fn new(provider: P, max_iterations: usize) -> Self {
        Self {
            provider,
            max_iterations,
            tools: BTreeMap::new(),
        }
    }

    pub fn register_tool(
        &mut self,
        name: impl Into<String>,
        handler: impl Fn(&Value) -> Value + Send + Sync + 'static,
    ) {
        self.tools.insert(name.into(), Box::new(handler));
    }

    pub fn run_conversation(&mut self, mut messages: Vec<Value>) -> ConversationResult {
        let mut api_call_count = 0;

        while api_call_count < self.max_iterations {
            let response = self.provider.chat(json!({
                "messages": messages,
                "tools": self.tools.keys().cloned().collect::<Vec<_>>(),
            }));
            api_call_count += 1;

            if response.tool_calls.is_empty() {
                let final_response = response.content.unwrap_or_default();
                messages.push(json!({
                    "content": final_response,
                    "role": "assistant",
                }));
                return ConversationResult {
                    final_response,
                    messages,
                    api_call_count,
                    stopped_for_iteration_limit: false,
                };
            }

            messages.push(json!({
                "content": response.content,
                "role": "assistant",
                "tool_calls": provider_tool_calls_json(&response.tool_calls),
            }));

            for tool_call in response.tool_calls {
                let result = self
                    .tools
                    .get(&tool_call.name)
                    .map(|handler| handler(&tool_call.arguments))
                    .unwrap_or_else(|| {
                        json!({
                            "error": format!("Unknown tool '{}'.", tool_call.name),
                            "success": false,
                        })
                    });
                messages.push(json!({
                    "content": result.to_string(),
                    "role": "tool",
                    "tool_call_id": tool_call.id,
                    "tool_name": tool_call.name,
                }));
            }
        }

        ConversationResult {
            final_response: String::new(),
            messages,
            api_call_count,
            stopped_for_iteration_limit: true,
        }
    }

    pub fn into_provider(self) -> P {
        self.provider
    }
}

fn provider_tool_calls_json(tool_calls: &[ToolCall]) -> Value {
    Value::Array(
        tool_calls
            .iter()
            .map(|call| {
                json!({
                    "function": {
                        "arguments": call.arguments.to_string(),
                        "name": call.name,
                    },
                    "id": call.id,
                    "type": "function",
                })
            })
            .collect(),
    )
}

pub fn deduplicate_tool_calls(tool_calls: &[Value]) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();
    for call in tool_calls {
        let key = (
            call.get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            call.get("arguments")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        );
        if seen.insert(key) {
            unique.push(call.clone());
        }
    }
    if unique.len() < tool_calls.len() {
        unique
    } else {
        tool_calls.to_vec()
    }
}

pub fn cap_delegate_task_calls(tool_calls: &[Value], max_children: usize) -> Vec<Value> {
    let delegate_count = tool_calls
        .iter()
        .filter(|call| call.get("name").and_then(Value::as_str) == Some("delegate_task"))
        .count();
    if delegate_count <= max_children {
        return tool_calls.to_vec();
    }
    let mut kept_delegates = 0;
    let mut out = Vec::new();
    for call in tool_calls {
        if call.get("name").and_then(Value::as_str) == Some("delegate_task") {
            if kept_delegates < max_children {
                out.push(call.clone());
                kept_delegates += 1;
            }
        } else {
            out.push(call.clone());
        }
    }
    out
}

pub fn sanitize_tool_calls_for_strict_api(mut message: Value) -> Value {
    let Some(tool_calls) = message.get_mut("tool_calls").and_then(Value::as_array_mut) else {
        return message;
    };
    for tool_call in tool_calls {
        if let Some(object) = tool_call.as_object_mut() {
            object.remove("call_id");
            object.remove("response_item_id");
        }
    }
    message
}

pub fn iteration_budget_events() -> Vec<Value> {
    let budget = IterationBudget::new(3);
    let mut events = vec![json!({
        "op": "initial",
        "used": budget.used(),
        "remaining": budget.remaining(),
    })];
    for idx in 0..4 {
        let allowed = budget.consume();
        events.push(json!({
            "op": format!("consume_{}", idx + 1),
            "allowed": allowed,
            "used": budget.used(),
            "remaining": budget.remaining(),
        }));
    }
    for idx in 0..2 {
        budget.refund();
        events.push(json!({
            "op": format!("refund_{}", idx + 1),
            "used": budget.used(),
            "remaining": budget.remaining(),
        }));
    }
    let allowed = budget.consume();
    events.push(json!({
        "op": "consume_after_refund",
        "allowed": allowed,
        "used": budget.used(),
        "remaining": budget.remaining(),
    }));
    events
}

#[derive(Debug, Default)]
pub struct PendingSteer {
    pending: Mutex<Option<String>>,
}

impl PendingSteer {
    pub fn steer(&self, text: &str) -> bool {
        if text.trim().is_empty() {
            return false;
        }
        let cleaned = text.trim();
        let mut pending = self.pending.lock().unwrap();
        *pending = Some(match pending.as_ref() {
            Some(existing) => format!("{existing}\n{cleaned}"),
            None => cleaned.to_string(),
        });
        true
    }

    pub fn drain(&self) -> Option<String> {
        self.pending.lock().unwrap().take()
    }

    pub fn pending(&self) -> Option<String> {
        self.pending.lock().unwrap().clone()
    }
}

pub fn steer_state_fixture() -> Value {
    let steer = PendingSteer::default();
    let accepted_empty = steer.steer("  ");
    let accepted_first = steer.steer(" first ");
    let accepted_second = steer.steer("second");
    let pending_before_drain = steer.pending();
    let drained = steer.drain();
    let pending_after_drain = steer.pending();
    let drained_again = steer.drain();
    json!({
        "accepted_empty": accepted_empty,
        "accepted_first": accepted_first,
        "accepted_second": accepted_second,
        "pending_before_drain": pending_before_drain,
        "drained": drained,
        "pending_after_drain": pending_after_drain,
        "drained_again": drained_again,
    })
}

pub fn interrupt_state_fixture() -> Value {
    let mut interrupted_threads = BTreeSet::new();
    let execution_thread_id = 111;
    let worker_threads = [222, 333];
    let interrupt_message = "x".repeat(45);

    interrupted_threads.insert(execution_thread_id);
    for worker in worker_threads {
        interrupted_threads.insert(worker);
    }
    let after_request = json!({
        "requested": true,
        "message": interrupt_message,
        "thread_signal_pending": false,
        "interrupted_threads": interrupted_threads.iter().copied().collect::<Vec<_>>(),
    });

    interrupted_threads.clear();
    let after_clear = json!({
        "requested": false,
        "message": null,
        "thread_signal_pending": false,
        "pending_steer": null,
        "interrupted_threads": interrupted_threads.iter().copied().collect::<Vec<i32>>(),
    });

    json!({
        "after_request": after_request,
        "after_clear": after_clear,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hermes_provider::{FakeProvider, ProviderResponse, ToolCall};
    use std::sync::Arc;

    #[test]
    fn runs_tool_loop_until_final_response() {
        let provider = FakeProvider::new([
            ProviderResponse {
                content: None,
                tool_calls: vec![ToolCall {
                    id: "call-1".to_string(),
                    name: "memory".to_string(),
                    arguments: json!({"action": "add"}),
                }],
            },
            ProviderResponse {
                content: Some("done".to_string()),
                tool_calls: Vec::new(),
            },
        ]);
        let mut runtime = AgentRuntime::new(provider, 4);
        runtime.register_tool("memory", |args| json!({"ok": true, "args": args}));

        let result = runtime.run_conversation(vec![json!({
            "content": "hello",
            "role": "user",
        })]);

        assert_eq!(result.final_response, "done");
        assert_eq!(result.api_call_count, 2);
        assert!(!result.stopped_for_iteration_limit);
        assert_eq!(result.messages[2]["role"], "tool");
        assert_eq!(result.messages[2]["tool_name"], "memory");
    }

    #[test]
    fn stops_at_iteration_limit() {
        let provider = FakeProvider::new([ProviderResponse {
            content: None,
            tool_calls: vec![ToolCall {
                id: "call-1".to_string(),
                name: "unknown".to_string(),
                arguments: json!({}),
            }],
        }]);
        let mut runtime = AgentRuntime::new(provider, 1);
        let result = runtime.run_conversation(vec![]);
        assert!(result.stopped_for_iteration_limit);
        assert_eq!(result.api_call_count, 1);
    }

    #[test]
    fn handles_repeated_tool_call_rounds_before_final_response() {
        let provider = FakeProvider::new([
            ProviderResponse {
                content: None,
                tool_calls: vec![ToolCall {
                    id: "call-1".to_string(),
                    name: "memory".to_string(),
                    arguments: json!({"action": "add", "content": "first"}),
                }],
            },
            ProviderResponse {
                content: None,
                tool_calls: vec![ToolCall {
                    id: "call-2".to_string(),
                    name: "memory".to_string(),
                    arguments: json!({"action": "add", "content": "second"}),
                }],
            },
            ProviderResponse {
                content: Some("done".to_string()),
                tool_calls: Vec::new(),
            },
        ]);
        let mut runtime = AgentRuntime::new(provider, 4);
        runtime.register_tool("memory", |args| json!({"stored": args["content"]}));

        let result = runtime.run_conversation(vec![json!({
            "content": "remember two facts",
            "role": "user",
        })]);

        assert_eq!(result.final_response, "done");
        assert_eq!(result.api_call_count, 3);
        let tool_messages = result
            .messages
            .iter()
            .filter(|message| message["role"] == "tool")
            .collect::<Vec<_>>();
        assert_eq!(tool_messages.len(), 2);
        assert_eq!(tool_messages[0]["tool_call_id"], "call-1");
        assert_eq!(tool_messages[1]["tool_call_id"], "call-2");
    }

    #[test]
    fn iteration_budget_is_thread_safe() {
        let budget = Arc::new(IterationBudget::new(50));
        let mut handles = Vec::new();
        for _ in 0..10 {
            let budget = Arc::clone(&budget);
            handles.push(std::thread::spawn(move || {
                for _ in 0..10 {
                    let _ = budget.consume();
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        assert_eq!(budget.used(), 50);
        assert_eq!(budget.remaining(), 0);
        assert!(!budget.consume());
    }
}
