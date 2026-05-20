from __future__ import annotations

import copy
import threading
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
        from agent.iteration_budget import IterationBudget
        from tools import interrupt as interrupt_mod

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

        budget = IterationBudget(max_total=3)
        budget_events = [
            {
                "op": "initial",
                "used": budget.used,
                "remaining": budget.remaining,
            }
        ]
        for idx in range(4):
            budget_events.append(
                {
                    "op": f"consume_{idx + 1}",
                    "allowed": budget.consume(),
                    "used": budget.used,
                    "remaining": budget.remaining,
                }
            )
        for idx in range(2):
            budget.refund()
            budget_events.append(
                {
                    "op": f"refund_{idx + 1}",
                    "used": budget.used,
                    "remaining": budget.remaining,
                }
            )
        budget_events.append(
            {
                "op": "consume_after_refund",
                "allowed": budget.consume(),
                "used": budget.used,
                "remaining": budget.remaining,
            }
        )

        steer_agent = AIAgent.__new__(AIAgent)
        steer_agent._pending_steer = None
        steer_agent._pending_steer_lock = threading.Lock()
        steer_case = {
            "accepted_empty": steer_agent.steer("  "),
            "accepted_first": steer_agent.steer(" first "),
            "accepted_second": steer_agent.steer("second"),
            "pending_before_drain": steer_agent._pending_steer,
            "drained": steer_agent._drain_pending_steer(),
            "pending_after_drain": steer_agent._pending_steer,
            "drained_again": steer_agent._drain_pending_steer(),
        }

        interrupt_mod._interrupted_threads.clear()
        interrupt_agent = AIAgent.__new__(AIAgent)
        interrupt_agent._execution_thread_id = 111
        interrupt_agent._tool_worker_threads = {222, 333}
        interrupt_agent._tool_worker_threads_lock = threading.Lock()
        interrupt_agent._active_children = []
        interrupt_agent._active_children_lock = threading.Lock()
        interrupt_agent._pending_steer = "late steer"
        interrupt_agent._pending_steer_lock = threading.Lock()
        interrupt_agent.quiet_mode = True
        interrupt_agent.interrupt("x" * 45)
        interrupt_after_request = {
            "requested": interrupt_agent._interrupt_requested,
            "message": interrupt_agent._interrupt_message,
            "thread_signal_pending": interrupt_agent._interrupt_thread_signal_pending,
            "interrupted_threads": sorted(interrupt_mod._interrupted_threads),
        }
        interrupt_agent.clear_interrupt()
        interrupt_after_clear = {
            "requested": interrupt_agent._interrupt_requested,
            "message": interrupt_agent._interrupt_message,
            "thread_signal_pending": interrupt_agent._interrupt_thread_signal_pending,
            "pending_steer": interrupt_agent._pending_steer,
            "interrupted_threads": sorted(interrupt_mod._interrupted_threads),
        }

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
        {
            "name": "iteration_budget",
            "events": budget_events,
        },
        {
            "name": "steer_state",
            "state": steer_case,
        },
        {
            "name": "interrupt_state",
            "after_request": interrupt_after_request,
            "after_clear": interrupt_after_clear,
        },
    ]
    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
