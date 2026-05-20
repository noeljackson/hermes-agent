from __future__ import annotations

from parity_common import fixture, isolated_hermes_home, parse_out_arg, redact, write_fixture


SCRIPT = "auth-discovery-fixture.py"


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home() as home:
        from hermes_cli.config import (
            OPTIONAL_ENV_VARS,
            get_env_value,
            load_env,
            redact_key,
            _sanitize_env_lines,
        )
        from agent.redact import mask_secret, redact_sensitive_text

        env_path = home / ".env"
        env_path.write_text(
            "\n".join(
                [
                    "OPENAI_API_KEY=sk-hermes-parity-openai",
                    "ANTHROPIC_API_KEY=sk-ant-hermes-parity",
                    "OPENROUTER_API_KEY=sk-or-hermes-parity",
                    "",
                ]
            ),
            encoding="utf-8",
        )

        loaded = load_env()
        sanitized_lines = _sanitize_env_lines(
            ["OPENAI_API_KEY=sk-oneANTHROPIC_API_KEY=sk-two\n"]
        )
        cases = [
            {
                "name": "known_secret_metadata",
                "keys": {
                    key: {
                        "category": OPTIONAL_ENV_VARS.get(key, {}).get("category"),
                        "password": OPTIONAL_ENV_VARS.get(key, {}).get("password"),
                    }
                    for key in ["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "OPENROUTER_API_KEY"]
                },
            },
            {
                "name": "env_file_discovery",
                "loaded_keys": sorted(loaded.keys()),
                "values": {key: redact(value) for key, value in loaded.items()},
                "openai_value_present": bool(get_env_value("OPENAI_API_KEY")),
            },
            {
                "name": "redaction",
                "redacted": redact_key("sk-hermes-parity-openai"),
                "contains_raw_secret": "sk-hermes-parity-openai" in redact_key("sk-hermes-parity-openai"),
            },
            {
                "name": "env_line_sanitization",
                "lines": [line.strip() for line in sanitized_lines],
            },
            {
                "name": "sensitive_text_redaction",
                "mask_cases": {
                    "empty": mask_secret("", empty="(not set)"),
                    "short": mask_secret("short"),
                    "long": mask_secret("sk-proj-abcdefghijklmnopqrstuvwxyz123456"),
                },
                "redact_cases": {
                    "provider_prefix": redact_sensitive_text(
                        "Token sk-proj-abcdefghijklmnopqrstuvwxyz123456",
                        force=True,
                    ),
                    "auth_header": redact_sensitive_text(
                        "Authorization: Bearer ghp_abcdefghijklmnopqrstuvwxyz123456",
                        force=True,
                    ),
                    "env_assignment": redact_sensitive_text(
                        "OPENAI_API_KEY=sk-test-abcdefghijklmnopqrstuvwxyz",
                        force=True,
                    ),
                    "json_field": redact_sensitive_text(
                        '{"api_key": "sk-test-abcdefghijklmnopqrstuvwxyz"}',
                        force=True,
                    ),
                    "json_field_code_file": redact_sensitive_text(
                        '{"api_key": "fixture-value"}',
                        force=True,
                        code_file=True,
                    ),
                    "url_query": redact_sensitive_text(
                        "https://example.invalid/cb?code=abc123&state=ok&access_token=tok123",
                        force=True,
                    ),
                    "db_url": redact_sensitive_text(
                        "postgres://user:secret-password@example.invalid/db",
                        force=True,
                    ),
                    "userinfo_url": redact_sensitive_text(
                        "https://user:secret@example.invalid/path",
                        force=True,
                    ),
                    "form_body": redact_sensitive_text(
                        "client_secret=abc123&scope=read&token=tok123",
                        force=True,
                    ),
                    "private_key": redact_sensitive_text(
                        "-----BEGIN PRIVATE KEY-----\nabc\n-----END PRIVATE KEY-----",
                        force=True,
                    ),
                    "discord_mention": redact_sensitive_text(
                        "ping <@123456789012345678>",
                        force=True,
                    ),
                    "phone": redact_sensitive_text(
                        "call +15551234567",
                        force=True,
                    ),
                },
            },
        ]
    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
