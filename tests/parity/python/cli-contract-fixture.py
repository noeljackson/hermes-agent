from __future__ import annotations

import os
import json
import sqlite3
import stat
import subprocess
import sys
import tarfile
import time
import zipfile
import yaml

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


def quick_snapshot_state(home):
    root = home / "state-snapshots"
    snapshots = sorted([p for p in root.iterdir() if p.is_dir()]) if root.exists() else []
    if not snapshots:
        return {"snapshot_count": 0, "files": [], "manifest": {}}
    snap = snapshots[-1]
    manifest = json.loads((snap / "manifest.json").read_text(encoding="utf-8"))
    files = sorted(
        str(path.relative_to(snap)).replace(os.sep, "/")
        for path in snap.rglob("*")
        if path.is_file()
    )
    manifest_files = dict(sorted((manifest.get("files") or {}).items()))
    return {
        "snapshot_count": len(snapshots),
        "id_has_label": str(manifest.get("id", "")).endswith("-parity"),
        "label": manifest.get("label"),
        "file_count": manifest.get("file_count"),
        "total_size": manifest.get("total_size"),
        "files": files,
        "manifest_files": manifest_files,
    }


def main() -> int:
    import json

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
            "config_show": [
                "Hermes Configuration",
                "Paths",
                "Config:",
                "Secrets:",
                "API Keys",
                "Model",
                "Terminal",
                "Backend:",
                "Context Compression",
                "Messaging Platforms",
            ],
            "sessions_list_empty": ["No sessions found."],
            "completion_bash": [
                "# Hermes Agent bash completion",
                "_hermes_completion()",
                "complete -F _hermes_completion hermes",
                "config",
                "profile",
            ],
            "completion_zsh": [
                "#compdef hermes",
                "_arguments -C",
                "compdef _hermes hermes",
                "config",
                "profile",
            ],
            "completion_fish": [
                "# Hermes Agent fish completion",
                "function __hermes_profiles",
                "complete -c hermes -f",
                "config",
                "profile",
            ],
            "status": [
                "Hermes Agent Status",
                "Environment",
                "API Keys",
                "Auth Providers",
                "Terminal Backend",
                "Messaging Platforms",
                "Gateway Service",
                "Scheduled Jobs",
                "Sessions",
            ],
        }
        for argv in [
            ["--version"],
            ["version"],
            ["status"],
            ["logout"],
            ["config", "path"],
            ["config", "env-path"],
            ["config", "check"],
            ["config", "set", "display.skin", "mono"],
            ["config", "set", "terminal.timeout", "42"],
            ["config", "set", "terminal.cwd", "/workspace/project"],
            ["config", "set", "OPENROUTER_API_KEY", "sk-test-parity"],
            ["config", "set"],
            ["config", "set", "model.provider"],
            ["config", "edit"],
            ["config", "show"],
            ["completion", "bash"],
            ["completion", "zsh"],
            ["completion", "fish"],
            ["computer-use", "status"],
            ["cron", "list"],
            ["dashboard", "--status"],
            ["dashboard", "--stop"],
            ["hooks", "list"],
            ["hooks", "doctor"],
            ["insights"],
            ["insights", "--days", "7", "--source", "cli"],
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
            elif argv == ["status"]:
                marker_key = "status"
            elif argv == ["tools", "list"]:
                marker_key = "tools_list"
            elif argv == ["mcp", "list"]:
                marker_key = "mcp_list"
            elif argv == ["config", "check"]:
                marker_key = "config_check"
            elif argv == ["config", "show"]:
                marker_key = "config_show"
            elif argv == ["sessions", "list", "--limit", "5"]:
                marker_key = "sessions_list_empty"
            elif argv == ["completion", "bash"]:
                marker_key = "completion_bash"
            elif argv == ["completion", "zsh"]:
                marker_key = "completion_zsh"
            elif argv == ["completion", "fish"]:
                marker_key = "completion_fish"
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

        auth_home = home / "auth-command-home"
        auth_env = env.copy()
        auth_env["HERMES_HOME"] = str(auth_home)
        for secret_name in [
            "OPENROUTER_API_KEY",
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "NOUS_API_KEY",
        ]:
            auth_env.pop(secret_name, None)
        auth_commands = []
        for argv in [
            ["auth", "list"],
            ["auth", "list", "openrouter"],
            ["auth", "status", "openrouter"],
            ["auth", "status", "nous"],
            ["auth", "logout", "openrouter"],
            ["auth", "reset", "openrouter"],
            ["auth", "remove", "openrouter", "missing"],
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=30,
                env=auth_env,
            )
            auth_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stdout": normalize_output(command_result.stdout, auth_home),
                    "stdout_markers": {},
                    "stderr": normalize_output(command_result.stderr, auth_home),
                }
            )

        memory_home = home / "memory-command-home"
        memory_env = env.copy()
        memory_env["HERMES_HOME"] = str(memory_home)
        (memory_home / "memories").mkdir(parents=True, exist_ok=True)
        (memory_home / "memories" / "MEMORY.md").write_text(
            "remember\n", encoding="utf-8"
        )
        (memory_home / "memories" / "USER.md").write_text("user\n", encoding="utf-8")
        memory_commands = []
        for argv in [
            ["memory", "status"],
            ["memory", "off"],
            ["memory", "status"],
            ["memory", "reset", "--target", "memory", "--yes"],
            ["memory", "reset", "--target", "user", "--yes"],
            ["memory", "reset", "--target", "all", "--yes"],
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=30,
                env=memory_env,
            )
            memory_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stdout": normalize_output(command_result.stdout, memory_home),
                    "stdout_markers": {},
                    "stderr": normalize_output(command_result.stderr, memory_home),
                }
            )
        memory_config_path = memory_home / "config.yaml"
        memory_config = {}
        if memory_config_path.exists():
            memory_config = yaml.safe_load(
                memory_config_path.read_text(encoding="utf-8")
            ) or {}
        memory_state = {
            "provider": (memory_config.get("memory") or {}).get("provider"),
            "memory_exists": (memory_home / "memories" / "MEMORY.md").exists(),
            "user_exists": (memory_home / "memories" / "USER.md").exists(),
        }

        pairing_home = home / "pairing-command-home"
        pairing_env = env.copy()
        pairing_env["HERMES_HOME"] = str(pairing_home)
        (pairing_home / ".env").parent.mkdir(parents=True, exist_ok=True)
        (pairing_home / ".env").write_text("", encoding="utf-8")
        pairing_dir = pairing_home / "pairing"
        pairing_dir.mkdir(parents=True, exist_ok=True)
        now = time.time()
        (pairing_dir / "telegram-pending.json").write_text(
            json.dumps(
                {
                    "TEST1234": {
                        "user_id": "U123",
                        "user_name": "Ada",
                        "created_at": now,
                    }
                },
                indent=2,
            ),
            encoding="utf-8",
        )
        (pairing_dir / "discord-pending.json").write_text(
            json.dumps(
                {
                    "DISC1234": {
                        "user_id": "D123",
                        "user_name": "Dee",
                        "created_at": now,
                    }
                },
                indent=2,
            ),
            encoding="utf-8",
        )
        (pairing_dir / "slack-approved.json").write_text(
            json.dumps(
                {"S123": {"user_name": "Sam", "approved_at": now}},
                indent=2,
            ),
            encoding="utf-8",
        )
        pairing_commands = []
        for argv in [
            ["pairing", "list"],
            ["pairing", "approve", "telegram", "TEST1234"],
            ["pairing", "revoke", "slack", "S123"],
            ["pairing", "approve", "telegram", "MISSING"],
            ["pairing", "revoke", "telegram", "nobody"],
            ["pairing", "clear-pending"],
            ["pairing", "list"],
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=30,
                env=pairing_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, pairing_home)
            case = {
                "argv": ["hermes", *argv],
                "exit_code": command_result.returncode,
                "stdout": normalized_stdout,
                "stdout_markers": {},
                "stderr": normalize_output(command_result.stderr, pairing_home),
            }
            if argv == ["pairing", "list"]:
                case["stdout"] = ""
                case["stdout_markers"] = {
                    marker: marker in normalized_stdout
                    for marker in [
                        "Pending Pairing Requests",
                        "Approved Users",
                        "telegram",
                        "U123",
                        "Ada",
                    ]
                }
            pairing_commands.append(case)
        telegram_approved = json.loads(
            (pairing_dir / "telegram-approved.json").read_text(encoding="utf-8")
        )
        telegram_pending = json.loads(
            (pairing_dir / "telegram-pending.json").read_text(encoding="utf-8")
        )
        discord_pending = json.loads(
            (pairing_dir / "discord-pending.json").read_text(encoding="utf-8")
        )
        slack_approved = json.loads(
            (pairing_dir / "slack-approved.json").read_text(encoding="utf-8")
        )
        rate_limits = json.loads(
            (pairing_dir / "_rate_limits.json").read_text(encoding="utf-8")
        )
        pairing_state = {
            "telegram_approved_user_name": (
                telegram_approved.get("U123") or {}
            ).get("user_name"),
            "telegram_pending_empty": telegram_pending == {},
            "discord_pending_empty": discord_pending == {},
            "slack_approved_empty": slack_approved == {},
            "telegram_failure_count": rate_limits.get("_failures:telegram"),
        }

        slack_home = home / "slack-command-home"
        slack_env = env.copy()
        slack_env["HERMES_HOME"] = str(slack_home)
        slack_home.mkdir(parents=True, exist_ok=True)
        (slack_home / ".env").write_text("", encoding="utf-8")
        slack_commands = []
        slack_write_path = slack_home / "slack-parity-manifest.json"
        for argv in [
            ["slack", "manifest", "--slashes-only"],
            [
                "slack",
                "manifest",
                "--name",
                "ParityHermes",
                "--description",
                "Parity Slack manifest",
            ],
            ["slack", "manifest", "--write", str(slack_write_path), "--slashes-only"],
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=30,
                env=slack_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, slack_home)
            normalized_stderr = normalize_output(command_result.stderr, slack_home)
            case = {
                "argv": [
                    part.replace(str(slack_home), "<HERMES_HOME>")
                    for part in ["hermes", *argv]
                ],
                "exit_code": command_result.returncode,
                "stdout": "",
                "stderr": normalized_stderr,
                "stdout_markers": {},
            }
            if argv[0:3] == ["slack", "manifest", "--write"]:
                case["stderr_markers"] = {
                    marker: marker in normalized_stderr
                    for marker in [
                        "Slack manifest written to:",
                        "Next steps:",
                        "<HERMES_HOME>/slack-parity-manifest.json",
                    ]
                }
                payload = json.loads(slack_write_path.read_text(encoding="utf-8"))
                commands = [entry["command"] for entry in payload]
                case["summary"] = {
                    "count": len(payload),
                    "first_command": commands[0],
                    "contains": {
                        name: name in commands
                        for name in ["/hermes", "/background", "/btw"]
                    },
                }
            elif argv[-1] == "--slashes-only":
                payload = json.loads(normalized_stdout)
                commands = [entry["command"] for entry in payload]
                case["summary"] = {
                    "count": len(payload),
                    "first_command": commands[0],
                    "contains": {
                        name: name in commands
                        for name in [
                            "/hermes",
                            "/background",
                            "/btw",
                            "/model",
                            "/status",
                            "/topic",
                            "/reload-mcp",
                        ]
                    },
                    "all_urls": sorted({entry.get("url") for entry in payload}),
                    "should_escape_values": sorted(
                        {entry.get("should_escape") for entry in payload}
                    ),
                }
            elif argv[0:2] == ["slack", "manifest"]:
                payload = json.loads(normalized_stdout)
                case["summary"] = {
                    "display_name": payload["display_information"]["name"],
                    "display_description": payload["display_information"][
                        "description"
                    ],
                    "bot_display_name": payload["features"]["bot_user"][
                        "display_name"
                    ],
                    "slash_count": len(payload["features"]["slash_commands"]),
                    "socket_mode_enabled": payload["settings"][
                        "socket_mode_enabled"
                    ],
                    "bot_scopes_contains_commands": "commands"
                    in payload["oauth_config"]["scopes"]["bot"],
                }
            slack_commands.append(case)

        backup_home = home / "backup-command-home"
        backup_env = env.copy()
        backup_env["HERMES_HOME"] = str(backup_home)
        backup_home.mkdir(parents=True, exist_ok=True)
        (backup_home / "config.yaml").write_text(
            "model:\n  provider: parity\n", encoding="utf-8"
        )
        (backup_home / ".env").write_text(
            "OPENROUTER_API_KEY=sk-fake-parity\n", encoding="utf-8"
        )
        (backup_home / "auth.json").write_text(
            json.dumps({"providers": {"openrouter": {"api_key": "sk-fake"}}}),
            encoding="utf-8",
        )
        (backup_home / "cron").mkdir(parents=True, exist_ok=True)
        (backup_home / "cron" / "jobs.json").write_text(
            json.dumps({"jobs": []}), encoding="utf-8"
        )
        (backup_home / "gateway_state.json").write_text(
            json.dumps({"running": False}), encoding="utf-8"
        )
        (backup_home / "channel_directory.json").write_text(
            json.dumps({"channels": {}}), encoding="utf-8"
        )
        (backup_home / "processes.json").write_text(
            json.dumps({"gateway": {"pid": 123}}), encoding="utf-8"
        )
        pairing_snapshot_dir = backup_home / "pairing"
        pairing_snapshot_dir.mkdir(parents=True, exist_ok=True)
        (pairing_snapshot_dir / "telegram-approved.json").write_text(
            json.dumps({"U123": {"user_name": "Ada"}}), encoding="utf-8"
        )
        platform_pairing_dir = backup_home / "platforms" / "pairing"
        platform_pairing_dir.mkdir(parents=True, exist_ok=True)
        (platform_pairing_dir / "discord-pending.json").write_text(
            json.dumps({"DISC1234": {"user_name": "Dee"}}), encoding="utf-8"
        )
        (backup_home / "feishu_comment_pairing.json").write_text(
            json.dumps({"tenant": "fake"}), encoding="utf-8"
        )
        sqlite_conn = sqlite3.connect(backup_home / "state.db")
        sqlite_conn.execute("CREATE TABLE parity (id INTEGER PRIMARY KEY, value TEXT)")
        sqlite_conn.execute("INSERT INTO parity(value) VALUES ('session')")
        sqlite_conn.commit()
        sqlite_conn.close()
        (backup_home / "memories").mkdir(parents=True, exist_ok=True)
        (backup_home / "memories" / "MEMORY.md").write_text(
            "Remember backup.\n", encoding="utf-8"
        )
        (backup_home / "skills" / "demo").mkdir(parents=True, exist_ok=True)
        (backup_home / "skills" / "demo" / "SKILL.md").write_text(
            "---\nname: demo\n---\n", encoding="utf-8"
        )
        (backup_home / "hermes-agent").mkdir(parents=True, exist_ok=True)
        (backup_home / "hermes-agent" / "run_agent.py").write_text(
            "excluded\n", encoding="utf-8"
        )
        (backup_home / "backups").mkdir(parents=True, exist_ok=True)
        (backup_home / "backups" / "old.zip").write_text("excluded\n", encoding="utf-8")
        (backup_home / "checkpoints" / "s1").mkdir(parents=True, exist_ok=True)
        (backup_home / "checkpoints" / "s1" / "checkpoint.json").write_text(
            "excluded\n", encoding="utf-8"
        )
        (backup_home / "__pycache__").mkdir(parents=True, exist_ok=True)
        (backup_home / "__pycache__" / "module.pyc").write_bytes(b"excluded")
        (backup_home / "gateway.pid").write_text("123\n", encoding="utf-8")
        (backup_home / "state.db-wal").write_text("excluded\n", encoding="utf-8")
        backup_commands = []
        full_backup_path = backup_home / "full-backup.zip"
        for argv in [
            ["backup", "-o", str(full_backup_path)],
            ["backup", "--quick", "--label", "parity"],
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=30,
                env=backup_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, backup_home)
            case = {
                "argv": [
                    part.replace(str(backup_home), "<HERMES_HOME>")
                    for part in ["hermes", *argv]
                ],
                "exit_code": command_result.returncode,
                "stdout": "",
                "stdout_markers": {},
                "stderr": normalize_output(command_result.stderr, backup_home),
            }
            if argv[0:2] == ["backup", "-o"]:
                case["stdout_markers"] = {
                    marker: marker in normalized_stdout
                    for marker in [
                        "Scanning <HERMES_HOME> ...",
                        "Backing up",
                        "Backup complete: <HERMES_HOME>/full-backup.zip",
                        "Restore with: hermes import full-backup.zip",
                        "Excluded directories:",
                    ]
                }
                with zipfile.ZipFile(full_backup_path, "r") as zf:
                    case["zip_members"] = sorted(zf.namelist())
            else:
                case["stdout_markers"] = {
                    marker: marker in normalized_stdout
                    for marker in [
                        "State snapshot created:",
                        "<HERMES_HOME>/state-snapshots/",
                        "Restore with: /snapshot restore",
                    ]
                }
            backup_commands.append(case)
        backup_state = quick_snapshot_state(backup_home)

        import_home = home / "backup-import-home"
        import_env = env.copy()
        import_env["HERMES_HOME"] = str(import_home)
        import_zip = home / "backup-import-source.zip"
        with zipfile.ZipFile(import_zip, "w") as zf:
            zf.writestr(".hermes/config.yaml", "model: imported\n")
            zf.writestr(".hermes/.env", "OPENROUTER_API_KEY=sk-imported\n")
            zf.writestr(".hermes/auth.json", '{"token": "fake"}')
            zf.writestr(".hermes/state.db", "not-a-real-db")
            zf.writestr(".hermes/memories/MEMORY.md", "Imported memory.\n")
            zf.writestr(".hermes/../escape.txt", "blocked\n")
        import_commands = []
        for argv in [["import", str(import_zip), "--force"]]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=30,
                env=import_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, import_home)
            import_commands.append(
                {
                    "argv": [
                        part.replace(str(import_zip), "<IMPORT_ZIP>")
                        for part in ["hermes", *argv]
                    ],
                    "exit_code": command_result.returncode,
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout
                        for marker in [
                            "Backup contains 6 files",
                            "Target: <HERMES_HOME>",
                            "Detected archive prefix: '.hermes/'",
                            "Import complete:",
                            "path traversal blocked",
                            "Done. Your Hermes configuration has been restored.",
                        ]
                    },
                    "stderr": normalize_output(command_result.stderr, import_home),
                }
            )
        import_state = {
            "config": (import_home / "config.yaml").read_text(encoding="utf-8"),
            "env": (import_home / ".env").read_text(encoding="utf-8"),
            "auth": (import_home / "auth.json").read_text(encoding="utf-8"),
            "state_db": (import_home / "state.db").read_text(encoding="utf-8"),
            "memory": (import_home / "memories" / "MEMORY.md").read_text(
                encoding="utf-8"
            ),
            "escaped_exists": (import_home.parent / "escape.txt").exists(),
            "env_mode": oct(stat.S_IMODE((import_home / ".env").stat().st_mode)),
            "auth_mode": oct(stat.S_IMODE((import_home / "auth.json").stat().st_mode)),
            "state_db_mode": oct(stat.S_IMODE((import_home / "state.db").stat().st_mode)),
        }

        doctor_home = home / "doctor-command-home"
        doctor_env = env.copy()
        doctor_env["HERMES_HOME"] = str(doctor_home)
        doctor_commands = []
        for argv in [
            ["doctor", "--ack", "missing-advisory"],
            ["doctor", "--ack", "shai-hulud-2026-05"],
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=30,
                env=doctor_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, doctor_home)
            doctor_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout
                        for marker in [
                            "Unknown advisory ID: 'missing-advisory'",
                            "Known IDs: shai-hulud-2026-05",
                            "Acknowledged advisory shai-hulud-2026-05",
                        ]
                    },
                    "stderr": normalize_output(command_result.stderr, doctor_home),
                }
            )
        doctor_config = yaml.safe_load(
            (doctor_home / "config.yaml").read_text(encoding="utf-8")
        )
        doctor_state = {
            "acked_advisories": (
                (doctor_config.get("security") or {}).get("acked_advisories") or []
            )
        }

        bundles_home = home / "bundles-command-home"
        bundles_env = env.copy()
        bundles_env["HERMES_HOME"] = str(bundles_home)
        bundles_home.mkdir(parents=True, exist_ok=True)
        bundle_path = bundles_home / "skill-bundles" / "daily-flow.yaml"
        bundles_commands = []
        bundle_state = {}
        for argv, marker_map, state_name in [
            (
                ["bundles", "list"],
                {
                    "No bundles installed yet": True,
                    "hermes bundles create": True,
                    "<HERMES_HOME>/skill-bundles": True,
                },
                None,
            ),
            (
                [
                    "bundles",
                    "create",
                    "Daily Flow",
                    "--skill",
                    "github/review",
                    "--skill",
                    "mlops/train",
                    "--description",
                    "Daily work",
                    "--instruction",
                    "Prioritize tests.",
                ],
                {
                    "Created bundle:": True,
                    "<HERMES_HOME>/skill-bundles/daily-flow.yaml": True,
                    "Invoke with: /daily-flow": True,
                    "loads 2 skills": True,
                },
                "after_create",
            ),
            (
                ["bundles", "create", "Daily Flow", "--skill", "other"],
                {
                    "Bundle already exists at": True,
                    "Pass --force to overwrite": True,
                },
                None,
            ),
            (
                ["bundles", "show", "daily_flow"],
                {
                    "/daily-flow": True,
                    "Daily Flow": True,
                    "Daily work": True,
                    "Skills (2):": True,
                    "github/review": True,
                    "mlops/train": True,
                    "Instruction:": True,
                    "Prioritize tests.": True,
                },
                None,
            ),
            (
                ["bundles", "reload"],
                {"No changes. 1 bundle(s) loaded.": True},
                None,
            ),
            (
                ["bundles", "delete", "missing"],
                {"No bundle at": True, "missing.yaml": True},
                None,
            ),
            (
                ["bundles", "delete", "daily-flow"],
                {
                    "Deleted bundle:": True,
                    "<HERMES_HOME>/skill-bundles/daily-flow.yaml": True,
                },
                "after_delete",
            ),
            (
                ["bundles", "list"],
                {
                    "No bundles installed yet": True,
                    "hermes bundles create": True,
                    "<HERMES_HOME>/skill-bundles": True,
                },
                None,
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=60,
                env=bundles_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, bundles_home)
            bundles_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(command_result.stderr, bundles_home),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout for marker in marker_map
                    },
                    "expected_markers": marker_map,
                }
            )
            if state_name == "after_create":
                bundle_state["after_create_exists"] = bundle_path.exists()
                bundle_state["after_create_yaml"] = yaml.safe_load(
                    bundle_path.read_text(encoding="utf-8")
                )
            elif state_name == "after_delete":
                bundle_state["after_delete_exists"] = bundle_path.exists()

        fallback_home = home / "fallback-command-home"
        fallback_env = env.copy()
        fallback_env["HERMES_HOME"] = str(fallback_home)
        fallback_home.mkdir(parents=True, exist_ok=True)
        fallback_commands = []
        for argv, marker_map in [
            (
                ["fallback", "list"],
                {
                    "No fallback providers configured.": True,
                    "Add one with:  hermes fallback add": True,
                },
            ),
            (
                ["fallback"],
                {
                    "No fallback providers configured.": True,
                    "Add one with:  hermes fallback add": True,
                },
            ),
            (
                ["fallback", "remove"],
                {"No fallback providers configured": True, "nothing to remove": True},
            ),
            (
                ["fallback", "clear"],
                {"No fallback providers configured": True, "nothing to clear": True},
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=30,
                env=fallback_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, fallback_home)
            fallback_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(command_result.stderr, fallback_home),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout for marker in marker_map
                    },
                    "expected_markers": marker_map,
                }
            )
        (fallback_home / "config.yaml").write_text(
            yaml.safe_dump(
                {
                    "model": {
                        "provider": "openrouter",
                        "default": "nous/hermes",
                    },
                    "fallback_model": {
                        "provider": "legacy",
                        "model": "legacy/model",
                    },
                    "fallback_providers": [
                        {"provider": "openai", "model": "gpt-4o-mini"},
                        {
                            "provider": "local",
                            "model": "llama",
                            "base_url": "http://127.0.0.1:11434/v1",
                        },
                    ],
                },
                sort_keys=True,
            ),
            encoding="utf-8",
        )
        for argv, marker_map in [
            (
                ["fallback", "list"],
                {
                    "Primary:": True,
                    "nous/hermes  (via openrouter)": True,
                    "Fallback chain (2 entries):": True,
                    "1. gpt-4o-mini  (via openai)": True,
                    "2. llama  (via local)  [http://127.0.0.1:11434/v1]": True,
                    "Tried in order": True,
                },
            )
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=30,
                env=fallback_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, fallback_home)
            fallback_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(command_result.stderr, fallback_home),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout for marker in marker_map
                    },
                    "expected_markers": marker_map,
                }
            )
        fallback_state = yaml.safe_load(
            (fallback_home / "config.yaml").read_text(encoding="utf-8")
        )

        curator_home = home / "curator-command-home"
        curator_env = env.copy()
        curator_env["HERMES_HOME"] = str(curator_home)
        curator_home.mkdir(parents=True, exist_ok=True)
        curator_commands = []
        for argv, marker_map in [
            (
                ["curator", "status"],
                {
                    "curator: ENABLED": True,
                    "runs:           0": True,
                    "last run:       never": True,
                    "last summary:   (none)": True,
                    "interval:       every 7d": True,
                    "stale after:    30d unused": True,
                    "archive after:  90d unused": True,
                    "no agent-created skills": True,
                },
            ),
            (["curator", "pause"], {"curator: paused": True}),
            (
                ["curator", "status"],
                {
                    "curator: PAUSED": True,
                    "runs:           0": True,
                    "last run:       never": True,
                    "no agent-created skills": True,
                },
            ),
            (["curator", "resume"], {"curator: resumed": True}),
            (
                ["curator", "list-archived"],
                {"curator: no archived skills": True},
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=30,
                env=curator_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, curator_home)
            curator_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(command_result.stderr, curator_home),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout for marker in marker_map
                    },
                    "expected_markers": marker_map,
                }
            )
        curator_state_path = curator_home / "skills" / ".curator_state"
        curator_state = (
            json.loads(curator_state_path.read_text(encoding="utf-8"))
            if curator_state_path.exists()
            else {}
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
        session_file_export = home / "cli-session-export.jsonl"
        for argv in [
            ["sessions", "list", "--limit", "5"],
            ["sessions", "export", "-"],
            ["sessions", "stats"],
            ["sessions", "export", "-", "--session-id", "cli-session-1"],
            ["sessions", "export", "-", "--session-id", "missing"],
            ["sessions", "export", str(session_file_export), "--source", "cli"],
            ["sessions", "rename", "cli-session-1", "Renamed", "Session"],
            ["sessions", "rename", "missing-session", "Missing", "Title"],
            ["sessions", "delete", "telegram-session-1", "--yes"],
            ["sessions", "delete", "missing-session", "-y"],
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
                "argv": [
                    part.replace(str(home), "<HERMES_HOME>")
                    for part in ["hermes", *argv]
                ],
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
            elif argv == ["sessions", "export", "-", "--session-id", "missing"]:
                case["stdout"] = ""
                case["stdout_markers"] = {
                    "Session 'missing' not found.": "Session 'missing' not found."
                    in normalized_stdout
                }
            elif argv[:3] == ["sessions", "export", "-"]:
                import json

                case["stdout"] = ""
                case["export"] = normalize_timestamps(json.loads(normalized_stdout))
            elif argv[0:2] == ["sessions", "rename"] and argv[2] == "missing-session":
                case["stdout"] = ""
                case["stdout_markers"] = {
                    "Session 'missing-session' not found.": "Session 'missing-session' not found."
                    in normalized_stdout
                }
            elif argv[0:2] == ["sessions", "delete"] and argv[2] == "missing-session":
                case["stdout"] = ""
                case["stdout_markers"] = {
                    "Session 'missing-session' not found.": "Session 'missing-session' not found."
                    in normalized_stdout
                }
            session_commands.append(case)

        final_db = SessionDB(home / "state.db")
        session_state = {
            "session_count": final_db.session_count(),
            "message_count": final_db.message_count(),
            "renamed_title": final_db.get_session_title("cli-session-1"),
            "deleted_session": final_db.get_session("telegram-session-1"),
            "file_export_lines": [
                normalize_timestamps(json.loads(line))
                for line in session_file_export.read_text(encoding="utf-8").splitlines()
                if line.strip()
            ],
        }
        final_db.close()

        ambiguous_home = home / "session-ambiguous-home"
        ambiguous_env = env.copy()
        ambiguous_env["HERMES_HOME"] = str(ambiguous_home)
        ambiguous_home.mkdir(parents=True, exist_ok=True)
        ambiguous_db = SessionDB(ambiguous_home / "state.db")
        for session_id, content in [
            ("abc111-session", "first ambiguous"),
            ("abc222-session", "second ambiguous"),
        ]:
            ambiguous_db.create_session(
                session_id,
                "cli",
                user_id="ambiguous-user",
                model="fake/model",
                model_config={"provider": "fake"},
                system_prompt="system",
            )
            ambiguous_db.append_message(session_id, "user", content)
        ambiguous_db.close()
        ambiguous_commands = []
        for argv in [
            ["sessions", "export", "-", "--session-id", "abc"],
            ["sessions", "rename", "abc", "Ambiguous", "Name"],
            ["sessions", "delete", "abc", "--yes"],
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=30,
                env=ambiguous_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, ambiguous_home)
            ambiguous_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(command_result.stderr, ambiguous_home),
                    "stdout": "",
                    "stdout_markers": {
                        "Session 'abc' not found.": "Session 'abc' not found."
                        in normalized_stdout
                    },
                }
            )
        ambiguous_final_db = SessionDB(ambiguous_home / "state.db")
        ambiguous_state = {
            "session_count": ambiguous_final_db.session_count(),
            "message_count": ambiguous_final_db.message_count(),
            "first_title": ambiguous_final_db.get_session_title("abc111-session"),
            "second_title": ambiguous_final_db.get_session_title("abc222-session"),
            "first_exists": ambiguous_final_db.get_session("abc111-session") is not None,
            "second_exists": ambiguous_final_db.get_session("abc222-session") is not None,
        }
        ambiguous_final_db.close()

        title_conflict_home = home / "session-title-conflict-home"
        title_conflict_env = env.copy()
        title_conflict_env["HERMES_HOME"] = str(title_conflict_home)
        title_conflict_home.mkdir(parents=True, exist_ok=True)
        title_conflict_db = SessionDB(title_conflict_home / "state.db")
        for session_id, content in [
            ("session-one", "first title conflict"),
            ("session-two", "second title conflict"),
        ]:
            title_conflict_db.create_session(
                session_id,
                "cli",
                user_id="title-user",
                model="fake/model",
                model_config={"provider": "fake"},
                system_prompt="system",
            )
            title_conflict_db.append_message(session_id, "user", content)
        title_conflict_db.set_session_title("session-one", "Existing Title")
        title_conflict_db.close()
        title_conflict_commands = []
        for argv, marker_list in [
            (
                ["sessions", "rename", "session-two", "Existing", "Title"],
                ["Error: Title 'Existing Title' is already in use by session session-one"],
            ),
            (
                ["sessions", "rename", "session-two", "Fresh", "Title"],
                ["Session 'session-two' renamed to: Fresh Title"],
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=30,
                env=title_conflict_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, title_conflict_home)
            title_conflict_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(
                        command_result.stderr, title_conflict_home
                    ),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout for marker in marker_list
                    },
                }
            )
        title_conflict_final_db = SessionDB(title_conflict_home / "state.db")
        title_conflict_state = {
            "session_count": title_conflict_final_db.session_count(),
            "message_count": title_conflict_final_db.message_count(),
            "first_title": title_conflict_final_db.get_session_title("session-one"),
            "second_title": title_conflict_final_db.get_session_title("session-two"),
        }
        title_conflict_final_db.close()

        prune_home = home / "session-prune-home"
        prune_env = env.copy()
        prune_env["HERMES_HOME"] = str(prune_home)
        prune_home.mkdir(parents=True, exist_ok=True)
        prune_sessions_dir = prune_home / "sessions"
        prune_sessions_dir.mkdir(parents=True, exist_ok=True)
        prune_db = SessionDB(prune_home / "state.db")
        for session_id, source, content in [
            ("old-ended-cli", "cli", "old cli"),
            ("old-active-cli", "cli", "active cli"),
            ("recent-ended-cli", "cli", "recent cli"),
            ("old-ended-telegram", "telegram", "old telegram"),
        ]:
            prune_db.create_session(
                session_id,
                source,
                user_id=f"user-{source}",
                model="fake/model",
                model_config={"provider": "fake"},
                system_prompt="system",
            )
            prune_db.append_message(session_id, "user", content)
            (prune_sessions_dir / f"{session_id}.jsonl").write_text(
                f"{content}\n", encoding="utf-8"
            )
            (prune_sessions_dir / f"request_dump_{session_id}_001.json").write_text(
                "{}", encoding="utf-8"
            )
        prune_db.close()

        now = time.time()
        old = now - 120 * 86400
        recent = now - 5 * 86400
        with sqlite3.connect(prune_home / "state.db") as conn:
            conn.execute(
                "UPDATE sessions SET started_at = ?, ended_at = ? WHERE id = ?",
                (old, old + 60, "old-ended-cli"),
            )
            conn.execute(
                "UPDATE sessions SET started_at = ?, ended_at = NULL WHERE id = ?",
                (old, "old-active-cli"),
            )
            conn.execute(
                "UPDATE sessions SET started_at = ?, ended_at = ? WHERE id = ?",
                (recent, recent + 60, "recent-ended-cli"),
            )
            conn.execute(
                "UPDATE sessions SET started_at = ?, ended_at = ? WHERE id = ?",
                (old, old + 60, "old-ended-telegram"),
            )

        prune_commands = []
        prune_states = {}
        for argv, marker_list, state_name in [
            (
                ["sessions", "prune", "--older-than", "90", "--source", "cli", "--yes"],
                ["Pruned 1 session(s)."],
                "after_source_prune",
            ),
            (
                ["sessions", "prune", "--older-than", "90", "--yes"],
                ["Pruned 1 session(s)."],
                "after_all_prune",
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=60,
                env=prune_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, prune_home)
            prune_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(command_result.stderr, prune_home),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout for marker in marker_list
                    },
                }
            )
            state_db = SessionDB(prune_home / "state.db")
            with sqlite3.connect(prune_home / "state.db") as conn:
                remaining_ids = [
                    row[0]
                    for row in conn.execute("SELECT id FROM sessions ORDER BY id")
                ]
            prune_states[state_name] = {
                "session_count": state_db.session_count(),
                "message_count": state_db.message_count(),
                "remaining_ids": remaining_ids,
                "old_ended_cli_file_exists": (
                    prune_sessions_dir / "old-ended-cli.jsonl"
                ).exists(),
                "old_ended_cli_dump_exists": any(
                    path.name.startswith("request_dump_old-ended-cli_")
                    for path in prune_sessions_dir.glob("request_dump_old-ended-cli_*.json")
                ),
                "old_active_cli_file_exists": (
                    prune_sessions_dir / "old-active-cli.jsonl"
                ).exists(),
                "recent_ended_cli_file_exists": (
                    prune_sessions_dir / "recent-ended-cli.jsonl"
                ).exists(),
                "old_ended_telegram_file_exists": (
                    prune_sessions_dir / "old-ended-telegram.jsonl"
                ).exists(),
            }
            state_db.close()

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
                [
                    "profile",
                    "create",
                    "research",
                    "--no-alias",
                    "--no-skills",
                    "--description",
                    "Duplicate work",
                ],
                ["Profile 'research' already exists at <HERMES_HOME>/profiles/research"],
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
                ["profile", "show", "missing"],
                ["Profile 'missing' does not exist."],
            ),
            (
                ["profile", "describe", "missing"],
                [],
            ),
            (
                ["profile", "describe", "missing", "--text", "Nope"],
                [],
            ),
            (
                ["profile", "use", "research"],
                ["Switched to: research"],
            ),
            (
                ["profile", "use", "missing"],
                [
                    "Profile 'missing' does not exist. Create it with: hermes profile create missing"
                ],
            ),
            (
                ["profile", "list"],
                ["Profile", "default", "research", "stopped"],
            ),
            (
                ["profile", "use", "default"],
                ["Switched to: default (~/.hermes)"],
            ),
            (
                ["profile", "use", "research"],
                ["Switched to: research"],
            ),
            (
                ["profile", "delete", "default", "--yes"],
                [
                    "Cannot delete the default profile (~/.hermes).",
                    "hermes uninstall",
                ],
            ),
            (
                ["profile", "delete", "missing", "--yes"],
                ["Profile 'missing' does not exist."],
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

        rename_home = home / "profile-rename-home"
        rename_env = env.copy()
        rename_env["HERMES_HOME"] = str(rename_home)
        rename_commands = []
        for argv, marker_list in [
            (
                ["profile", "rename", "default", "renamed-default"],
                ["Cannot rename the default profile."],
            ),
            (
                [
                    "profile",
                    "create",
                    "rename-me",
                    "--no-alias",
                    "--no-skills",
                    "--description",
                    "Rename role",
                ],
                ["Profile 'rename-me' created", "No bundled skills seeded"],
            ),
            (
                [
                    "profile",
                    "create",
                    "target",
                    "--no-alias",
                    "--no-skills",
                    "--description",
                    "Target role",
                ],
                ["Profile 'target' created", "No bundled skills seeded"],
            ),
            (
                ["profile", "rename", "rename-me", "target"],
                ["Profile 'target' already exists."],
            ),
            (
                ["profile", "rename", "target", "default"],
                ["Cannot rename to 'default' — it is reserved."],
            ),
            (["profile", "use", "rename-me"], ["Switched to: rename-me"]),
            (
                ["profile", "rename", "rename-me", "renamed"],
                ["Renamed rename-me", "Profile renamed: rename-me", "renamed"],
            ),
            (
                ["profile", "rename", "missing", "renamed-again"],
                ["Profile 'missing' does not exist."],
            ),
            (
                ["profile", "describe", "renamed"],
                ["Rename role"],
            ),
            (
                ["profile", "show", "renamed"],
                ["Profile: renamed", "SOUL.md: exists"],
            ),
            (
                ["profile", "list"],
                ["Profile", "renamed", "stopped"],
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=60,
                env=rename_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, rename_home)
            rename_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(command_result.stderr, rename_home),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout for marker in marker_list
                    },
                }
            )
        rename_active_path = rename_home / "active_profile"
        renamed_profile = rename_home / "profiles" / "renamed"
        rename_state = {
            "active_profile_file": (
                rename_active_path.read_text(encoding="utf-8").strip()
                if rename_active_path.exists()
                else None
            ),
            "old_exists": (rename_home / "profiles" / "rename-me").exists(),
            "new_exists": renamed_profile.exists(),
            "description": (
                yaml.safe_load((renamed_profile / "profile.yaml").read_text(encoding="utf-8"))
                or {}
            ).get("description"),
            "soul_exists": (renamed_profile / "SOUL.md").exists(),
            "no_bundled_skills_marker_exists": (
                renamed_profile / ".no-bundled-skills"
            ).exists(),
        }

        profile_validation_home = home / "profile-validation-home"
        profile_validation_env = env.copy()
        profile_validation_env["HERMES_HOME"] = str(profile_validation_home)
        profile_validation_commands = []
        for argv, marker_list in [
            (
                [
                    "profile",
                    "create",
                    "BadName",
                    "--no-alias",
                    "--no-skills",
                    "--description",
                    "Bad",
                ],
                ["Profile 'BadName' created", "profiles/badname"],
            ),
            (["profile", "use", "BadName"], ["Switched to: BadName"]),
            (["profile", "show", "BadName"], ["Profile: BadName", "profiles/badname"]),
            (["profile", "describe", "BadName"], ["Bad"]),
            (
                ["profile", "delete", "BadName", "--yes"],
                ["Profile: badname", "Profile 'badname' deleted."],
            ),
            (
                [
                    "profile",
                    "create",
                    "../escape",
                    "--no-alias",
                    "--no-skills",
                    "--description",
                    "Bad",
                ],
                ["Invalid profile name '../escape'"],
            ),
            (
                [
                    "profile",
                    "create",
                    "../clone-escape",
                    "--clone",
                    "--no-alias",
                    "--description",
                    "Bad",
                ],
                ["Invalid profile name '../clone-escape'"],
            ),
            (
                [
                    "profile",
                    "create",
                    "../clone-all-escape",
                    "--clone-all",
                    "--no-alias",
                    "--description",
                    "Bad",
                ],
                ["Invalid profile name '../clone-all-escape'"],
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=60,
                env=profile_validation_env,
            )
            normalized_stdout = normalize_output(
                command_result.stdout, profile_validation_home
            )
            profile_validation_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(
                        command_result.stderr, profile_validation_home
                    ),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout for marker in marker_list
                    },
                }
            )
        profile_validation_active = profile_validation_home / "active_profile"
        profile_validation_state = {
            "active_profile_file": (
                profile_validation_active.read_text(encoding="utf-8").strip()
                if profile_validation_active.exists()
                else None
            ),
            "badname_exists": (profile_validation_home / "profiles" / "badname").exists(),
            "raw_badname_exists": (
                profile_validation_home / "profiles" / "BadName"
            ).exists(),
            "escape_exists": (profile_validation_home.parent / "escape").exists(),
            "clone_escape_exists": (
                profile_validation_home.parent / "clone-escape"
            ).exists(),
            "clone_all_escape_exists": (
                profile_validation_home.parent / "clone-all-escape"
            ).exists(),
        }

        clone_home = home / "profile-clone-home"
        clone_env = env.copy()
        clone_env["HERMES_HOME"] = str(clone_home)
        (clone_home / "skills" / "demo").mkdir(parents=True, exist_ok=True)
        (clone_home / "memories").mkdir(parents=True, exist_ok=True)
        (clone_home / "config.yaml").write_text("model: clone-model\n", encoding="utf-8")
        (clone_home / ".env").write_text("OPENROUTER_API_KEY=sk-clone\n", encoding="utf-8")
        (clone_home / "SOUL.md").write_text("Clone soul.\n", encoding="utf-8")
        (clone_home / "skills" / "demo" / "SKILL.md").write_text(
            "# Demo Clone Skill\n", encoding="utf-8"
        )
        (clone_home / "memories" / "MEMORY.md").write_text(
            "Remember clone.\n", encoding="utf-8"
        )
        (clone_home / "memories" / "USER.md").write_text("User clone.\n", encoding="utf-8")
        clone_commands = []
        for argv, marker_list in [
            (
                [
                    "profile",
                    "create",
                    "cloned",
                    "--clone",
                    "--no-alias",
                    "--description",
                    "Cloned role",
                ],
                [
                    "Profile 'cloned' created",
                    "Cloned config, .env, SOUL.md, and skills from default.",
                    "Next steps:",
                    "cloned setup",
                ],
            ),
            (
                [
                    "profile",
                    "create",
                    "cloned",
                    "--clone",
                    "--no-alias",
                    "--description",
                    "Duplicate clone",
                ],
                ["Profile 'cloned' already exists at <HERMES_HOME>/profiles/cloned"],
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=60,
                env=clone_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, clone_home)
            clone_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(command_result.stderr, clone_home),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout for marker in marker_list
                    },
                }
            )
        cloned_profile = clone_home / "profiles" / "cloned"
        clone_state = {
            "cloned_exists": cloned_profile.exists(),
            "config": (cloned_profile / "config.yaml").read_text(encoding="utf-8"),
            "env": (cloned_profile / ".env").read_text(encoding="utf-8"),
            "soul": (cloned_profile / "SOUL.md").read_text(encoding="utf-8"),
            "memory": (cloned_profile / "memories" / "MEMORY.md").read_text(
                encoding="utf-8"
            ),
            "user": (cloned_profile / "memories" / "USER.md").read_text(encoding="utf-8"),
            "skill_exists": (cloned_profile / "skills" / "demo" / "SKILL.md").exists(),
            "description": (
                yaml.safe_load((cloned_profile / "profile.yaml").read_text(encoding="utf-8"))
                or {}
            ).get("description"),
            "no_bundled_skills_marker_exists": (
                cloned_profile / ".no-bundled-skills"
            ).exists(),
        }

        clone_all_home = home / "profile-clone-all-home"
        clone_all_env = env.copy()
        clone_all_env["HERMES_HOME"] = str(clone_all_home)
        (clone_all_home / "skills" / "demo").mkdir(parents=True, exist_ok=True)
        (clone_all_home / "memories").mkdir(parents=True, exist_ok=True)
        (clone_all_home / "sessions").mkdir(parents=True, exist_ok=True)
        (clone_all_home / "profiles" / "sibling").mkdir(parents=True, exist_ok=True)
        (clone_all_home / "config.yaml").write_text("model: clone-all-model\n", encoding="utf-8")
        (clone_all_home / ".env").write_text(
            "OPENROUTER_API_KEY=sk-clone-all\n", encoding="utf-8"
        )
        (clone_all_home / "SOUL.md").write_text("Clone all soul.\n", encoding="utf-8")
        (clone_all_home / "skills" / "demo" / "SKILL.md").write_text(
            "# Demo Clone All Skill\n", encoding="utf-8"
        )
        (clone_all_home / "memories" / "MEMORY.md").write_text(
            "Remember clone all.\n", encoding="utf-8"
        )
        (clone_all_home / "sessions" / "session.jsonl").write_text(
            "{}\n", encoding="utf-8"
        )
        (clone_all_home / "gateway.pid").write_text("12345\n", encoding="utf-8")
        (clone_all_home / "gateway_state.json").write_text("{}\n", encoding="utf-8")
        (clone_all_home / "processes.json").write_text("{}\n", encoding="utf-8")
        clone_all_commands = []
        for argv, marker_list in [
            (
                [
                    "profile",
                    "create",
                    "fullcopy",
                    "--clone-all",
                    "--no-alias",
                    "--description",
                    "Full role",
                ],
                [
                    "Profile 'fullcopy' created",
                    "Full copy from default.",
                    "Next steps:",
                    "fullcopy setup",
                ],
            ),
            (
                [
                    "profile",
                    "create",
                    "fullcopy",
                    "--clone-all",
                    "--no-alias",
                    "--description",
                    "Duplicate full",
                ],
                ["Profile 'fullcopy' already exists at <HERMES_HOME>/profiles/fullcopy"],
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=60,
                env=clone_all_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, clone_all_home)
            clone_all_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(command_result.stderr, clone_all_home),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout for marker in marker_list
                    },
                }
            )
        fullcopy_profile = clone_all_home / "profiles" / "fullcopy"
        clone_all_state = {
            "fullcopy_exists": fullcopy_profile.exists(),
            "config": (fullcopy_profile / "config.yaml").read_text(encoding="utf-8"),
            "env": (fullcopy_profile / ".env").read_text(encoding="utf-8"),
            "soul": (fullcopy_profile / "SOUL.md").read_text(encoding="utf-8"),
            "memory": (fullcopy_profile / "memories" / "MEMORY.md").read_text(
                encoding="utf-8"
            ),
            "session_exists": (fullcopy_profile / "sessions" / "session.jsonl").exists(),
            "skill_exists": (fullcopy_profile / "skills" / "demo" / "SKILL.md").exists(),
            "nested_profiles_exists": (fullcopy_profile / "profiles").exists(),
            "gateway_pid_exists": (fullcopy_profile / "gateway.pid").exists(),
            "gateway_state_exists": (fullcopy_profile / "gateway_state.json").exists(),
            "processes_exists": (fullcopy_profile / "processes.json").exists(),
            "description": (
                yaml.safe_load((fullcopy_profile / "profile.yaml").read_text(encoding="utf-8"))
                or {}
            ).get("description"),
        }

        logs_home = home / "logs-command-home"
        logs_env = env.copy()
        logs_env["HERMES_HOME"] = str(logs_home)
        logs_dir = logs_home / "logs"
        logs_dir.mkdir(parents=True, exist_ok=True)
        (logs_dir / "agent.log").write_text(
            "\n".join(
                [
                    "2026-05-20 10:00:00,000 INFO [sessA] hermes_cli.main: boot",
                    "2026-05-20 10:01:00,000 WARNING [sessA] tools.terminal_tool: tool warn",
                    "2026-05-20 10:02:00,000 ERROR [sessB] gateway.run: gateway error",
                ]
            )
            + "\n",
            encoding="utf-8",
        )
        (logs_dir / "errors.log").write_text(
            "2026-05-20 10:03:00,000 ERROR [sessC] run_agent: failure\n",
            encoding="utf-8",
        )
        (logs_dir / "gateway.log").write_text(
            "2026-05-20 10:04:00,000 INFO [sessG] gateway.run: ready\n",
            encoding="utf-8",
        )
        logs_commands = []
        for argv, marker_list in [
            (
                ["logs", "list"],
                ["Log files in <HERMES_HOME>/logs/", "agent.log", "errors.log", "gateway.log"],
            ),
            (
                ["logs", "agent", "-n", "2"],
                ["agent.log", "last 2", "tool warn", "gateway error"],
            ),
            (
                [
                    "logs",
                    "agent",
                    "-n",
                    "5",
                    "--level",
                    "WARNING",
                    "--session",
                    "sessA",
                    "--component",
                    "tools",
                ],
                ["level>=WARNING", "session=sessA", "component=tools", "tool warn"],
            ),
            (
                ["logs", "errors", "-n", "1"],
                ["errors.log", "last 1", "failure"],
            ),
            (
                ["logs", "gateway", "-n", "1"],
                ["gateway.log", "last 1", "ready"],
            ),
            (
                ["logs", "unknown", "-n", "1"],
                ["Unknown log: 'unknown'. Available: agent, errors, gateway"],
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=60,
                env=logs_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, logs_home)
            logs_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(command_result.stderr, logs_home),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout for marker in marker_list
                    },
                }
            )

        logs_missing_home = home / "logs-missing-home"
        logs_missing_env = env.copy()
        logs_missing_env["HERMES_HOME"] = str(logs_missing_home)
        logs_missing_home.mkdir(parents=True, exist_ok=True)
        command_result = subprocess.run(
            [sys.executable, "-m", "hermes_cli.main", "logs", "agent", "-n", "1"],
            text=True,
            capture_output=True,
            timeout=60,
            env=logs_missing_env,
        )
        logs_missing_command = {
            "argv": ["hermes", "logs", "agent", "-n", "1"],
            "exit_code": command_result.returncode,
            "stderr": normalize_output(command_result.stderr, logs_missing_home),
            "stdout": normalize_output(command_result.stdout, logs_missing_home),
            "stdout_markers": {},
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
                [
                    "profile",
                    "import",
                    str(archive_home / "missing.tar.gz"),
                    "--name",
                    "restored",
                ],
                ["Error:", "Archive not found"],
            ),
            (
                ["profile", "import", str(archive_path), "--name", "default"],
                ["Error:", "Cannot import as 'default'"],
            ),
            (
                ["profile", "import", str(archive_path), "--name", "../escape"],
                ["Error:", "Invalid profile name '../escape'"],
            ),
            (
                ["profile", "import", str(archive_path), "--name", "BadName"],
                ["Imported profile 'badname'"],
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
            "badname_exists": (archive_home / "profiles" / "badname").exists(),
            "badname_config": (
                archive_home / "profiles" / "badname" / "config.yaml"
            ).read_text(encoding="utf-8"),
            "escaped_exists": (archive_home.parent / "escape").exists(),
        }

        tool_home = home / "tools-command-home"
        tool_env = env.copy()
        tool_env["HERMES_HOME"] = str(tool_home)
        tool_home.mkdir(parents=True, exist_ok=True)
        (tool_home / "config.yaml").write_text(
            yaml.safe_dump(
                {
                    "mcp_servers": {
                        "demo": {
                            "command": "node",
                            "enabled": True,
                        }
                    }
                },
                sort_keys=True,
            ),
            encoding="utf-8",
        )
        tool_commands = []
        for argv, marker_list in [
            (["tools", "enable", "no-such-toolset"], ["Unknown toolset 'no-such-toolset'"]),
            (
                ["tools", "enable", "web", "no-such-toolset", "video"],
                ["Unknown toolset 'no-such-toolset'", "Enabled: web, video"],
            ),
            (["tools", "enable", "video"], ["Enabled: video"]),
            (
                ["tools", "disable", "browser", "no-such-toolset", "video"],
                ["Unknown toolset 'no-such-toolset'", "Disabled: browser, video"],
            ),
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
            (
                ["tools", "list", "--platform", "telegram"],
                [
                    "Built-in toolsets (telegram):",
                    "✓ enabled  browser",
                    "✓ enabled  web",
                ],
            ),
            (
                ["tools", "disable", "web", "--platform", "telegram"],
                ["Disabled: web"],
            ),
            (
                ["tools", "list", "--platform", "telegram"],
                [
                    "Built-in toolsets (telegram):",
                    "✗ disabled  web",
                    "✓ enabled  browser",
                ],
            ),
            (
                ["tools", "list", "--platform", "bad"],
                ["Unknown platform 'bad'"],
            ),
            (
                ["tools", "disable", "demo:delete_file"],
                ["Disabled: demo:delete_file"],
            ),
            (
                ["tools", "list"],
                ["MCP servers:", "demo", "excluded: delete_file"],
            ),
            (
                ["tools", "enable", "demo:delete_file"],
                ["Enabled: demo:delete_file"],
            ),
            (
                ["tools", "disable", "missing:tool"],
                ["MCP server 'missing' not found in config"],
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
            "unknown_present": "no-such-toolset"
            in ((tool_config.get("platform_toolsets") or {}).get("cli") or []),
            "platform_toolsets_telegram": sorted(
                str(item)
                for item in (
                    (tool_config.get("platform_toolsets") or {}).get("telegram") or []
                )
            ),
            "telegram_web_enabled": "web"
            in ((tool_config.get("platform_toolsets") or {}).get("telegram") or []),
            "telegram_browser_enabled": "browser"
            in ((tool_config.get("platform_toolsets") or {}).get("telegram") or []),
            "demo_exclude": (
                ((tool_config.get("mcp_servers") or {}).get("demo") or {})
                .get("tools", {})
                .get("exclude", [])
            ),
            "missing_server_present": "missing" in (tool_config.get("mcp_servers") or {}),
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
                ["cron", "pause", "missing"],
                ["Failed to pause job: Job with ID or name 'missing' not found."],
                None,
            ),
            (
                ["cron", "resume", "missing"],
                ["Failed to resume job: Job with ID or name 'missing' not found."],
                None,
            ),
            (
                ["cron", "remove", "missing"],
                ["Failed to remove job: Job with ID or name 'missing' not found."],
                "after_missing_remove",
            ),
            (
                ["cron", "list", "--all"],
                ["No scheduled jobs.", "hermes cron create"],
                None,
            ),
            (
                ["cron", "status"],
                [
                    "Gateway is not running",
                    "cron jobs will NOT fire",
                    "No active jobs",
                ],
                None,
            ),
            (
                ["cron", "run", "missing"],
                ["Failed to run job: Job with ID or name 'missing' not found."],
                None,
            ),
            (
                [
                    "cron",
                    "create",
                    "2026-06-03T09:00:00+00:00",
                    "first duplicate",
                    "--name",
                    "duplicate",
                    "--deliver",
                    "local",
                ],
                ["Created job:", "Name: duplicate"],
                None,
            ),
            (
                [
                    "cron",
                    "create",
                    "2026-06-04T09:00:00+00:00",
                    "second duplicate",
                    "--name",
                    "duplicate",
                    "--deliver",
                    "local",
                ],
                ["Created job:", "Name: duplicate"],
                "after_duplicate_create",
            ),
            (
                ["cron", "pause", "duplicate"],
                ["Failed to pause job: Job name 'duplicate' is ambiguous", "matches 2 jobs"],
                "after_ambiguous_pause",
            ),
            (
                ["cron", "resume", "duplicate"],
                ["Failed to resume job: Job name 'duplicate' is ambiguous", "matches 2 jobs"],
                None,
            ),
            (
                ["cron", "remove", "duplicate"],
                ["Failed to remove job: Job name 'duplicate' is ambiguous", "matches 2 jobs"],
                "after_ambiguous_remove",
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

        mcp_home = home / "mcp-command-home"
        mcp_env = env.copy()
        mcp_env["HERMES_HOME"] = str(mcp_home)
        mcp_home.mkdir(parents=True, exist_ok=True)
        (mcp_home / "config.yaml").write_text(
            yaml.safe_dump(
                {
                    "mcp_servers": {
                        "remote-demo": {
                            "url": "https://example.com/mcp",
                            "enabled": True,
                            "tools": {"include": ["search", "read_file"]},
                        },
                        "local-demo": {
                            "command": "npx",
                            "args": [
                                "@modelcontextprotocol/server-filesystem",
                                "/tmp/demo",
                            ],
                            "env": {"DEMO_TOKEN": "fake-token"},
                            "enabled": False,
                            "tools": {"exclude": ["delete"]},
                        },
                    },
                    "model": {"name": "preserved-model"},
                },
                sort_keys=True,
            ),
            encoding="utf-8",
        )
        mcp_commands = []
        for argv, marker_map in [
            (
                ["mcp", "list"],
                {
                    "MCP Servers": True,
                    "remote-demo": True,
                    "https://example.com/mcp": True,
                    "2 selected": True,
                    "enabled": True,
                    "local-demo": True,
                    "npx @modelcontext": True,
                    "-1 excluded": True,
                    "disabled": True,
                },
            ),
            (
                ["mcp", "ls"],
                {
                    "MCP Servers": True,
                    "remote-demo": True,
                    "https://example.com/mcp": True,
                    "2 selected": True,
                    "enabled": True,
                    "local-demo": True,
                    "npx @modelcontext": True,
                    "-1 excluded": True,
                    "disabled": True,
                },
            ),
            (
                ["mcp", "rm", "remote-demo"],
                {"Removed 'remote-demo' from config": True},
            ),
            (
                ["mcp", "list"],
                {
                    "local-demo": True,
                    "remote-demo": False,
                    "preserved-model": False,
                },
            ),
            (
                ["mcp", "remove", "missing"],
                {"Server 'missing' not found in config.": True},
            ),
            (
                ["mcp", "add", "missing-transport"],
                {"Must specify --url <endpoint>, --command <cmd>, or --preset <name>": True},
            ),
            (
                ["mcp", "add", "bad-env", "--command", "node", "--env", "not-an-assignment"],
                {"Invalid --env value 'not-an-assignment'": True},
            ),
            (
                ["mcp", "test", "missing"],
                {
                    "Server 'missing' not found in config.": True,
                    "Available: local-demo": True,
                },
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=60,
                env=mcp_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, mcp_home)
            mcp_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(command_result.stderr, mcp_home),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout
                        for marker in marker_map
                    },
                    "expected_markers": marker_map,
                }
            )
        mcp_config = yaml.safe_load((mcp_home / "config.yaml").read_text(encoding="utf-8")) or {}
        mcp_state = {
            "server_names": sorted((mcp_config.get("mcp_servers") or {}).keys()),
            "remote_present": "remote-demo" in (mcp_config.get("mcp_servers") or {}),
            "local_config": (mcp_config.get("mcp_servers") or {}).get("local-demo"),
            "model_preserved": ((mcp_config.get("model") or {}).get("name")),
        }

        gateway_home = home / "gateway-command-home"
        gateway_env = env.copy()
        gateway_env["HERMES_HOME"] = str(gateway_home)
        gateway_home.mkdir(parents=True, exist_ok=True)
        messenger_profile = gateway_home / "profiles" / "messenger"
        messenger_profile.mkdir(parents=True, exist_ok=True)
        (messenger_profile / "profile.yaml").write_text(
            "description: Messenger role\n", encoding="utf-8"
        )
        gateway_commands = []
        for argv, marker_map in [
            (
                ["gateway", "status"],
                {
                    "Gateway is not running": True,
                    "To start:": True,
                    "hermes gateway run": True,
                    "hermes gateway install": True,
                },
            ),
            (
                ["gateway", "list"],
                {
                    "Gateways:": True,
                    "default (current)": True,
                    "messenger": True,
                },
            ),
            (
                ["gateway", "stop"],
                {
                    "No gateway running for this profile": True,
                },
            ),
            (
                ["gateway", "start"],
                {
                    "Service start is not applicable inside a Docker container.": True,
                    "docker start <container>": True,
                    "hermes gateway run": True,
                },
            ),
            (
                ["gateway", "install"],
                {
                    "Service installation is not needed inside a Docker container.": True,
                    "Docker restart policies": True,
                    "hermes gateway run": True,
                },
            ),
            (
                ["gateway", "uninstall"],
                {
                    "Service uninstall is not applicable inside a Docker container.": True,
                    "docker stop <container>": True,
                    "docker rm <container>": True,
                },
            ),
        ]:
            command_result = subprocess.run(
                [sys.executable, "-m", "hermes_cli.main", *argv],
                text=True,
                capture_output=True,
                timeout=60,
                env=gateway_env,
            )
            normalized_stdout = normalize_output(command_result.stdout, gateway_home)
            gateway_commands.append(
                {
                    "argv": ["hermes", *argv],
                    "exit_code": command_result.returncode,
                    "stderr": normalize_output(command_result.stderr, gateway_home),
                    "stdout": "",
                    "stdout_markers": {
                        marker: marker in normalized_stdout
                        for marker in marker_map
                    },
                    "expected_markers": marker_map,
                }
            )

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
                "name": "safe_auth_command_execution",
                "commands": auth_commands,
            },
            {
                "name": "safe_memory_command_execution",
                "commands": memory_commands,
                "state": memory_state,
            },
            {
                "name": "safe_pairing_command_execution",
                "commands": pairing_commands,
                "state": pairing_state,
            },
            {
                "name": "safe_slack_manifest_command_execution",
                "commands": slack_commands,
            },
            {
                "name": "safe_backup_command_execution",
                "commands": backup_commands,
                "state": backup_state,
            },
            {
                "name": "safe_backup_import_command_execution",
                "commands": import_commands,
                "state": import_state,
            },
            {
                "name": "safe_doctor_command_execution",
                "commands": doctor_commands,
                "state": doctor_state,
            },
            {
                "name": "safe_bundles_command_execution",
                "commands": bundles_commands,
                "state": bundle_state,
            },
            {
                "name": "safe_fallback_command_execution",
                "commands": fallback_commands,
                "state": fallback_state,
            },
            {
                "name": "safe_curator_command_execution",
                "commands": curator_commands,
                "state": curator_state,
            },
            {
                "name": "safe_session_command_execution",
                "commands": session_commands,
                "state": session_state,
            },
            {
                "name": "safe_session_ambiguous_prefix_command_execution",
                "commands": ambiguous_commands,
                "state": ambiguous_state,
            },
            {
                "name": "safe_session_title_conflict_command_execution",
                "commands": title_conflict_commands,
                "state": title_conflict_state,
            },
            {
                "name": "safe_session_prune_command_execution",
                "commands": prune_commands,
                "states": prune_states,
            },
            {
                "name": "safe_profile_command_execution",
                "commands": profile_commands,
                "state": profile_state,
            },
            {
                "name": "safe_profile_rename_command_execution",
                "commands": rename_commands,
                "state": rename_state,
            },
            {
                "name": "safe_profile_validation_command_execution",
                "commands": profile_validation_commands,
                "state": profile_validation_state,
            },
            {
                "name": "safe_profile_clone_command_execution",
                "commands": clone_commands,
                "state": clone_state,
            },
            {
                "name": "safe_profile_clone_all_command_execution",
                "commands": clone_all_commands,
                "state": clone_all_state,
            },
            {
                "name": "safe_logs_command_execution",
                "commands": logs_commands,
                "missing_file_command": logs_missing_command,
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
            {
                "name": "safe_mcp_command_execution",
                "commands": mcp_commands,
                "state": mcp_state,
            },
            {
                "name": "safe_gateway_command_execution",
                "commands": gateway_commands,
            },
        ]
    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
