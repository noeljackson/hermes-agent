#!/usr/bin/env python3
from __future__ import annotations

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "gateway-platform-fixture.py"


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home():
        from gateway.config import HomeChannel, Platform, PlatformConfig, SessionResetPolicy
        from gateway.session import SessionSource
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
