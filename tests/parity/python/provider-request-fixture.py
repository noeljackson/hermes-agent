from __future__ import annotations

from types import SimpleNamespace

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "provider-request-fixture.py"


def max_tokens_param(value):
    return {"max_tokens": value}


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home():
        from agent.transports.chat_completions import ChatCompletionsTransport
        from agent.transports.anthropic import AnthropicTransport
        from agent.transports.codex import ResponsesApiTransport
        from agent.transports.types import (
            NormalizedResponse,
            Usage,
            build_tool_call,
            map_finish_reason,
        )
        from agent.transports.codex_event_projector import CodexEventProjector
        from agent.codex_responses_adapter import (
            _chat_messages_to_responses_input,
            _derive_responses_function_call_id,
            _deterministic_call_id,
            _normalize_codex_response,
            _preflight_codex_api_kwargs,
            _preflight_codex_input_items,
            _split_responses_tool_id,
        )
        import agent.stream_diag as stream_diag
        from agent.error_classifier import classify_api_error
        from providers.base import ProviderProfile
        from run_agent import AIAgent

        chat_transport = ChatCompletionsTransport()
        profile = ProviderProfile(
            name="fake",
            display_name="Fake Provider",
            base_url="https://provider.invalid/v1",
            env_vars=("FAKE_API_KEY",),
            fixed_temperature=0.2,
            default_max_tokens=123,
        )
        messages = [
            {"role": "system", "content": "system"},
            {"role": "user", "content": "hello"},
        ]
        tools = [
            {
                "type": "function",
                "function": {
                    "name": "memory",
                    "description": "Remember facts.",
                    "parameters": {
                        "type": "object",
                        "properties": {"action": {"type": "string"}},
                        "required": ["action"],
                    },
                },
            }
        ]
        kwargs = chat_transport.build_kwargs(
            "fake/model",
            messages,
            tools,
            provider_profile=profile,
            timeout=30,
            max_tokens_param_fn=max_tokens_param,
            reasoning_config={"enabled": True, "effort": "medium"},
            session_id="session-1",
        )

        leaked_messages = [
            {
                "role": "assistant",
                "content": "ok",
                "codex_reasoning_items": [{"id": "rs_1"}],
                "codex_message_items": [{"id": "msg_1", "type": "message"}],
                "tool_calls": [
                    {
                        "id": "call_1",
                        "call_id": "call_1",
                        "response_item_id": "fc_1",
                        "type": "function",
                        "function": {"name": "terminal", "arguments": "{}"},
                    }
                ],
            }
        ]
        sanitized_chat_kwargs = chat_transport.build_kwargs(
            "gpt-4o",
            leaked_messages,
            timeout=10,
        )

        codex_transport = ResponsesApiTransport()
        codex_kwargs = codex_transport.build_kwargs(
            "gpt-5.4",
            messages,
            tools,
            session_id="session-1",
            max_tokens=4096,
            reasoning_config={"enabled": True, "effort": "minimal"},
        )
        codex_xai_kwargs = codex_transport.build_kwargs(
            "grok-3-mini",
            [{"role": "user", "content": "hello"}],
            [],
            session_id="conv-xai-1",
            max_tokens=2048,
            reasoning_config={"enabled": True, "effort": "high"},
            is_xai_responses=True,
            request_overrides={
                "extra_headers": {"X-Test": "1"},
                "extra_body": {"caller": True},
            },
        )

        anthropic_transport = AnthropicTransport()
        anthropic_kwargs = anthropic_transport.build_kwargs(
            "claude-sonnet-4-6",
            messages,
            tools,
            max_tokens=1024,
            reasoning_config={"enabled": True, "effort": "high"},
            tool_choice="required",
        )
        service_tier_kwargs = chat_transport.build_kwargs(
            "fake/model",
            messages,
            [],
            provider_profile=profile,
            timeout=5,
            extra_body_additions={"trace": "yes"},
            request_overrides={
                "service_tier": "priority",
                "extra_body": {"provider": {"allow_fallbacks": False}},
            },
        )
        tool_call = build_tool_call(
            id="call_3",
            name="terminal",
            arguments={"cmd": "ls"},
            call_id="call_3",
            response_item_id="fc_3",
            extra_content={"google": {"thought_signature": "SIG_ABC123"}},
        )
        normalized_response = NormalizedResponse(
            content="answer",
            tool_calls=[tool_call],
            finish_reason="tool_calls",
            reasoning="I thought about it",
            usage=Usage(),
            provider_data={
                "reasoning_content": "hidden chain",
                "reasoning_details": [{"type": "thinking", "thinking": "hmm"}],
                "codex_reasoning_items": [{"id": "rs_1"}],
                "codex_message_items": [{"id": "msg_1", "type": "message"}],
            },
        )
        finish_mapping = {
            "end_turn": "stop",
            "tool_use": "tool_calls",
            "max_tokens": "length",
            "stop_sequence": "stop",
            "refusal": "content_filter",
        }
        responses_api_routing = {
            "gpt5_plain": AIAgent._model_requires_responses_api("gpt-5.4"),
            "gpt5_vendor_prefixed": AIAgent._model_requires_responses_api(
                "openai/gpt-5.4"
            ),
            "gpt4o_plain": AIAgent._model_requires_responses_api("gpt-4o"),
            "nous_gpt5": AIAgent._provider_model_requires_responses_api(
                "gpt-5.4", provider="nous"
            ),
            "openrouter_gpt5": AIAgent._provider_model_requires_responses_api(
                "openai/gpt-5.4", provider="openrouter"
            ),
            "blank_provider_gpt5": AIAgent._provider_model_requires_responses_api(
                "gpt-5.4", provider=""
            ),
            "copilot_gpt4o": AIAgent._provider_model_requires_responses_api(
                "gpt-4o", provider="copilot"
            ),
        }
        max_tokens_routing = {}
        for name, base_url in {
            "direct_openai": "https://api.openai.com/v1",
            "azure_openai": "https://example-resource.openai.azure.com/openai/v1",
            "github_copilot": "https://api.githubcopilot.com",
            "openrouter": "https://openrouter.ai/api/v1",
            "local": "http://localhost:11434/v1",
        }.items():
            agent = AIAgent.__new__(AIAgent)
            agent._base_url_lower = base_url.lower()
            agent._base_url_hostname = ""
            max_tokens_routing[name] = AIAgent._max_tokens_param(agent, 321)

        codex_projector_notifications = [
            {"method": "item/agentMessage/outputDelta", "params": {"delta": "ignored"}},
            {
                "method": "item/completed",
                "params": {
                    "item": {
                        "type": "reasoning",
                        "id": "reason-1",
                        "summary": ["summary one"],
                        "content": ["detail two"],
                    }
                },
            },
            {
                "method": "item/completed",
                "params": {
                    "item": {
                        "type": "agentMessage",
                        "id": "agent-1",
                        "text": "final answer",
                    }
                },
            },
            {
                "method": "item/completed",
                "params": {
                    "item": {
                        "type": "userMessage",
                        "id": "user-1",
                        "content": [
                            {"type": "text", "text": "hello"},
                            {"type": "image", "url": "ignored"},
                            {"text": "fallback text"},
                        ],
                    }
                },
            },
            {
                "method": "item/completed",
                "params": {
                    "item": {
                        "type": "commandExecution",
                        "id": "cmd-1",
                        "command": "ls -la",
                        "cwd": "/workspace",
                        "aggregatedOutput": "ok\n",
                        "exitCode": 0,
                    }
                },
            },
            {
                "method": "item/completed",
                "params": {
                    "item": {
                        "type": "commandExecution",
                        "id": "cmd-2",
                        "command": "false",
                        "cwd": "",
                        "aggregatedOutput": "failed",
                        "exitCode": 2,
                    }
                },
            },
            {
                "method": "item/completed",
                "params": {
                    "item": {
                        "type": "fileChange",
                        "id": "patch-1",
                        "status": "accepted",
                        "changes": [
                            {"kind": {"type": "add"}, "path": "src/new.rs"},
                            {"kind": {"type": "delete"}, "path": "old.py"},
                            {"path": "updated.md"},
                        ],
                    }
                },
            },
            {
                "method": "item/completed",
                "params": {
                    "item": {
                        "type": "mcpToolCall",
                        "id": "mcp-1",
                        "server": "github",
                        "tool": "search",
                        "arguments": {"query": "hermes", "limit": 2},
                        "result": {"items": [{"name": "repo"}]},
                    }
                },
            },
            {
                "method": "item/completed",
                "params": {
                    "item": {
                        "type": "mcpToolCall",
                        "id": "mcp-2",
                        "server": "github",
                        "tool": "create_issue",
                        "arguments": ["not", "a", "dict"],
                        "error": {"message": "denied"},
                    }
                },
            },
            {
                "method": "item/completed",
                "params": {
                    "item": {
                        "type": "dynamicToolCall",
                        "id": "dyn-1",
                        "tool": "browser_navigate",
                        "arguments": {"url": "https://example.invalid"},
                        "contentItems": [{"text": "loaded", "type": "text"}],
                    }
                },
            },
            {
                "method": "item/completed",
                "params": {
                    "item": {
                        "type": "dynamicToolCall",
                        "id": "dyn-2",
                        "tool": "custom_tool",
                        "arguments": "raw-args",
                        "success": False,
                    }
                },
            },
            {
                "method": "item/completed",
                "params": {
                    "item": {
                        "id": "plan-1",
                        "steps": ["inspect", "edit"],
                        "type": "plan",
                    }
                },
            },
        ]
        codex_projector = CodexEventProjector()
        codex_projection = []
        for notification in codex_projector_notifications:
            result = codex_projector.project(notification)
            codex_projection.append(
                {
                    "messages": result.messages,
                    "is_tool_iteration": result.is_tool_iteration,
                    "final_text": result.final_text,
                }
            )

        class FakeHeadersResponse:
            status_code = 529
            headers = {
                "cf-ray": "cf123",
                "x-openrouter-provider": "openrouter-provider-a",
                "x-request-id": "r" * 150,
                "authorization": "must-not-be-captured",
            }

        class FakeStreamAgent:
            provider = "openrouter"
            base_url = "https://openrouter.ai/api/v1"
            _subagent_id = "sub-1"
            _delegate_depth = 2

            def __init__(self):
                self.status_events = []
                self.activity_events = []

            def _summarize_api_error(self, error):
                return f"summary: {type(error).__name__}"

            def _emit_status(self, value):
                self.status_events.append(value)

            def _touch_activity(self, value):
                self.activity_events.append(value)

        chained = RuntimeError("outer\nline")
        chained.__cause__ = ValueError("inner cause")
        empty_message = RuntimeError()
        long_message = RuntimeError("x" * 145)
        captured_diag = {
            "started_at": 1000.0,
            "first_chunk_at": None,
            "chunks": 0,
            "bytes": 0,
            "headers": {},
            "http_status": None,
        }
        stream_diag.stream_diag_capture_response(
            FakeStreamAgent(), captured_diag, FakeHeadersResponse()
        )

        fake_time = stream_diag.time.time
        fake_agent = FakeStreamAgent()
        try:
            stream_diag.time.time = lambda: 1005.25
            stream_diag.emit_stream_drop(
                fake_agent,
                error=RuntimeError("socket closed"),
                attempt=2,
                max_attempts=4,
                mid_tool_call=True,
                diag={
                    "started_at": 1000.0,
                    "first_chunk_at": 1001.0,
                    "chunks": 3,
                    "bytes": 42,
                    "headers": {"cf-ray": "abc"},
                    "http_status": 502,
                },
            )
        finally:
            stream_diag.time.time = fake_time

        stream_diagnostics = {
            "captured_response": captured_diag,
            "drop_emit": {
                "status_events": fake_agent.status_events,
                "activity_events": fake_agent.activity_events,
            },
            "flatten_exception_chain": {
                "chained": stream_diag.flatten_exception_chain(chained),
                "empty": stream_diag.flatten_exception_chain(empty_message),
                "truncated": stream_diag.flatten_exception_chain(long_message),
            },
        }

        class FakeAPIError(Exception):
            def __init__(self, message, status_code=None, body=None):
                super().__init__(message)
                self.status_code = status_code
                self.body = body

        class RateLimitError(Exception):
            pass

        class RemoteProtocolError(Exception):
            pass

        def classified_case(
            name,
            error,
            provider="openrouter",
            model="openai/gpt-5.4",
            approx_tokens=0,
            context_length=200000,
            num_messages=0,
        ):
            result = classify_api_error(
                error,
                provider=provider,
                model=model,
                approx_tokens=approx_tokens,
                context_length=context_length,
                num_messages=num_messages,
            )
            return {
                "name": name,
                "input": {
                    "error_type": type(error).__name__,
                    "message": str(error),
                    "status_code": getattr(error, "status_code", None),
                    "body": getattr(error, "body", None),
                    "provider": provider,
                    "model": model,
                    "approx_tokens": approx_tokens,
                    "context_length": context_length,
                    "num_messages": num_messages,
                },
                "result": {
                    "reason": result.reason.value,
                    "status_code": result.status_code,
                    "message": result.message,
                    "retryable": result.retryable,
                    "should_compress": result.should_compress,
                    "should_rotate_credential": result.should_rotate_credential,
                    "should_fallback": result.should_fallback,
                    "is_auth": result.is_auth,
                },
            }

        error_classification = [
            classified_case(
                "http_401_auth",
                FakeAPIError(
                    "unauthorized",
                    401,
                    {"error": {"message": "bad key"}},
                ),
            ),
            classified_case(
                "http_403_key_limit_billing",
                FakeAPIError("key limit exceeded", 403),
            ),
            classified_case(
                "http_402_transient_usage_limit",
                FakeAPIError(
                    "payment required",
                    402,
                    {"error": {"message": "Usage limit, try again in 5 minutes"}},
                ),
            ),
            classified_case(
                "http_402_billing",
                FakeAPIError("Insufficient credits", 402),
            ),
            classified_case(
                "http_404_model_not_found",
                FakeAPIError("model not found", 404),
            ),
            classified_case(
                "http_404_policy_blocked",
                FakeAPIError(
                    "No endpoints available matching your data policy. Configure: https://openrouter.ai/settings/privacy",
                    404,
                ),
            ),
            classified_case(
                "http_413_payload_too_large",
                FakeAPIError("payload too large", 413),
            ),
            classified_case(
                "http_429_long_context_tier",
                FakeAPIError("extra usage requires long context tier", 429),
            ),
            classified_case(
                "http_400_thinking_signature",
                FakeAPIError("thinking block signature is invalid", 400),
            ),
            classified_case(
                "http_400_llama_cpp_grammar",
                FakeAPIError("json-schema-to-grammar error parsing grammar", 400),
            ),
            classified_case(
                "rate_limit_type_without_status",
                RateLimitError("limit reached"),
            ),
            classified_case(
                "large_remote_disconnect_context_overflow",
                RemoteProtocolError("server disconnected without sending response"),
                approx_tokens=130000,
                context_length=200000,
                num_messages=210,
            ),
            classified_case(
                "small_remote_disconnect_timeout",
                RemoteProtocolError("server disconnected without sending response"),
                approx_tokens=1000,
                context_length=200000,
                num_messages=3,
            ),
            classified_case(
                "ssl_alert_timeout_not_compression",
                RuntimeError("[SSL: BAD_RECORD_MAC] bad record mac"),
                approx_tokens=180000,
                context_length=200000,
                num_messages=300,
            ),
            classified_case(
                "grok_subscription_entitlement",
                RuntimeError(
                    "You have either run out of available resources or do not have an active Grok subscription"
                ),
                provider="xai",
                model="grok-4",
            ),
        ]

        codex_id_helpers = {
            "deterministic": {
                "terminal_zero": _deterministic_call_id("terminal", '{"cmd":"ls"}', 0),
                "terminal_one": _deterministic_call_id("terminal", '{"cmd":"ls"}', 1),
                "unicode": _deterministic_call_id("unicode", '{"text":"olá"}', 2),
            },
            "split": {
                "pipe": list(_split_responses_tool_id(" call_abc | fc_def ")),
                "fc_only": list(_split_responses_tool_id("fc_response_item")),
                "call_only": list(_split_responses_tool_id("call_plain")),
                "empty": list(_split_responses_tool_id("  ")),
                "nonstr": list(_split_responses_tool_id(42)),
            },
            "derive": {
                "response_item_wins": _derive_responses_function_call_id(
                    "call_abc", " fc_existing "
                ),
                "call_prefix": _derive_responses_function_call_id("call_abc", None),
                "already_fc": _derive_responses_function_call_id("fc_raw", None),
                "sanitized": _derive_responses_function_call_id("weird id/!*", None),
                "response_seed": _derive_responses_function_call_id("", "notfc"),
            },
        }

        codex_messages = [
            {"role": "system", "content": "ignored system"},
            {
                "role": "user",
                "content": [
                    "lead",
                    {"type": "text", "text": "hello"},
                    {
                        "type": "image_url",
                        "image_url": {"url": "https://example.invalid/a.png", "detail": "low"},
                    },
                    {"type": "unknown", "text": "ignored"},
                ],
            },
            {
                "role": "assistant",
                "content": "",
                "codex_reasoning_items": [
                    {
                        "id": "rs_1",
                        "type": "reasoning",
                        "encrypted_content": "enc-1",
                        "summary": [{"type": "summary_text", "text": "sum"}],
                    },
                    {
                        "id": "rs_1",
                        "type": "reasoning",
                        "encrypted_content": "enc-duplicate",
                    },
                ],
                "codex_message_items": [
                    {
                        "id": "msg_1",
                        "type": "message",
                        "role": "assistant",
                        "status": "in progress",
                        "phase": "final_answer",
                        "content": [
                            {"type": "text", "text": "prior"},
                            {"type": "ignored", "text": "drop"},
                        ],
                    }
                ],
                "tool_calls": [
                    {
                        "id": " call_embedded | fc_item ",
                        "type": "function",
                        "function": {"name": "terminal", "arguments": {"cmd": "pwd"}},
                    },
                    {
                        "id": "fc_only_item",
                        "type": "function",
                        "function": {"name": "read_file", "arguments": " {\"path\":\"x\"} "},
                    },
                    {
                        "type": "function",
                        "function": {"name": "missing_id", "arguments": {"a": 1}},
                    },
                ],
            },
            {
                "role": "tool",
                "tool_call_id": " call_embedded | fc_item ",
                "content": [
                    {"type": "text", "text": "tool output"},
                    {
                        "type": "image_url",
                        "image_url": "https://example.invalid/tool.png",
                        "detail": "high",
                    },
                    {"type": "unknown", "text": "drop"},
                ],
            },
            {"role": "tool", "tool_call_id": "", "content": "ignored"},
        ]
        codex_input_conversion = {
            "standard": _chat_messages_to_responses_input(codex_messages),
            "xai": _chat_messages_to_responses_input(
                codex_messages, is_xai_responses=True
            ),
        }

        preflight_items_input = [
            {"type": "function_call", "call_id": " call_1 ", "name": " terminal ", "arguments": {"cmd": "ls"}},
            {
                "type": "function_call_output",
                "call_id": " call_1 ",
                "output": [
                    {"type": "input_text", "text": "ok"},
                    {"type": "input_image", "image_url": "https://example.invalid/i.png", "detail": " low "},
                    {"type": "input_image", "image_url": ""},
                    {"type": "bad", "text": "drop"},
                ],
            },
            {"type": "reasoning", "id": "rs_a", "encrypted_content": "enc-a", "summary": ["raw"]},
            {"type": "reasoning", "id": "rs_a", "encrypted_content": "enc-b"},
            {
                "type": "message",
                "role": "assistant",
                "status": "IN-PROGRESS",
                "id": " msg_keep ",
                "phase": " commentary ",
                "content": [{"type": "text", "text": 123}],
            },
            {
                "role": "assistant",
                "content": [
                    "inline",
                    {"type": "input_text", "text": "assistant text"},
                    {"type": "input_image", "image_url": "https://example.invalid/ignored.png"},
                ],
            },
            {
                "role": "user",
                "content": [
                    {"type": "output_text", "text": "coerced"},
                    {"type": "image_url", "image_url": {"url": "https://example.invalid/user.png", "detail": "auto"}},
                ],
            },
        ]

        def preflight_error(raw_items):
            try:
                _preflight_codex_input_items(raw_items)
            except Exception as exc:
                return {"type": type(exc).__name__, "message": str(exc)}
            return {"type": None, "message": None}

        codex_preflight = {
            "items": _preflight_codex_input_items(preflight_items_input),
            "errors": {
                "not_list": preflight_error({"bad": True}),
                "bad_tool_output": preflight_error([
                    {"type": "function_call_output", "output": "missing call"},
                ]),
                "bad_message_role": preflight_error([
                    {"type": "message", "role": "user", "content": []},
                ]),
                "bad_content_part": preflight_error([
                    {"role": "user", "content": [{"type": "unsupported"}]},
                ]),
            },
            "api_kwargs": _preflight_codex_api_kwargs(
                {
                    "model": " gpt-5.4 ",
                    "instructions": " ",
                    "input": preflight_items_input[:2],
                    "tools": [
                        {
                            "type": "function",
                            "name": " terminal ",
                            "description": 123,
                            "strict": "yes",
                            "parameters": {"type": "object"},
                        }
                    ],
                    "store": False,
                    "include": ["reasoning.encrypted_content"],
                    "reasoning": {"effort": "medium"},
                    "max_output_tokens": 99.9,
                    "temperature": 0,
                    "tool_choice": "auto",
                    "parallel_tool_calls": True,
                    "prompt_cache_key": "session-x",
                    "service_tier": " priority ",
                    "extra_headers": {" X-Test ": 7, "Skip": None},
                    "extra_body": {"prompt_cache_key": "session-x"},
                }
            ),
        }

        response = SimpleNamespace(
            status="incomplete",
            output=[
                SimpleNamespace(
                    type="reasoning",
                    id="rs_resp",
                    encrypted_content="enc-resp",
                    summary=[SimpleNamespace(text="reason one")],
                    status="completed",
                ),
                SimpleNamespace(
                    type="message",
                    id="msg_resp",
                    status="completed",
                    phase="commentary",
                    content=[SimpleNamespace(type="output_text", text="thinking aloud")],
                ),
                SimpleNamespace(
                    type="function_call",
                    id="fc_tool",
                    call_id="",
                    name="terminal",
                    arguments={"cmd": "ls"},
                    status="completed",
                ),
                SimpleNamespace(
                    type="custom_tool_call",
                    id="custom|fc_custom",
                    call_id=None,
                    name="custom_tool",
                    input=["raw"],
                    status="completed",
                ),
            ],
        )
        normalized_message, normalized_finish_reason = _normalize_codex_response(response)
        codex_response_normalization = {
            "finish_reason": normalized_finish_reason,
            "message": {
                "content": normalized_message.content,
                "reasoning": normalized_message.reasoning,
                "codex_reasoning_items": normalized_message.codex_reasoning_items,
                "codex_message_items": normalized_message.codex_message_items,
                "tool_calls": [
                    {
                        "id": tool_call.id,
                        "call_id": tool_call.call_id,
                        "response_item_id": tool_call.response_item_id,
                        "type": tool_call.type,
                        "function": {
                            "name": tool_call.function.name,
                            "arguments": tool_call.function.arguments,
                        },
                    }
                    for tool_call in normalized_message.tool_calls
                ],
            },
        }

    cases = [
        {"name": "chat_completions_fake_provider", "request": kwargs},
        {"name": "chat_completions_strips_codex_leaks", "request": sanitized_chat_kwargs},
        {"name": "codex_responses_standard", "request": codex_kwargs},
        {"name": "codex_responses_xai_cache_routing", "request": codex_xai_kwargs},
        {"name": "anthropic_messages_standard", "request": anthropic_kwargs},
        {"name": "chat_completions_service_tier_override", "request": service_tier_kwargs},
        {
            "name": "normalized_transport_types",
            "tool_call": {
                "id": tool_call.id,
                "name": tool_call.name,
                "arguments": tool_call.arguments,
                "provider_data": tool_call.provider_data,
                "type": tool_call.type,
                "function_is_self": tool_call.function is tool_call,
                "function_name": tool_call.function.name,
                "function_arguments": tool_call.function.arguments,
                "call_id": tool_call.call_id,
                "response_item_id": tool_call.response_item_id,
                "extra_content": tool_call.extra_content,
            },
            "usage_defaults": {
                "prompt_tokens": normalized_response.usage.prompt_tokens,
                "completion_tokens": normalized_response.usage.completion_tokens,
                "total_tokens": normalized_response.usage.total_tokens,
                "cached_tokens": normalized_response.usage.cached_tokens,
            },
            "response": {
                "content": normalized_response.content,
                "finish_reason": normalized_response.finish_reason,
                "reasoning": normalized_response.reasoning,
                "reasoning_content": normalized_response.reasoning_content,
                "reasoning_details": normalized_response.reasoning_details,
                "codex_reasoning_items": normalized_response.codex_reasoning_items,
                "codex_message_items": normalized_response.codex_message_items,
            },
            "finish_reason_map": {
                "known": map_finish_reason("tool_use", finish_mapping),
                "unknown": map_finish_reason("something_new", finish_mapping),
                "none": map_finish_reason(None, finish_mapping),
            },
        },
        {"name": "responses_api_routing", "cases": responses_api_routing},
        {"name": "max_tokens_param_routing", "cases": max_tokens_routing},
        {
            "name": "codex_event_projection",
            "notifications": codex_projector_notifications,
            "results": codex_projection,
        },
        {"name": "stream_diagnostics", "cases": stream_diagnostics},
        {"name": "error_classification", "cases": error_classification},
        {"name": "codex_responses_id_helpers", "cases": codex_id_helpers},
        {"name": "codex_responses_input_conversion", "cases": codex_input_conversion},
        {"name": "codex_responses_preflight", "cases": codex_preflight},
        {"name": "codex_response_normalization", "cases": codex_response_normalization},
    ]
    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
