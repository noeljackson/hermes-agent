from __future__ import annotations

import os
import subprocess
import sys

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
        }
        for argv in [
            ["--version"],
            ["version"],
            ["config", "path"],
            ["config", "env-path"],
            ["config", "set", "display.skin", "mono"],
            ["config", "set", "terminal.timeout", "42"],
            ["config", "set", "OPENROUTER_API_KEY", "sk-test-parity"],
            ["cron", "list"],
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=30,
                env=env,
            )
            normalized_stdout = normalize_output(command_result.stdout, home)
            marker_key = "version" if argv in (["--version"], ["version"]) else None
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

        import yaml

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
        ]
    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
