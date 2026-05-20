from __future__ import annotations

from parity_common import (
    fixture,
    isolated_hermes_home,
    normalize_timestamps,
    parse_out_arg,
    write_fixture,
)


SCRIPT = "session-export-fixture.py"


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home() as home:
        import sqlite3

        from hermes_state import SessionDB, _set_last_init_error, format_session_db_unavailable

        db = SessionDB(home / "state.db")
        session_id = "parity-session-1"
        db.create_session(
            session_id,
            "cli",
            user_id="user-1",
            model="fake/model",
            model_config={"provider": "fake"},
            system_prompt="system prompt",
        )
        db.append_message(session_id, "user", "hello")
        db.append_message(
            session_id,
            "assistant",
            "calling tool",
            tool_calls=[
                {
                    "id": "call-1",
                    "type": "function",
                    "function": {"name": "memory", "arguments": "{}"},
                }
            ],
            finish_reason="tool_calls",
        )
        db.append_message(
            session_id,
            "tool",
            "{\"success\": true}",
            tool_call_id="call-1",
            tool_name="memory",
        )
        exported = normalize_timestamps(db.export_session(session_id))
        conversation = normalize_timestamps(db.get_messages_as_conversation(session_id))
        exported_all = normalize_timestamps(db.export_all(source="cli"))

        state_db = SessionDB(home / "state-ops.db")
        for sid in (
            "alpha-111",
            "alpha-222",
            "beta-111",
            "literal%one",
            "literal_one",
            "parent-delete",
            "child-delete",
        ):
            state_db.create_session(
                sid,
                "cli",
                user_id="user-state",
                model="fake/model",
                model_config={"provider": "fake"},
                system_prompt="system prompt",
                parent_session_id="parent-delete" if sid == "child-delete" else None,
            )
        state_db.append_message("parent-delete", "user", "delete me")
        state_db.append_message("parent-delete", "assistant", "deleted")
        state_db.append_message("child-delete", "user", "keep child")

        title_inputs = [
            ("none", None),
            ("empty", ""),
            ("whitespace", " \t\n "),
            ("collapsed", "  My\tSession\nTitle  "),
            ("ascii_control", "bad\x00 title\x7f"),
            ("unicode_control", "zero\u200bwidth\u202e"),
            ("max_len", "x" * 100),
            ("too_long", "x" * 101),
        ]
        title_cases = []
        for name, value in title_inputs:
            try:
                title_cases.append(
                    {
                        "name": name,
                        "input": value,
                        "ok": True,
                        "value": state_db.sanitize_title(value),
                    }
                )
            except ValueError as exc:
                title_cases.append(
                    {
                        "name": name,
                        "input": value,
                        "ok": False,
                        "error": str(exc),
                    }
                )

        set_title_ok = state_db.set_session_title(
            "alpha-111", "  My\tSession\nTitle  "
        )
        title_after_set = state_db.get_session_title("alpha-111")
        by_title = normalize_timestamps(state_db.get_session_by_title("My Session Title"))
        missing_title_result = state_db.set_session_title("missing-session", "Missing")
        try:
            state_db.set_session_title("beta-111", "My Session Title")
            duplicate_title_error = None
        except ValueError as exc:
            duplicate_title_error = str(exc)

        resolve_cases = {
            "exact": state_db.resolve_session_id("alpha-111"),
            "unique_prefix": state_db.resolve_session_id("beta"),
            "ambiguous_prefix": state_db.resolve_session_id("alpha"),
            "missing_prefix": state_db.resolve_session_id("missing"),
            "literal_percent_prefix": state_db.resolve_session_id("literal%"),
            "literal_underscore_prefix": state_db.resolve_session_id("literal_"),
        }

        counts_before_delete = {
            "sessions_all": state_db.session_count(),
            "sessions_cli": state_db.session_count(source="cli"),
            "sessions_gateway": state_db.session_count(source="gateway"),
            "messages_all": state_db.message_count(),
            "messages_parent": state_db.message_count("parent-delete"),
            "messages_missing": state_db.message_count("missing-session"),
        }

        sessions_dir = home / "sessions"
        sessions_dir.mkdir()
        for name in (
            "parent-delete.json",
            "parent-delete.jsonl",
            "request_dump_parent-delete_1.json",
        ):
            (sessions_dir / name).write_text("{}", encoding="utf-8")
        delete_result = state_db.delete_session("parent-delete", sessions_dir=sessions_dir)
        delete_again_result = state_db.delete_session("parent-delete", sessions_dir=sessions_dir)
        child_after_delete = normalize_timestamps(state_db.get_session("child-delete"))
        state_ops = {
            "title_cases": title_cases,
            "set_title_ok": set_title_ok,
            "title_after_set": title_after_set,
            "by_title": by_title,
            "missing_title_result": missing_title_result,
            "duplicate_title_error": duplicate_title_error,
            "resolve_cases": resolve_cases,
            "counts_before_delete": counts_before_delete,
            "delete": {
                "first": delete_result,
                "second": delete_again_result,
                "parent_after": state_db.get_session("parent-delete"),
                "child_parent_session_id": child_after_delete["parent_session_id"],
                "sessions_all": state_db.session_count(),
                "messages_all": state_db.message_count(),
                "messages_parent": state_db.message_count("parent-delete"),
                "transcript_files_remaining": sorted(
                    path.name for path in sessions_dir.glob("*")
                ),
            },
        }

        legacy_path = home / "legacy-state.db"
        legacy_conn = sqlite3.connect(legacy_path)
        legacy_conn.executescript(
            """
            CREATE TABLE schema_version (version INTEGER NOT NULL);
            INSERT INTO schema_version (version) VALUES (1);
            CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                source TEXT,
                user_id TEXT,
                model TEXT,
                model_config TEXT,
                system_prompt TEXT,
                parent_session_id TEXT,
                started_at REAL NOT NULL
            );
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL REFERENCES sessions(id),
                role TEXT NOT NULL,
                content TEXT,
                timestamp REAL NOT NULL
            );
            INSERT INTO sessions (
                id, source, user_id, model, model_config, system_prompt, started_at
            ) VALUES (
                'legacy-session', 'cli', 'user-legacy', 'fake/model',
                '{"provider":"fake"}', 'system prompt', 1710000000.0
            );
            INSERT INTO messages (session_id, role, content, timestamp)
            VALUES ('legacy-session', 'user', 'legacy hello', 1710000001.0);
            """
        )
        legacy_conn.commit()
        legacy_conn.close()
        legacy_db = SessionDB(legacy_path)
        legacy_export = normalize_timestamps(legacy_db.export_session("legacy-session"))
        legacy_conversation = normalize_timestamps(
            legacy_db.get_messages_as_conversation("legacy-session")
        )
        legacy_columns_conn = sqlite3.connect(legacy_path)
        legacy_migration = {
            "session": legacy_export,
            "conversation": legacy_conversation,
            "schema_version": legacy_columns_conn.execute(
                "SELECT version FROM schema_version LIMIT 1"
            ).fetchone()[0],
            "sessions_columns": [
                row[1]
                for row in legacy_columns_conn.execute("PRAGMA table_info(sessions)")
            ],
            "messages_columns": [
                row[1]
                for row in legacy_columns_conn.execute("PRAGMA table_info(messages)")
            ],
            "fts_tables": sorted(
                row[0]
                for row in legacy_columns_conn.execute(
                    "SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'messages_fts%'"
                )
            ),
        }
        legacy_columns_conn.close()

        _set_last_init_error(None)
        unavailable_messages = {
            "no_cause": format_session_db_unavailable(),
        }
        _set_last_init_error("OperationalError: locking protocol")
        unavailable_messages["wal_incompatible"] = format_session_db_unavailable()
        unavailable_messages["custom_prefix"] = format_session_db_unavailable(
            "Resume unavailable"
        )
        _set_last_init_error("OperationalError: database is locked")
        unavailable_messages["plain_error"] = format_session_db_unavailable()
        _set_last_init_error(None)

    cases = [
        {"name": "single_session_export", "session": exported},
        {"name": "resume_conversation_shape", "messages": conversation},
        {"name": "export_all_shape", "sessions": exported_all},
        {"name": "session_state_operations", "state": state_ops},
        {"name": "legacy_schema_migration", "migration": legacy_migration},
        {"name": "db_unavailable_error_format", "messages": unavailable_messages},
    ]
    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
