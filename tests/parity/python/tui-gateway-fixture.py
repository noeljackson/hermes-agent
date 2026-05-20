from __future__ import annotations

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "tui-gateway-fixture.py"


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home():
        import tui_gateway.server as server
        from hermes_cli import web_server

        def normalize_request(req):
            normalized = server._normalize_request(req)
            if isinstance(normalized, dict):
                return {"ok": False, "response": normalized}
            rid, method, params = normalized
            return {"ok": True, "id": rid, "method": method, "params": params}

        def normalize_case(name, request):
            return {
                "name": name,
                "request": request,
                "normalized": normalize_request(request),
            }

        frames = []
        original_write_json = server.write_json
        try:
            server.write_json = lambda obj: frames.append(obj) or True
            server._emit("approval.request", "sid-1", {"prompt": "Allow?", "id": "req-1"})
            server._emit("message.start", "sid-1")
        finally:
            server.write_json = original_write_json

        def resize_case(text: str):
            raw = text.encode("utf-8")
            match = web_server._RESIZE_RE.match(raw)
            if match and match.end() == len(raw):
                return {"cols": int(match.group(1)), "rows": int(match.group(2))}
            return None

        cases = [
            {
                "name": "method_inventory",
                "methods": sorted(server._methods.keys()),
                "long_handlers": sorted(server._LONG_HANDLERS),
            },
            {
                "name": "jsonrpc_frame_helpers",
                "ok": server._ok("rpc-1", {"status": "ok"}),
                "err": server._err("rpc-2", -32000, "handler error"),
                "unknown_method": server.handle_request(
                    {"jsonrpc": "2.0", "id": "rpc-3", "method": "missing.method"}
                ),
            },
            {
                "name": "request_normalization",
                "cases": [
                    normalize_case("non_object", ["not", "an", "object"]),
                    normalize_case("missing_method", {"jsonrpc": "2.0", "id": "rpc-4"}),
                    normalize_case(
                        "empty_method", {"jsonrpc": "2.0", "id": "rpc-5", "method": ""}
                    ),
                    normalize_case(
                        "non_object_params",
                        {
                            "jsonrpc": "2.0",
                            "id": "rpc-6",
                            "method": "session.status",
                            "params": ["bad"],
                        },
                    ),
                    normalize_case(
                        "null_params",
                        {
                            "jsonrpc": "2.0",
                            "id": "rpc-7",
                            "method": "session.status",
                            "params": None,
                        },
                    ),
                    normalize_case(
                        "object_params",
                        {
                            "jsonrpc": "2.0",
                            "id": "rpc-8",
                            "method": "terminal.resize",
                            "params": {"cols": 120},
                        },
                    ),
                ],
            },
            {
                "name": "event_frames",
                "frames": frames,
            },
            {
                "name": "pty_bridge_contract",
                "resize_frames": {
                    "valid": resize_case("\x1b[RESIZE:120;40]"),
                    "trailing_bytes_rejected": resize_case("\x1b[RESIZE:120;40]x"),
                    "non_numeric_rejected": resize_case("\x1b[RESIZE:cols;40]"),
                    "missing_rows_rejected": resize_case("\x1b[RESIZE:120]"),
                },
                "valid_channels": {
                    "simple": bool(web_server._VALID_CHANNEL_RE.match("chat_1")),
                    "dot_dash": bool(web_server._VALID_CHANNEL_RE.match("chat.1-side")),
                    "empty": bool(web_server._VALID_CHANNEL_RE.match("")),
                    "space": bool(web_server._VALID_CHANNEL_RE.match("chat 1")),
                    "too_long": bool(web_server._VALID_CHANNEL_RE.match("x" * 129)),
                },
                "loopback_hosts": sorted(web_server._LOOPBACK_HOSTS),
            },
            {
                "name": "command_resolution",
                "responses": {
                    "help": server.handle_request(
                        {
                            "jsonrpc": "2.0",
                            "id": "resolve-help",
                            "method": "command.resolve",
                            "params": {"name": "help"},
                        }
                    ),
                    "alias_bg": server.handle_request(
                        {
                            "jsonrpc": "2.0",
                            "id": "resolve-bg",
                            "method": "command.resolve",
                            "params": {"name": "bg"},
                        }
                    ),
                    "unknown": server.handle_request(
                        {
                            "jsonrpc": "2.0",
                            "id": "resolve-missing",
                            "method": "command.resolve",
                            "params": {"name": "no-such-command"},
                        }
                    ),
                },
            },
            {
                "name": "cli_exec_blocking",
                "cases": {
                    "bare": server._cli_exec_blocked([]),
                    "setup": server._cli_exec_blocked(["setup"]),
                    "gateway": server._cli_exec_blocked(["gateway"]),
                    "sessions_browse": server._cli_exec_blocked(["sessions", "browse"]),
                    "config_edit": server._cli_exec_blocked(["config", "edit"]),
                    "version_allowed": server._cli_exec_blocked(["version"]),
                },
            },
            {
                "name": "details_completions",
                "cases": {
                    "root": server._details_completions("/details"),
                    "root_prefix": server._details_completions("/details t"),
                    "section_modes": server._details_completions("/details tools "),
                    "section_mode_prefix": server._details_completions("/details tools h"),
                    "not_details": server._details_completions("/help"),
                },
                "rpc": server.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": "complete-details",
                        "method": "complete.slash",
                        "params": {"text": "/details tools "},
                    }
                ),
            },
        ]

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
