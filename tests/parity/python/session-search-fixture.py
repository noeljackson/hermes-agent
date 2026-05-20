#!/usr/bin/env python3
from __future__ import annotations

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "session-search-fixture.py"


def _compact_result(row: dict) -> dict:
    return {
        "session_id": row.get("session_id"),
        "role": row.get("role"),
        "source": row.get("source"),
        "snippet": row.get("snippet"),
        "context": row.get("context", []),
    }


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home() as home:
        from hermes_state import SessionDB

        db = SessionDB(home / "state.db")
        db.create_session(session_id="s-cli", source="cli", model="fake/model")
        db.append_message("s-cli", role="user", content="How do I deploy with Docker?")
        db.append_message("s-cli", role="assistant", content="Use docker compose up.")
        db.append_message("s-cli", role="user", content="Run the chat-send command")

        db.create_session(session_id="s-telegram", source="telegram", model="fake/model")
        db.append_message("s-telegram", role="user", content="Telegram question about Python")

        db.create_session(session_id="s-api", source="cli", model="fake/model")
        db.append_message("s-api", role="user", content="What is FastAPI?")
        db.append_message("s-api", role="assistant", content="FastAPI is a web framework.")

        db.create_session(session_id="s-tool", source="cli", model="fake/model")
        db.append_message(
            "s-tool",
            role="assistant",
            content="",
            tool_calls=[
                {
                    "id": "call-tool",
                    "type": "function",
                    "function": {
                        "name": "terminal",
                        "arguments": "{\"cmd\":\"echo unique_tool_token\"}",
                    },
                }
            ],
            finish_reason="tool_calls",
        )
        db.append_message(
            "s-tool",
            role="tool",
            content="{}",
            tool_call_id="call-tool",
            tool_name="terminal",
        )

        sanitize_cases = {
            query: SessionDB._sanitize_fts5_query(query)
            for query in [
                "hello world",
                "C++",
                '"unterminated',
                "(problem",
                "hello AND",
                "OR world",
                "***",
                "deploy*",
                "chat-send",
                "simulate.p2.test.ts",
                '"docker networking"',
            ]
        }
        searches = {
            "empty": [_compact_result(r) for r in db.search_messages("")],
            "docker": [_compact_result(r) for r in db.search_messages("docker", limit=5)],
            "telegram_python": [
                _compact_result(r)
                for r in db.search_messages("Python", source_filter=["telegram"], limit=5)
            ],
            "assistant_fastapi": [
                _compact_result(r)
                for r in db.search_messages("FastAPI", role_filter=["assistant"], limit=5)
            ],
            "hyphenated": [
                _compact_result(r)
                for r in db.search_messages("chat-send", limit=5)
            ],
            "tool_name": [
                _compact_result(r)
                for r in db.search_messages("terminal", limit=5)
            ],
            "tool_call_arguments": [
                _compact_result(r)
                for r in db.search_messages("unique_tool_token", limit=5)
            ],
        }
        cases = [
            {"name": "sanitize_fts5_query", "queries": sanitize_cases},
            {"name": "message_search", "searches": searches},
        ]
        db.close()

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
