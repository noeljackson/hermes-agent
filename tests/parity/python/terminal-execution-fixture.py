from __future__ import annotations

import json
import os

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "terminal-execution-fixture.py"


def parsed(result: str):
    return json.loads(result)


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home():
        for key in list(os.environ):
            if key.startswith("TERMINAL_"):
                os.environ.pop(key, None)
        os.environ["TERMINAL_ENV"] = "local"
        os.environ["TERMINAL_TIMEOUT"] = "5"

        from tools.terminal_tool import terminal_tool

        cases = [
            {
                "name": "local_printf",
                "result": parsed(
                    terminal_tool(
                        command="printf 'hello parity\\n'",
                        task_id="parity-terminal",
                    )
                ),
            },
            {
                "name": "local_nonzero_exit",
                "result": parsed(
                    terminal_tool(
                        command="sh -c 'printf fail; exit 7'",
                        task_id="parity-terminal",
                    )
                ),
            },
            {
                "name": "invalid_command_type",
                "result": parsed(terminal_tool(command=["not", "a", "string"])),
            },
            {
                "name": "foreground_timeout_too_large",
                "result": parsed(
                    terminal_tool(
                        command="printf no-run",
                        timeout=999999,
                        task_id="parity-terminal",
                    )
                ),
            },
        ]

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
