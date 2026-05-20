from __future__ import annotations

import os

from types import SimpleNamespace

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "mcp-filtering-fixture.py"


def fake_tool(name: str, description: str = ""):
    return SimpleNamespace(
        name=name,
        description=description,
        inputSchema={
            "type": "object",
            "properties": {"query": {"type": "string"}},
            "required": ["query", "missing_property"],
        },
    )


def fake_server(name: str, resources=True, prompts=False):
    capabilities = SimpleNamespace(
        resources=object() if resources else None,
        prompts=object() if prompts else None,
    )
    return SimpleNamespace(
        name=name,
        _tools=[
            fake_tool("search"),
            fake_tool("read-file"),
            fake_tool("dangerous/tool"),
        ],
        tool_timeout=5,
        session=SimpleNamespace(),
        initialize_result=SimpleNamespace(capabilities=capabilities),
    )


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home():
        from tools.mcp_tool import (
            InvalidMcpUrlError,
            _build_safe_env,
            _normalize_mcp_input_schema,
            _register_server_tools,
            _safe_numeric,
            _sanitize_error,
            _validate_remote_mcp_url,
        )

        cases = []
        for name, config in [
            ("all_tools", {}),
            ("include_only", {"tools": {"include": ["search"]}}),
            (
                "include_precedence",
                {"tools": {"include": ["search"], "exclude": ["search"]}},
            ),
            ("include_string", {"tools": {"include": "read-file"}}),
            ("invalid_filters_ignored", {"tools": {"include": 7, "exclude": {"x": 1}}}),
            ("exclude_one", {"tools": {"exclude": ["dangerous/tool"]}}),
            ("disable_utilities", {"tools": {"resources": False, "prompts": False}}),
            ("boolish_false_utilities", {"tools": {"resources": "off", "prompts": "no"}}),
        ]:
            server_name = f"demo-{name}"
            registered = _register_server_tools(server_name, fake_server(server_name), config)
            cases.append(
                {
                    "name": name,
                    "registered": registered,
                }
            )
        prompts_server = fake_server("demo-prompts-only", resources=False, prompts=True)
        cases.append(
            {
                "name": "prompts_only_utilities",
                "registered": _register_server_tools(
                    "demo-prompts-only",
                    prompts_server,
                    {"tools": {"resources": "yes", "prompts": "on"}},
                ),
            }
        )
        cases.append(
            {
                "name": "schema_normalization",
                "schema": _normalize_mcp_input_schema(
                    {
                        "definitions": {
                            "Nested": {
                                "required": ["present", "missing"],
                                "properties": {"present": {"type": "string"}},
                            }
                        },
                        "required": ["query", "missing_property", "optional_note"],
                        "properties": {
                            "query": {"type": "string"},
                            "optional_note": {
                                "anyOf": [{"type": "string"}, {"type": "null"}],
                                "default": None,
                            },
                            "nested": {"$ref": "#/definitions/Nested"},
                        },
                    }
                ),
                "empty_schema": _normalize_mcp_input_schema(None),
            }
        )
        original_env = dict(os.environ)
        try:
            os.environ.clear()
            os.environ.update(
                {
                    "PATH": "/usr/bin",
                    "HOME": "/home/parity",
                    "TERM": "xterm-256color",
                    "XDG_CONFIG_HOME": "/home/parity/.config",
                    "OPENAI_API_KEY": "sk-host-secret",
                    "UNSAFE_TOKEN": "ghp_host_secret",
                }
            )
            safe_env = _build_safe_env(
                {
                    "PATH": "/custom/bin",
                    "CUSTOM_TOKEN": "sk-user-configured",
                    "MCP_ALLOWED": "yes",
                }
            )
        finally:
            os.environ.clear()
            os.environ.update(original_env)
        cases.append(
            {
                "name": "safe_env_filtering",
                "env": dict(sorted(safe_env.items())),
            }
        )
        cases.append(
            {
                "name": "error_redaction",
                "cases": {
                    "github": _sanitize_error("failed with ghp_abc123_TOKEN"),
                    "openai": _sanitize_error("bad sk-test_123"),
                    "bearer": _sanitize_error("Authorization: Bearer secret-token"),
                    "query": _sanitize_error("token=abc123&key=def456"),
                    "env": _sanitize_error("API_KEY=abc password=hunter2"),
                    "clean": _sanitize_error("plain connection failure"),
                },
            }
        )

        url_cases = []
        for name, value in [
            ("valid_http", "http://localhost:8000/mcp"),
            ("valid_https", " https://example.com/mcp?x=1 "),
            ("none", None),
            ("empty", " "),
            ("missing_scheme", "example.com/mcp"),
            ("bad_scheme", "file:///tmp/mcp"),
            ("missing_host", "https:///mcp"),
            ("missing_hostname", "http://:8080/mcp"),
        ]:
            try:
                url_cases.append(
                    {
                        "name": name,
                        "ok": True,
                        "value": _validate_remote_mcp_url("demo", value),
                    }
                )
            except InvalidMcpUrlError as exc:
                url_cases.append(
                    {
                        "name": name,
                        "ok": False,
                        "error": str(exc),
                    }
                )
        cases.append({"name": "remote_url_validation", "cases": url_cases})

        numeric_cases = {}
        for name, value in [
            ("int", 7),
            ("string_int", "8"),
            ("zero_minimum", 0),
            ("negative_minimum", -4),
            ("bad_string", "abc"),
            ("none", None),
            ("float_int_coerce", 2.8),
        ]:
            numeric_cases[name] = _safe_numeric(value, default=5, coerce=int, minimum=2)
        cases.append({"name": "safe_numeric", "cases": numeric_cases})

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
