from __future__ import annotations

import json
import os
import subprocess
import tempfile
from pathlib import Path

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "tool-execution-fixture.py"


def parsed(result: str):
    return json.loads(result)


class LocalFixtureEnv:
    def __init__(self, cwd: str):
        self.cwd = cwd

    def execute(self, command, cwd=None, timeout=None, stdin_data=None):
        result = subprocess.run(
            command,
            shell=True,
            cwd=cwd or self.cwd,
            input=stdin_data,
            text=True,
            capture_output=True,
            timeout=timeout,
        )
        return {
            "output": result.stdout + result.stderr,
            "returncode": result.returncode,
        }


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home():
        from model_tools import handle_function_call
        from tools.clarify_tool import clarify_tool
        from tools.file_operations import ShellFileOperations
        import tools.file_tools as file_tools
        from tools.memory_tool import MemoryStore, memory_tool
        from tools.registry import tool_error
        from tools.skills_tool import skills_list

        cases = [
            {
                "name": "clarify_empty_question",
                "result": parsed(clarify_tool("  ")),
            },
            {
                "name": "clarify_no_callback",
                "result": parsed(
                    clarify_tool(
                        "Pick one",
                        choices=[" first ", "second", "", "third", "fourth", "fifth"],
                    )
                ),
            },
            {
                "name": "agent_loop_tool_block",
                "result": parsed(
                    handle_function_call(
                        "memory",
                        {"action": "add", "target": "memory", "content": "Remember this."},
                    )
                ),
            },
            {
                "name": "unknown_tool_error",
                "result": parsed(handle_function_call("__missing_tool__", {})),
            },
            {
                "name": "tool_error_with_extra",
                "result": parsed(tool_error("bad input", success=False, code=400)),
            },
        ]

        memory_store = MemoryStore(memory_char_limit=500, user_char_limit=500)
        memory_store.load_from_disk()
        cases.extend(
            [
                {
                    "name": "memory_handler_add",
                    "result": parsed(
                        memory_tool(
                            "add",
                            target="memory",
                            content="Tool handler remembers durable facts.",
                            store=memory_store,
                        )
                    ),
                },
                {
                    "name": "memory_handler_replace",
                    "result": parsed(
                        memory_tool(
                            "replace",
                            target="memory",
                            old_text="durable facts",
                            content="Tool handler remembers Rust parity facts.",
                            store=memory_store,
                        )
                    ),
                },
                {
                    "name": "memory_handler_remove_missing",
                    "result": parsed(
                        memory_tool(
                            "remove",
                            target="memory",
                            old_text="not present",
                            store=memory_store,
                        )
                    ),
                },
            ]
        )

        skills_root = Path(os.environ["HERMES_HOME"]) / "skills"
        demo_skill = skills_root / "testing" / "demo-skill"
        demo_skill.mkdir(parents=True)
        (demo_skill / "SKILL.md").write_text(
            """---
name: Demo Skill
description: Demonstrates tool handler listing.
platforms: [linux, macos]
---
# Demo Skill
""",
            encoding="utf-8",
        )
        root_skill = skills_root / "root-skill"
        root_skill.mkdir(parents=True)
        (root_skill / "SKILL.md").write_text(
            """---
name: Root Skill
---
# Root Skill

Fallback description for root skill.
""",
            encoding="utf-8",
        )
        cases.extend(
            [
                {
                    "name": "skills_list_handler_all",
                    "result": parsed(skills_list()),
                },
                {
                    "name": "skills_list_handler_category",
                    "result": parsed(skills_list(category="testing")),
                },
            ]
        )

        with tempfile.TemporaryDirectory(prefix="hermes-file-tools-") as tmp:
            workspace = Path(tmp)
            (workspace / "notes.txt").write_text(
                "alpha\nbeta\nalpha beta\n", encoding="utf-8"
            )
            (workspace / "patch.txt").write_text(
                "alpha\nbeta\nalpha beta\n", encoding="utf-8"
            )
            (workspace / "nested").mkdir()
            (workspace / "nested" / "alpha.md").write_text(
                "nested alpha\n", encoding="utf-8"
            )
            task_id = "file-tool-fixture"
            os.environ["TERMINAL_CWD"] = str(workspace)
            file_tools.clear_file_ops_cache(task_id)
            with file_tools._file_ops_lock:
                file_tools._file_ops_cache[task_id] = ShellFileOperations(
                    LocalFixtureEnv(str(workspace))
                )

            cases.extend(
                [
                    {
                        "name": "read_file_handler",
                        "result": parsed(
                            file_tools._handle_read_file(
                                {"path": "notes.txt", "offset": 2, "limit": 2},
                                task_id=task_id,
                            )
                        ),
                    },
                    {
                        "name": "write_file_handler_missing_content",
                        "result": parsed(
                            file_tools._handle_write_file(
                                {"path": "created.txt"}, task_id=task_id
                            )
                        ),
                    },
                    {
                        "name": "write_file_handler",
                        "result": parsed(
                            file_tools._handle_write_file(
                                {"path": "created.txt", "content": "created\n"},
                                task_id=task_id,
                            )
                        ),
                        "file_content": (workspace / "created.txt").read_text(
                            encoding="utf-8"
                        ),
                    },
                    {
                        "name": "patch_replace_handler",
                        "result": parsed(
                            file_tools._handle_patch(
                                {
                                    "mode": "replace",
                                    "path": "patch.txt",
                                    "old_string": "alpha beta",
                                    "new_string": "alpha BETA",
                                },
                                task_id=task_id,
                            )
                        ),
                        "file_content": (workspace / "patch.txt").read_text(
                            encoding="utf-8"
                        ),
                    },
                    {
                        "name": "search_files_files_handler",
                        "result": parsed(
                            file_tools._handle_search_files(
                                {
                                    "pattern": "*.md",
                                    "target": "files",
                                    "path": ".",
                                    "limit": 5,
                                },
                                task_id=task_id,
                            )
                        ),
                    },
                ]
            )

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
