from __future__ import annotations

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "gateway-message-fixture.py"


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home():
        from gateway.config import Platform
        from gateway.platforms.base import (
            MessageEvent,
            MessageType,
            coerce_plaintext_gateway_command,
        )
        from gateway.session import SessionSource, build_session_key, is_shared_multi_user_session

        def make_source(chat_type="dm"):
            return SessionSource(
                platform=Platform.TELEGRAM,
                chat_id="chat-1",
                chat_name="Parity Chat",
                chat_type=chat_type,
                user_id="user-1",
                user_name="Ada",
                message_id="msg-1",
            )

        source = make_source()
        group_source = make_source("group")

        def coerce_case(text, message_type=MessageType.TEXT, source=source):
            event = MessageEvent(
                text=text,
                message_type=message_type,
                source=source,
                message_id="coerce",
            )
            coerce_plaintext_gateway_command(event)
            return event.text

        command_source = SessionSource(
            platform=Platform.TELEGRAM,
            chat_id="chat-1",
            chat_name="Parity Chat",
            chat_type="dm",
            user_id="user-1",
            user_name="Ada",
            message_id="msg-1",
        )
        command_event = MessageEvent(
            text="/new --model fake/model",
            message_type=MessageType.COMMAND,
            source=command_source,
            message_id="msg-1",
        )
        mention_event = MessageEvent(
            text="/help@HermesBot topic",
            message_type=MessageType.COMMAND,
            source=command_source,
            message_id="msg-2",
        )
        path_event = MessageEvent(
            text="/tmp/file.txt",
            message_type=MessageType.TEXT,
            source=command_source,
            message_id="msg-3",
        )
        rich_source = SessionSource(
            platform=Platform.SLACK,
            chat_id="channel-1",
            chat_name="ops",
            chat_type="channel",
            user_id="user-1",
            user_name="Ada",
            thread_id="thread-1",
            chat_topic="operations",
            user_id_alt="union-1",
            chat_id_alt="internal-1",
            guild_id="workspace-1",
            parent_chat_id="parent-1",
            message_id="msg-99",
        )
        cases = [
            {
                "name": "message_normalization",
                "source": command_source.to_dict(),
                "command": command_event.get_command(),
                "command_args": command_event.get_command_args(),
                "mention_command": mention_event.get_command(),
                "mention_args": mention_event.get_command_args(),
                "path_command": path_event.get_command(),
                "is_command": command_event.is_command(),
            },
            {
                "name": "plaintext_restart_coercion",
                "cases": {
                    "dm_restart_gateway": coerce_case("restart gateway"),
                    "dm_restart_hermes_gateway": coerce_case("please restart hermes gateway!"),
                    "dm_restart_hermes": coerce_case("restart hermes"),
                    "group_restart_gateway": coerce_case("restart gateway", source=group_source),
                    "already_slash": coerce_case("/restart"),
                    "command_message_type": coerce_case(
                        "restart gateway",
                        message_type=MessageType.COMMAND,
                    ),
                    "unrelated": coerce_case("restart the build"),
                },
            },
            {
                "name": "session_source_roundtrip",
                "dm_description": source.description,
                "group_description": group_source.description,
                "rich_source": rich_source.to_dict(),
                "from_dict_roundtrip": SessionSource.from_dict(
                    rich_source.to_dict()
                ).to_dict(),
            },
            {
                "name": "session_key_construction",
                "cases": {
                    "telegram_dm": build_session_key(
                        SessionSource(Platform.TELEGRAM, chat_id="chat-1")
                    ),
                    "telegram_dm_thread": build_session_key(
                        SessionSource(
                            Platform.TELEGRAM,
                            chat_id="chat-1",
                            thread_id="topic-1",
                        )
                    ),
                    "telegram_group_default_per_user": build_session_key(
                        SessionSource(
                            Platform.TELEGRAM,
                            chat_id="group-1",
                            chat_type="group",
                            user_id="user-1",
                        )
                    ),
                    "telegram_group_shared": build_session_key(
                        SessionSource(
                            Platform.TELEGRAM,
                            chat_id="group-1",
                            chat_type="group",
                            user_id="user-1",
                        ),
                        group_sessions_per_user=False,
                    ),
                    "discord_thread_shared": build_session_key(
                        SessionSource(
                            Platform.DISCORD,
                            chat_id="channel-1",
                            chat_type="group",
                            user_id="user-1",
                            thread_id="thread-1",
                        )
                    ),
                    "discord_thread_per_user": build_session_key(
                        SessionSource(
                            Platform.DISCORD,
                            chat_id="channel-1",
                            chat_type="group",
                            user_id="user-1",
                            thread_id="thread-1",
                        ),
                        thread_sessions_per_user=True,
                    ),
                    "group_no_ids": build_session_key(
                        SessionSource(Platform.TELEGRAM, chat_id="", chat_type="group")
                    ),
                    "whatsapp_dm_normalized": build_session_key(
                        SessionSource(
                            Platform.WHATSAPP,
                            chat_id="+15551234567:9@s.whatsapp.net",
                        )
                    ),
                    "whatsapp_group_participant_normalized": build_session_key(
                        SessionSource(
                            Platform.WHATSAPP,
                            chat_id="group-1@g.us",
                            chat_type="group",
                            user_id="15551234567:9@s.whatsapp.net",
                        )
                    ),
                },
            },
            {
                "name": "shared_multi_user_detection",
                "cases": {
                    "dm": is_shared_multi_user_session(
                        SessionSource(Platform.TELEGRAM, chat_id="chat-1")
                    ),
                    "group_default_per_user": is_shared_multi_user_session(
                        SessionSource(
                            Platform.TELEGRAM,
                            chat_id="group-1",
                            chat_type="group",
                            user_id="user-1",
                        )
                    ),
                    "group_shared": is_shared_multi_user_session(
                        SessionSource(
                            Platform.TELEGRAM,
                            chat_id="group-1",
                            chat_type="group",
                            user_id="user-1",
                        ),
                        group_sessions_per_user=False,
                    ),
                    "thread_shared_default": is_shared_multi_user_session(
                        SessionSource(
                            Platform.DISCORD,
                            chat_id="channel-1",
                            chat_type="group",
                            user_id="user-1",
                            thread_id="thread-1",
                        )
                    ),
                    "thread_per_user": is_shared_multi_user_session(
                        SessionSource(
                            Platform.DISCORD,
                            chat_id="channel-1",
                            chat_type="group",
                            user_id="user-1",
                            thread_id="thread-1",
                        ),
                        thread_sessions_per_user=True,
                    ),
                },
            },
        ]
    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
