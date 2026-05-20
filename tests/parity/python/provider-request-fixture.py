from __future__ import annotations

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
        from providers.base import ProviderProfile

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

    cases = [
        {"name": "chat_completions_fake_provider", "request": kwargs},
        {"name": "chat_completions_strips_codex_leaks", "request": sanitized_chat_kwargs},
        {"name": "codex_responses_standard", "request": codex_kwargs},
        {"name": "codex_responses_xai_cache_routing", "request": codex_xai_kwargs},
        {"name": "anthropic_messages_standard", "request": anthropic_kwargs},
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
    ]
    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
