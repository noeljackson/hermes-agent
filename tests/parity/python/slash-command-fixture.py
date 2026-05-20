from __future__ import annotations

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "slash-command-fixture.py"


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home():
        from hermes_cli.commands import (
            ACTIVE_SESSION_BYPASS_COMMANDS,
            COMMAND_REGISTRY,
            GATEWAY_KNOWN_COMMANDS,
            gateway_help_lines,
            resolve_command,
            should_bypass_active_session,
            slack_subcommand_map,
            telegram_bot_commands,
        )

        def resolved_name(value):
            resolved = resolve_command(value)
            return getattr(resolved, "name", resolved)

        commands = []
        for command in COMMAND_REGISTRY:
            commands.append(
                {
                    "name": command.name,
                    "description": command.description,
                    "category": command.category,
                    "aliases": list(command.aliases),
                    "args_hint": command.args_hint,
                    "cli_only": command.cli_only,
                    "gateway_only": command.gateway_only,
                    "gateway_config_gate": command.gateway_config_gate,
                }
            )
        commands.sort(key=lambda item: item["name"])
        cases = [
            {
                "name": "registry_inventory",
                "command_count": len(commands),
                "commands": commands,
            },
            {
                "name": "alias_resolution",
                "aliases": {
                    "h": resolved_name("h"),
                    "help": resolved_name("help"),
                    "q": resolved_name("q"),
                    "quit": resolved_name("quit"),
                },
            },
            {
                "name": "gateway_projection",
                "gateway_known_commands": sorted(GATEWAY_KNOWN_COMMANDS),
                "active_session_bypass_commands": sorted(ACTIVE_SESSION_BYPASS_COMMANDS),
                "bypass_cases": {
                    "background": should_bypass_active_session("background"),
                    "bg": should_bypass_active_session("bg"),
                    "copy": should_bypass_active_session("copy"),
                    "nonexistent": should_bypass_active_session("nonexistent"),
                    "reset": should_bypass_active_session("reset"),
                    "topic": should_bypass_active_session("topic"),
                },
                "gateway_help_lines": gateway_help_lines(),
                "telegram_commands": telegram_bot_commands(),
                "slack_subcommands": slack_subcommand_map(),
            },
        ]
    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
