from __future__ import annotations

import copy
from types import SimpleNamespace

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "agent-loop-fixture.py"


def tool_call(call_id: str, name: str, arguments: str):
    return SimpleNamespace(
        id=call_id,
        function=SimpleNamespace(name=name, arguments=arguments),
    )


def compact_tool_calls(calls):
    return [
        {
            "id": call.id,
            "name": call.function.name,
            "arguments": call.function.arguments,
        }
        for call in calls
    ]


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home():
        from run_agent import AIAgent

        duplicate_calls = [
            tool_call("call-1", "terminal", '{"cmd":"pwd"}'),
            tool_call("call-2", "terminal", '{"cmd":"pwd"}'),
            tool_call("call-3", "terminal", '{"cmd":"ls"}'),
            tool_call("call-4", "memory", '{"action":"add"}'),
        ]
        deduplicated = AIAgent._deduplicate_tool_calls(duplicate_calls)

        delegate_calls = [
            tool_call("delegate-1", "delegate_task", '{"prompt":"one"}'),
            tool_call("delegate-2", "delegate_task", '{"prompt":"two"}'),
            tool_call("terminal-1", "terminal", '{"cmd":"pwd"}'),
            tool_call("delegate-3", "delegate_task", '{"prompt":"three"}'),
            tool_call("delegate-4", "delegate_task", '{"prompt":"four"}'),
        ]
        capped_delegates = AIAgent._cap_delegate_task_calls(delegate_calls)

        strict_msg = {
            "role": "assistant",
            "content": "ok",
            "tool_calls": [
                {
                    "id": "call-1",
                    "call_id": "call-1",
                    "response_item_id": "fc-1",
                    "type": "function",
                    "function": {"name": "terminal", "arguments": "{}"},
                },
                "non-dict-tool-call",
            ],
        }
        strict_copy = copy.deepcopy(strict_msg)
        sanitized = AIAgent._sanitize_tool_calls_for_strict_api(strict_copy)

    cases = [
        {
            "name": "deduplicate_tool_calls",
            "tool_calls": compact_tool_calls(deduplicated),
        },
        {
            "name": "cap_delegate_task_calls",
            "tool_calls": compact_tool_calls(capped_delegates),
        },
        {
            "name": "strict_api_tool_call_sanitization",
            "message": sanitized,
            "original": strict_msg,
        },
    ]
    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
