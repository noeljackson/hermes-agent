#!/usr/bin/env python3
from __future__ import annotations

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "gateway-platform-fixture.py"


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home():
        from gateway.config import HomeChannel, Platform, PlatformConfig, SessionResetPolicy
        from gateway.config import (
            _coerce_bool,
            _coerce_float,
            _coerce_int,
            _normalize_notice_delivery,
            _normalize_unauthorized_dm_behavior,
        )
        from gateway.delivery import DeliveryTarget
        from gateway.restart import parse_restart_drain_timeout
        from gateway.runtime_footer import (
            build_footer_line,
            format_runtime_footer,
            resolve_footer_config,
        )
        from gateway.session import SessionSource
        from gateway.channel_directory import (
            _channel_target_name,
            _normalize_channel_query,
            _session_entry_id,
            _session_entry_name,
        )
        from gateway.platforms import api_server, slack, webhook
        from gateway.platforms.base import (
            MessageEvent,
            MessageType,
            ProcessingOutcome,
            _reply_anchor_for_event,
            _thread_metadata_for_source,
            safe_url_for_log,
            should_send_media_as_audio,
            utf16_len,
        )

        def delivery_case(target: str, origin: SessionSource | None = None):
            parsed = DeliveryTarget.parse(target, origin)
            return {
                "target": target,
                "parsed": {
                    "platform": parsed.platform.value,
                    "chat_id": parsed.chat_id,
                    "thread_id": parsed.thread_id,
                    "is_origin": parsed.is_origin,
                    "is_explicit": parsed.is_explicit,
                },
                "to_string": parsed.to_string(),
                "use_origin": origin is not None,
            }

        platforms = [platform.value for platform in Platform.__members__.values()]
        default_platform_config = PlatformConfig().to_dict()
        default_reset_policy = SessionResetPolicy().to_dict()
        home = HomeChannel(
            platform=Platform.TELEGRAM,
            chat_id="chat-1",
            name="Parity Chat",
            thread_id="topic-1",
        ).to_dict()
        parsed_platform = Platform("telegram").value

        cases = [
            {
                "name": "builtin_platform_inventory",
                "platform_count": len(platforms),
                "platforms": platforms,
                "parsed_platform": parsed_platform,
            },
            {
                "name": "platform_config_defaults",
                "config": default_platform_config,
                "reset_policy": default_reset_policy,
                "home_channel": home,
            },
            {
                "name": "adapter_base_contracts",
                "message_types": [item.value for item in MessageType],
                "processing_outcomes": [item.value for item in ProcessingOutcome],
                "audio_routing": {
                    "telegram_mp3": should_send_media_as_audio("telegram", ".mp3"),
                    "telegram_ogg_attachment": should_send_media_as_audio("telegram", ".ogg"),
                    "telegram_ogg_voice": should_send_media_as_audio("telegram", ".ogg", is_voice=True),
                    "slack_wav": should_send_media_as_audio("slack", ".wav"),
                    "unknown_txt": should_send_media_as_audio("slack", ".txt"),
                },
                "utf16": {
                    "ascii": utf16_len("abc"),
                    "emoji": utf16_len("a😀b"),
                    "cjk_ext": utf16_len("𠀋"),
                },
                "safe_urls": {
                    "secret_query": safe_url_for_log(
                        "https://user:pass@example.com/path/to/file.txt?token=secret#frag"
                    ),
                    "short_limit": safe_url_for_log("https://example.com/very/long/path", max_len=12),
                    "none": safe_url_for_log(None),
                },
            },
            {
                "name": "adapter_thread_contracts",
                "telegram_dm": {
                    "metadata": _thread_metadata_for_source(
                        SessionSource(
                            platform=Platform.TELEGRAM,
                            chat_id="chat-1",
                            chat_type="dm",
                            thread_id="42",
                            message_id="source-msg",
                        ),
                        reply_to_message_id="reply-msg",
                    ),
                    "reply_anchor": _reply_anchor_for_event(
                        MessageEvent(
                            text="hello",
                            source=SessionSource(
                                platform=Platform.TELEGRAM,
                                chat_id="chat-1",
                                chat_type="dm",
                                thread_id="42",
                            ),
                            message_id="msg-1",
                            reply_to_message_id="reply-1",
                        )
                    ),
                },
                "telegram_group_topic": {
                    "metadata": _thread_metadata_for_source(
                        SessionSource(
                            platform=Platform.TELEGRAM,
                            chat_id="group-1",
                            chat_type="group",
                            thread_id="topic-1",
                        ),
                        reply_to_message_id="reply-msg",
                    ),
                    "reply_anchor": _reply_anchor_for_event(
                        MessageEvent(
                            text="hello",
                            source=SessionSource(
                                platform=Platform.TELEGRAM,
                                chat_id="group-1",
                                chat_type="group",
                                thread_id="topic-1",
                            ),
                            message_id="msg-2",
                            reply_to_message_id="reply-2",
                        )
                    ),
                },
                "feishu_thread": {
                    "metadata": _thread_metadata_for_source(
                        SessionSource(
                            platform=Platform.FEISHU,
                            chat_id="chat-1",
                            chat_type="group",
                            thread_id="thread-1",
                        ),
                        reply_to_message_id="reply-msg",
                    ),
                    "reply_anchor": _reply_anchor_for_event(
                        MessageEvent(
                            text="hello",
                            source=SessionSource(
                                platform=Platform.FEISHU,
                                chat_id="chat-1",
                                chat_type="group",
                                thread_id="thread-1",
                            ),
                            message_id="msg-3",
                            reply_to_message_id="reply-3",
                        )
                    ),
                },
            },
            {
                "name": "webhook_api_server_helpers",
                "loopback": {
                    "localhost": webhook._is_loopback_host("localhost"),
                    "ipv6_bracket": webhook._is_loopback_host("::1"),
                    "public": webhook._is_loopback_host("0.0.0.0"),
                    "empty": webhook._is_loopback_host(""),
                },
                "ports": {
                    "none": api_server._coerce_port(None),
                    "string": api_server._coerce_port("9000"),
                    "bad": api_server._coerce_port("bad", default=1234),
                },
                "request_bools": {
                    "true_string": api_server._coerce_request_bool(" yes "),
                    "false_string": api_server._coerce_request_bool("off", default=True),
                    "int_zero": api_server._coerce_request_bool(0, default=True),
                    "unknown_default": api_server._coerce_request_bool("maybe", default=True),
                },
                "chat_content": {
                    "string": api_server._normalize_chat_content("hello"),
                    "parts": api_server._normalize_chat_content(
                        [
                            {"type": "text", "text": "one"},
                            {"type": "input_text", "text": "two"},
                            {"type": "image_url", "image_url": {"url": "https://example.com/i.png"}},
                            ["nested", {"type": "output_text", "text": "three"}],
                            7,
                        ]
                    ),
                    "none": api_server._normalize_chat_content(None),
                    "scalar": api_server._normalize_chat_content(123),
                },
            },
            {
                "name": "gateway_config_normalizers",
                "bools": [
                    {"value": None, "default": True, "result": _coerce_bool(None, True)},
                    {"value": None, "default": False, "result": _coerce_bool(None, False)},
                    {"value": " YES ", "default": False, "result": _coerce_bool(" YES ", False)},
                    {"value": "off", "default": True, "result": _coerce_bool("off", True)},
                    {"value": "maybe", "default": False, "result": _coerce_bool("maybe", False)},
                    {"value": 0, "default": True, "result": _coerce_bool(0, True)},
                    {"value": 2, "default": False, "result": _coerce_bool(2, False)},
                ],
                "floats": [
                    {"value": None, "default": 1.5, "result": _coerce_float(None, 1.5)},
                    {"value": "2.25", "default": 1.5, "result": _coerce_float("2.25", 1.5)},
                    {"value": "bad", "default": 1.5, "result": _coerce_float("bad", 1.5)},
                ],
                "ints": [
                    {"value": None, "default": 7, "result": _coerce_int(None, 7)},
                    {"value": "42", "default": 7, "result": _coerce_int("42", 7)},
                    {"value": "bad", "default": 7, "result": _coerce_int("bad", 7)},
                ],
                "unauthorized_dm": [
                    {"value": " PAIR ", "result": _normalize_unauthorized_dm_behavior(" PAIR ")},
                    {"value": "ignore", "result": _normalize_unauthorized_dm_behavior("ignore")},
                    {"value": "block", "result": _normalize_unauthorized_dm_behavior("block")},
                ],
                "notice_delivery": [
                    {"value": " private ", "result": _normalize_notice_delivery(" private ")},
                    {"value": "public", "result": _normalize_notice_delivery("public")},
                    {"value": "dm", "result": _normalize_notice_delivery("dm")},
                ],
            },
            {
                "name": "delivery_target_parsing",
                "origin": SessionSource(
                    platform=Platform.SLACK,
                    chat_id="C123",
                    chat_type="thread",
                    thread_id="T456",
                    user_id="U1",
                ).to_dict(),
                "cases": [
                    delivery_case(
                        "origin",
                        SessionSource(
                            platform=Platform.SLACK,
                            chat_id="C123",
                            chat_type="thread",
                            thread_id="T456",
                            user_id="U1",
                        ),
                    ),
                    delivery_case("origin"),
                    delivery_case(" local "),
                    delivery_case("Telegram"),
                    delivery_case("telegram:ChatID:TopicID"),
                    delivery_case("slack:C123"),
                    delivery_case("unknown:C123"),
                ],
            },
            {
                "name": "runtime_footer_helpers",
                "configs": [
                    {
                        "config": {},
                        "platform": None,
                        "resolved": resolve_footer_config({}, None),
                    },
                    {
                        "config": {
                            "display": {
                                "runtime_footer": {"enabled": True, "fields": ["cwd", "model"]},
                                "platforms": {
                                    "slack": {
                                        "runtime_footer": {
                                            "enabled": False,
                                            "fields": ["context_pct"],
                                        }
                                    }
                                },
                            }
                        },
                        "platform": "slack",
                        "resolved": resolve_footer_config(
                            {
                                "display": {
                                    "runtime_footer": {"enabled": True, "fields": ["cwd", "model"]},
                                    "platforms": {
                                        "slack": {
                                            "runtime_footer": {
                                                "enabled": False,
                                                "fields": ["context_pct"],
                                            }
                                        }
                                    },
                                }
                            },
                            "slack",
                        ),
                    },
                ],
                "formats": [
                    {
                        "model": "openai/gpt-5.4",
                        "context_tokens": 512,
                        "context_length": 2048,
                        "cwd": "/tmp/work",
                        "fields": ["model", "context_pct", "cwd"],
                        "footer": format_runtime_footer(
                            model="openai/gpt-5.4",
                            context_tokens=512,
                            context_length=2048,
                            cwd="/tmp/work",
                            fields=["model", "context_pct", "cwd"],
                        ),
                    },
                    {
                        "model": None,
                        "context_tokens": -1,
                        "context_length": 0,
                        "cwd": "",
                        "fields": ["model", "context_pct", "cwd", "unknown"],
                        "footer": format_runtime_footer(
                            model=None,
                            context_tokens=-1,
                            context_length=0,
                            cwd="",
                            fields=["model", "context_pct", "cwd", "unknown"],
                        ),
                    },
                ],
                "build_footer": build_footer_line(
                    user_config={"display": {"runtime_footer": {"enabled": True}}},
                    platform_key=None,
                    model="nous/gpt-5",
                    context_tokens=101,
                    context_length=400,
                    cwd="/tmp/work",
                ),
            },
            {
                "name": "restart_and_channel_helpers",
                "restart_timeouts": [
                    {"value": None, "result": parse_restart_drain_timeout(None)},
                    {"value": "", "result": parse_restart_drain_timeout("")},
                    {"value": "2.5", "result": parse_restart_drain_timeout("2.5")},
                    {"value": "-5", "result": parse_restart_drain_timeout("-5")},
                    {"value": "bad", "result": parse_restart_drain_timeout("bad")},
                ],
                "channel_queries": [
                    {"value": "#General ", "normalized": _normalize_channel_query("#General ")},
                    {"value": "  Thread Name  ", "normalized": _normalize_channel_query("  Thread Name  ")},
                ],
                "target_names": [
                    {
                        "platform": "discord",
                        "channel": {"name": "bot-home", "guild": "Hermes", "type": "channel"},
                        "target": _channel_target_name(
                            "discord",
                            {"name": "bot-home", "guild": "Hermes", "type": "channel"},
                        ),
                    },
                    {
                        "platform": "telegram",
                        "channel": {"name": "Ops", "type": "group"},
                        "target": _channel_target_name(
                            "telegram",
                            {"name": "Ops", "type": "group"},
                        ),
                    },
                    {
                        "platform": "slack",
                        "channel": {"name": "alerts"},
                        "target": _channel_target_name("slack", {"name": "alerts"}),
                    },
                ],
                "session_entries": [
                    {
                        "origin": {"chat_id": "chat-1"},
                        "id": _session_entry_id({"chat_id": "chat-1"}),
                        "name": _session_entry_name({"chat_id": "chat-1"}),
                    },
                    {
                        "origin": {
                            "chat_id": "chat-1",
                            "chat_name": "Main",
                            "thread_id": "42",
                            "chat_topic": "Ops",
                        },
                        "id": _session_entry_id(
                            {
                                "chat_id": "chat-1",
                                "chat_name": "Main",
                                "thread_id": "42",
                                "chat_topic": "Ops",
                            }
                        ),
                        "name": _session_entry_name(
                            {
                                "chat_id": "chat-1",
                                "chat_name": "Main",
                                "thread_id": "42",
                                "chat_topic": "Ops",
                            }
                        ),
                    },
                    {
                        "origin": {
                            "chat_id": "chat-1",
                            "user_name": "Noel",
                            "thread_id": "7",
                        },
                        "id": _session_entry_id(
                            {"chat_id": "chat-1", "user_name": "Noel", "thread_id": "7"}
                        ),
                        "name": _session_entry_name(
                            {"chat_id": "chat-1", "user_name": "Noel", "thread_id": "7"}
                        ),
                    },
                ],
            },
            {
                "name": "slack_rich_text_blocks",
                "blocks": [
                    {
                        "type": "rich_text",
                        "elements": [
                            {
                                "type": "rich_text_section",
                                "elements": [
                                    {"type": "text", "text": "Hello "},
                                    {"type": "user", "user_id": "U123"},
                                    {"type": "emoji", "name": "wave"},
                                ],
                            },
                            {
                                "type": "rich_text_quote",
                                "elements": [
                                    {
                                        "type": "rich_text_section",
                                        "elements": [
                                            {"type": "text", "text": "quoted"},
                                            {"type": "link", "text": "site", "url": "https://example.com"},
                                        ],
                                    }
                                ],
                            },
                            {
                                "type": "rich_text_list",
                                "style": "ordered",
                                "elements": [
                                    {
                                        "type": "rich_text_section",
                                        "elements": [{"type": "text", "text": "first"}],
                                    },
                                    {
                                        "type": "rich_text_section",
                                        "elements": [{"type": "text", "text": "second"}],
                                    },
                                ],
                            },
                        ],
                    }
                ],
                "text": slack._extract_text_from_slack_blocks(
                    [
                        {
                            "type": "rich_text",
                            "elements": [
                                {
                                    "type": "rich_text_section",
                                    "elements": [
                                        {"type": "text", "text": "Hello "},
                                        {"type": "user", "user_id": "U123"},
                                        {"type": "emoji", "name": "wave"},
                                    ],
                                },
                                {
                                    "type": "rich_text_quote",
                                    "elements": [
                                        {
                                            "type": "rich_text_section",
                                            "elements": [
                                                {"type": "text", "text": "quoted"},
                                                {"type": "link", "text": "site", "url": "https://example.com"},
                                            ],
                                        }
                                    ],
                                },
                                {
                                    "type": "rich_text_list",
                                    "style": "ordered",
                                    "elements": [
                                        {
                                            "type": "rich_text_section",
                                            "elements": [{"type": "text", "text": "first"}],
                                        },
                                        {
                                            "type": "rich_text_section",
                                            "elements": [{"type": "text", "text": "second"}],
                                        },
                                    ],
                                },
                            ],
                        }
                    ]
                ),
            },
        ]

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
