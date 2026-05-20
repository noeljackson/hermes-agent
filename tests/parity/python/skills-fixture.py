from __future__ import annotations

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "skills-fixture.py"


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home() as home:
        skill_dir = home / "skills" / "demo" / "demo-skill"
        skill_dir.mkdir(parents=True)
        (skill_dir / "SKILL.md").write_text(
            """---
name: Demo Skill
description: Demonstrates parity loading.
version: 1.0.0
author: Hermes Parity
platforms: [linux, macos]
metadata:
  hermes:
    tags: [parity]
    category: testing
---
# Demo Skill

Use this deterministic skill for parity tests.
""",
            encoding="utf-8",
        )
        (skill_dir / "scripts").mkdir()
        (skill_dir / "scripts" / "helper.sh").write_text(
            "#!/bin/sh\necho helper\n", encoding="utf-8"
        )
        edge_dir = home / "skills" / "edge" / "api-tool"
        edge_dir.mkdir(parents=True)
        (edge_dir / "SKILL.md").write_text(
            """---
name: C++/API Tool
---
# API Tool

Fallback body description should be used and kept stable for parity.
""",
            encoding="utf-8",
        )
        hidden_dir = home / "skills" / ".git" / "hidden"
        hidden_dir.mkdir(parents=True)
        (hidden_dir / "SKILL.md").write_text(
            """---
name: Hidden Skill
description: Should not be visible.
---
# Hidden
""",
            encoding="utf-8",
        )
        unsupported_dir = home / "skills" / "unsupported"
        unsupported_dir.mkdir(parents=True)
        (unsupported_dir / "SKILL.md").write_text(
            """---
name: Windows Only Skill
description: Should be filtered on Linux parity runner.
platforms: [windows]
---
# Unsupported
""",
            encoding="utf-8",
        )
        empty_slug_dir = home / "skills" / "empty-slug"
        empty_slug_dir.mkdir(parents=True)
        (empty_slug_dir / "SKILL.md").write_text(
            """---
name: +++///
description: Invalid command slug should be skipped.
---
# Empty Slug
""",
            encoding="utf-8",
        )

        from agent.skill_commands import (
            build_skill_invocation_message,
            reload_skills,
            resolve_skill_command_key,
            scan_skill_commands,
        )

        commands = scan_skill_commands()
        resolve_cases = {
            "demo-skill": resolve_skill_command_key("demo-skill"),
            "demo_skill": resolve_skill_command_key("demo_skill"),
            "capi_tool": resolve_skill_command_key("capi_tool"),
            "missing": resolve_skill_command_key("missing"),
            "empty": resolve_skill_command_key(""),
        }
        new_dir = home / "skills" / "new" / "new-skill"
        new_dir.mkdir(parents=True)
        (new_dir / "SKILL.md").write_text(
            """---
name: New Skill
description: Added during reload.
---
# New Skill
""",
            encoding="utf-8",
        )
        (edge_dir / "SKILL.md").unlink()
        reload_diff = reload_skills()
        invocation = build_skill_invocation_message(
            "/demo-skill",
            user_instruction="Use it now.",
            task_id="session-1",
            runtime_note="gateway runtime",
        )
        cases = [
            {
                "name": "user_skill_command_scan",
                "commands": {
                    key: {
                        "name": value.get("name"),
                        "description": value.get("description"),
                        "skill_md_basename": "SKILL.md",
                    }
                    for key, value in sorted(commands.items())
                },
            },
            {
                "name": "skill_command_resolution",
                "cases": resolve_cases,
            },
            {
                "name": "reload_skills_diff",
                "diff": reload_diff,
            },
            {
                "name": "skill_invocation_message",
                "message": invocation.replace(str(home), "<HERMES_HOME>"),
            },
        ]
    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
