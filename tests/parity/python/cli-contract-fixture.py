from __future__ import annotations

import os
import json
import subprocess
import sys
import tarfile

from parity_common import (
    fixture,
    isolated_hermes_home,
    normalize_timestamps,
    parse_out_arg,
    write_fixture,
)


SCRIPT = "cli-contract-fixture.py"


def normalize_output(text: str, home) -> str:
    return text.replace("\r\n", "\n").replace(str(home), "<HERMES_HOME>")


def cron_state(home):
    jobs_path = home / "cron" / "jobs.json"
    if not jobs_path.exists():
        return {"job_count": 0, "jobs": []}
    raw = json.loads(jobs_path.read_text(encoding="utf-8"))
    jobs = raw.get("jobs", raw if isinstance(raw, list) else [])
    simplified = []
    for job in jobs:
        simplified.append(
            {
                "id_present": bool(job.get("id")),
                "name": job.get("name"),
                "prompt": job.get("prompt"),
                "schedule_display": job.get("schedule_display"),
                "next_run_at": job.get("next_run_at"),
                "enabled": job.get("enabled"),
                "state": job.get("state"),
                "deliver": job.get("deliver"),
                "repeat": job.get("repeat"),
            }
        )
    return {"job_count": len(simplified), "jobs": simplified}


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home() as home:
        from hermes_cli.main import _BUILTIN_SUBCOMMANDS

        env = os.environ.copy()
        env["NO_COLOR"] = "1"
        result = subprocess.run(
            [sys.executable, "-m", "hermes_cli.main", "--help"],
            text=True,
            capture_output=True,
            timeout=30,
            env=env,
        )
        stdout = result.stdout.replace("\r\n", "\n")
        stderr = result.stderr.replace("\r\n", "\n")
        markers = ["Usage", "setup", "config", "tools", "gateway", "logs"]
        subcommand_cases = []
        for argv, markers_for_command in [
            (
                ["config", "--help"],
                ["show", "edit", "set", "path", "env-path", "check", "migrate"],
            ),
            (
                ["tools", "--help"],
                ["list", "enable", "disable", "--platform"],
            ),
            (
                ["mcp", "--help"],
                ["serve", "add", "remove", "list", "test", "configure", "login"],
            ),
            (
                ["sessions", "--help"],
                ["list", "export", "delete", "prune", "stats", "rename", "browse"],
            ),
            (
                ["cron", "--help"],
                ["list", "create", "pause", "resume", "remove", "tick"],
            ),
            (
                ["gateway", "--help"],
                ["run", "start", "stop", "restart", "status", "setup"],
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=30,
                env=env,
            )
            command_stdout = command_result.stdout.replace("\r\n", "\n")
            command_stderr = command_result.stderr.replace("\r\n", "\n")
            subcommand_cases.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stdout_markers": {
                        marker: marker in command_stdout
                        for marker in markers_for_command
                    },
                    "stderr_empty": not bool(command_stderr.strip()),
                }
            )
        execution_cases = []
        version_marker_names = {
            "version": ["Hermes Agent v", "Project:", "Python:", "OpenAI SDK:"],
            "tools_list": ["Built-in toolsets (cli):", "web", "terminal", "memory", "computer_use"],
            "mcp_list": ["No MCP servers configured.", "hermes mcp add <name>"],
            "config_check": ["Configuration Status", "Config version:", "Optional:", "OPENROUTER_API_KEY"],
            "sessions_list_empty": ["No sessions found."],
        }
        for argv in [
            ["--version"],
            ["version"],
            ["config", "path"],
            ["config", "env-path"],
            ["config", "check"],
            ["config", "set", "display.skin", "mono"],
            ["config", "set", "terminal.timeout", "42"],
            ["config", "set", "OPENROUTER_API_KEY", "sk-test-parity"],
            ["cron", "list"],
            ["mcp", "list"],
            ["sessions", "list", "--limit", "5"],
            ["tools", "list"],
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=30,
                env=env,
            )
            normalized_stdout = normalize_output(command_result.stdout, home)
            marker_key = None
            if argv in (["--version"], ["version"]):
                marker_key = "version"
            elif argv == ["tools", "list"]:
                marker_key = "tools_list"
            elif argv == ["mcp", "list"]:
                marker_key = "mcp_list"
            elif argv == ["config", "check"]:
                marker_key = "config_check"
            elif argv == ["sessions", "list", "--limit", "5"]:
                marker_key = "sessions_list_empty"
            stdout_markers = None
            if marker_key:
                stdout_markers = {
                    marker: marker in normalized_stdout
                    for marker in version_marker_names[marker_key]
                }
            execution_cases.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stdout": "" if stdout_markers else normalized_stdout,
                    "stdout_markers": stdout_markers or {},
                    "stderr": normalize_output(command_result.stderr, home),
                }
            )

        from hermes_state import SessionDB

        db = SessionDB(home / "state.db")
        db.create_session(
            "cli-session-1",
            "cli",
            user_id="user-cli",
            model="fake/model",
            model_config={"provider": "fake"},
            system_prompt="system",
        )
        db.append_message("cli-session-1", "user", "hello cli")
        db.create_session(
            "telegram-session-1",
            "telegram",
            user_id="user-telegram",
            model="fake/model",
            model_config={"provider": "fake"},
            system_prompt="system",
        )
        db.append_message("telegram-session-1", "user", "hello telegram")
        db.close()

        session_commands = []
        for argv in [
            ["sessions", "list", "--limit", "5"],
            ["sessions", "export", "-"],
            ["sessions", "stats"],
            ["sessions", "export", "-", "--session-id", "cli-session-1"],
            ["sessions", "rename", "cli-session-1", "Renamed", "Session"],
            ["sessions", "delete", "telegram-session-1", "--yes"],
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=30,
                env=env,
            )
            normalized_stdout = normalize_output(command_result.stdout, home)
            case = {
                "argv": ["hermes", *argv],
                "exit_code": command_result.returncode,
                "stderr": normalize_output(command_result.stderr, home),
                "stdout": normalized_stdout,
                "stdout_markers": {},
            }
            if argv[:2] == ["sessions", "list"]:
                case["stdout"] = ""
                case["stdout_markers"] = {
                    marker: marker in normalized_stdout
                    for marker in [
                        "hello cli",
                        "hello telegram",
                        "cli-session-1",
                        "telegram-session-1",
                    ]
                }
            elif argv == ["sessions", "stats"]:
                case["stdout"] = ""
                case["stdout_markers"] = {
                    marker: marker in normalized_stdout
                    for marker in [
                        "Total sessions: 2",
                        "Total messages: 2",
                        "cli: 1 sessions",
                        "telegram: 1 sessions",
                        "Database size:",
                    ]
                }
            elif argv == ["sessions", "export", "-"]:
                import json

                case["stdout"] = ""
                case["exports"] = [
                    normalize_timestamps(json.loads(line))
                    for line in normalized_stdout.splitlines()
                    if line.strip()
                ]
            elif argv[:3] == ["sessions", "export", "-"]:
                import json

                case["stdout"] = ""
                case["export"] = normalize_timestamps(json.loads(normalized_stdout))
            session_commands.append(case)

        final_db = SessionDB(home / "state.db")
        session_state = {
            "session_count": final_db.session_count(),
            "message_count": final_db.message_count(),
            "renamed_title": final_db.get_session_title("cli-session-1"),
            "deleted_session": final_db.get_session("telegram-session-1"),
        }
        final_db.close()

        profile_commands = []
        for argv, marker_list in [
            (
                ["profile"],
                ["Active profile: default", "Path:", "Gateway:", "Skills:"],
            ),
            (
                [
                    "profile",
                    "create",
                    "research",
                    "--no-alias",
                    "--no-skills",
                    "--description",
                    "Research work",
                ],
                [
                    "Profile 'research' created",
                    "No bundled skills seeded",
                    "Next steps:",
                    "research setup",
                ],
            ),
            (
                ["profile", "describe", "research"],
                ["Research work"],
            ),
            (
                ["profile", "describe", "research", "--text", "Updated role"],
                ["Description updated for 'research'."],
            ),
            (
                ["profile", "describe", "research"],
                ["Updated role"],
            ),
            (
                ["profile", "show", "research"],
                ["Profile: research", "Gateway: stopped", "Skills:", "SOUL.md: exists"],
            ),
            (
                ["profile", "use", "research"],
                ["Switched to: research"],
            ),
            (
                ["profile", "list"],
                ["Profile", "default", "research", "stopped"],
            ),
            (
                ["profile", "delete", "research", "--yes"],
                ["Profile 'research' deleted.", "Active profile reset to default"],
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=60,
                env=env,
            )
            normalized_stdout = normalize_output(command_result.stdout, home)
            profile_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(command_result.stderr, home),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout for marker in marker_list
                    },
                }
            )

        active_profile_file = home / "active_profile"
        profile_state = {
            "active_profile_file": (
                active_profile_file.read_text(encoding="utf-8").strip()
                if active_profile_file.exists()
                else None
            ),
            "research_exists": (home / "profiles" / "research").exists(),
            "profiles_root_exists": (home / "profiles").exists(),
        }

        archive_home = home / "profile-archive-home"
        archive_env = env.copy()
        archive_env["HERMES_HOME"] = str(archive_home)
        archive_commands = []
        archive_path = archive_home / "archive.tar.gz"
        for argv, marker_list in [
            (
                [
                    "profile",
                    "create",
                    "archive",
                    "--no-alias",
                    "--no-skills",
                    "--description",
                    "Archive role",
                ],
                ["Profile 'archive' created", "No bundled skills seeded"],
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=60,
                env=archive_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, archive_home)
            archive_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(command_result.stderr, archive_home),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout for marker in marker_list
                    },
                }
            )

        archive_profile = archive_home / "profiles" / "archive"
        (archive_profile / "memories").mkdir(parents=True, exist_ok=True)
        (archive_profile / "skills" / "demo").mkdir(parents=True, exist_ok=True)
        (archive_profile / "config.yaml").write_text("model: archive-model\n", encoding="utf-8")
        (archive_profile / ".env").write_text(
            "OPENROUTER_API_KEY=sk-secret-not-exported\n", encoding="utf-8"
        )
        (archive_profile / "auth.json").write_text(
            '{"token":"secret-not-exported"}\n', encoding="utf-8"
        )
        (archive_profile / "SOUL.md").write_text("Archive soul.\n", encoding="utf-8")
        (archive_profile / "memories" / "MEMORY.md").write_text(
            "Remember archive.\n", encoding="utf-8"
        )
        (archive_profile / "skills" / "demo" / "SKILL.md").write_text(
            "# Demo Skill\n", encoding="utf-8"
        )

        for argv, marker_list in [
            (
                ["profile", "export", "archive", "-o", str(archive_path)],
                ["Exported 'archive'"],
            ),
            (
                ["profile", "import", str(archive_path), "--name", "restored"],
                ["Imported profile 'restored'"],
            ),
            (
                ["profile", "import", str(archive_path), "--name", "restored"],
                ["Error:", "already exists"],
            ),
            (
                ["profile", "export", "missing", "-o", str(archive_home / "missing.tar.gz")],
                ["Error:", "does not exist"],
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=60,
                env=archive_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, archive_home)
            archive_commands.append(
                {
                    "argv": [
                        part.replace(str(archive_home), "<HERMES_HOME>")
                        for part in ["hermes", *argv]
                    ],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(command_result.stderr, archive_home),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout for marker in marker_list
                    },
                }
            )

        with tarfile.open(archive_path, "r:gz") as tf:
            archive_members = sorted(tf.getnames())
        restored = archive_home / "profiles" / "restored"
        archive_state = {
            "archive_members": archive_members,
            "contains_env": any(name.endswith("/.env") for name in archive_members),
            "contains_auth_json": any(name.endswith("/auth.json") for name in archive_members),
            "contains_config": "archive/config.yaml" in archive_members,
            "contains_soul": "archive/SOUL.md" in archive_members,
            "contains_memory": "archive/memories/MEMORY.md" in archive_members,
            "contains_skill": "archive/skills/demo/SKILL.md" in archive_members,
            "restored_exists": restored.exists(),
            "restored_config": (restored / "config.yaml").read_text(encoding="utf-8"),
            "restored_memory": (restored / "memories" / "MEMORY.md").read_text(
                encoding="utf-8"
            ),
            "restored_env_exists": (restored / ".env").exists(),
            "restored_auth_exists": (restored / "auth.json").exists(),
        }

        import yaml

        tool_home = home / "tools-command-home"
        tool_env = env.copy()
        tool_env["HERMES_HOME"] = str(tool_home)
        tool_commands = []
        for argv, marker_list in [
            (["tools", "enable", "video"], ["Enabled: video"]),
            (["tools", "disable", "browser"], ["Disabled: browser"]),
            (
                ["tools", "list"],
                [
                    "Built-in toolsets (cli):",
                    "✗ disabled  browser",
                    "✓ enabled  video",
                    "✓ enabled  terminal",
                ],
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=60,
                env=tool_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, tool_home)
            tool_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(command_result.stderr, tool_home),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout for marker in marker_list
                    },
                }
            )
        tool_config = {}
        tool_config_path = tool_home / "config.yaml"
        if tool_config_path.exists():
            tool_config = yaml.safe_load(tool_config_path.read_text(encoding="utf-8")) or {}
        tool_state = {
            "platform_toolsets_cli": sorted(
                str(item)
                for item in (
                    (tool_config.get("platform_toolsets") or {}).get("cli") or []
                )
            ),
            "browser_enabled": "browser"
            in ((tool_config.get("platform_toolsets") or {}).get("cli") or []),
            "video_enabled": "video"
            in ((tool_config.get("platform_toolsets") or {}).get("cli") or []),
            "default_composite_present": "hermes-cli"
            in ((tool_config.get("platform_toolsets") or {}).get("cli") or []),
        }

        cron_home = home / "cron-command-home"
        cron_env = env.copy()
        cron_env["HERMES_HOME"] = str(cron_home)
        cron_commands = []
        cron_states = {}
        for argv, marker_list, state_name in [
            (
                [
                    "cron",
                    "create",
                    "2026-06-01T09:00:00+00:00",
                    "check status",
                    "--name",
                    "demo",
                    "--deliver",
                    "local",
                ],
                ["Created job:", "Name: demo", "Schedule: once at 2026-06-01 09:00"],
                "after_create",
            ),
            (
                ["cron", "list", "--all"],
                ["Scheduled Jobs", "demo", "[active]", "Deliver:   local"],
                None,
            ),
            (["cron", "pause", "demo"], ["Paused job: demo"], "after_pause"),
            (
                ["cron", "list", "--all"],
                ["Scheduled Jobs", "demo", "[paused]"],
                None,
            ),
            (["cron", "resume", "demo"], ["Resumed job: demo"], "after_resume"),
            (["cron", "remove", "demo"], ["Removed job: demo"], "after_remove"),
            (
                ["cron", "list", "--all"],
                ["No scheduled jobs.", "hermes cron create"],
                None,
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=60,
                env=cron_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, cron_home)
            cron_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(command_result.stderr, cron_home),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout for marker in marker_list
                    },
                }
            )
            if state_name:
                cron_states[state_name] = cron_state(cron_home)

        config_state = {}
        config_path = home / "config.yaml"
        if config_path.exists():
            config_state = yaml.safe_load(config_path.read_text(encoding="utf-8")) or {}
        env_path = home / ".env"
        env_lines = []
        if env_path.exists():
            env_lines = [
                line.strip()
                for line in env_path.read_text(encoding="utf-8").splitlines()
                if line.strip()
            ]
        cases = [
            {
                "name": "top_level_help",
                "argv": ["hermes", "--help"],
                "exit_code": result.returncode,
                "stdout_markers": {marker: marker in stdout for marker in markers},
                "stderr_empty": not bool(stderr.strip()),
            },
            {
                "name": "builtin_subcommand_inventory",
                "commands": sorted(_BUILTIN_SUBCOMMANDS),
            },
            {
                "name": "selected_subcommand_help",
                "commands": subcommand_cases,
            },
            {
                "name": "safe_command_execution",
                "commands": execution_cases,
            },
            {
                "name": "safe_command_file_state",
                "config": config_state,
                "env_lines": env_lines,
            },
            {
                "name": "safe_session_command_execution",
                "commands": session_commands,
                "state": session_state,
            },
            {
                "name": "safe_profile_command_execution",
                "commands": profile_commands,
                "state": profile_state,
            },
            {
                "name": "safe_profile_archive_command_execution",
                "commands": archive_commands,
                "state": archive_state,
            },
            {
                "name": "safe_tools_command_execution",
                "commands": tool_commands,
                "state": tool_state,
            },
            {
                "name": "safe_cron_command_execution",
                "commands": cron_commands,
                "states": cron_states,
            },
        ]
    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
