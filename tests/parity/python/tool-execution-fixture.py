from __future__ import annotations

import asyncio
import json
import os
import subprocess
import tempfile
from pathlib import Path

from parity_common import fixture, isolated_hermes_home, parse_out_arg, write_fixture


SCRIPT = "tool-execution-fixture.py"


def parsed(result: str):
    return json.loads(result)


def normalize_home(value, home: Path):
    if isinstance(value, dict):
        return {key: normalize_home(item, home) for key, item in value.items()}
    if isinstance(value, list):
        return [normalize_home(item, home) for item in value]
    if isinstance(value, str):
        return value.replace(str(home), "<HERMES_HOME>")
    return value


def normalize_path_marker(value, path: Path, marker: str):
    if isinstance(value, dict):
        return {key: normalize_path_marker(item, path, marker) for key, item in value.items()}
    if isinstance(value, list):
        return [normalize_path_marker(item, path, marker) for item in value]
    if isinstance(value, str):
        return value.replace(str(path), marker)
    return value


class LocalFixtureEnv:
    def __init__(self, cwd: str):
        self.cwd = cwd

    def execute(self, command, cwd=None, timeout=None, stdin_data=None):
        result = subprocess.run(
            command,
            shell=True,
            cwd=cwd or self.cwd,
            input=stdin_data,
            text=True,
            capture_output=True,
            timeout=timeout,
        )
        return {
            "output": result.stdout + result.stderr,
            "returncode": result.returncode,
        }


def deterministic_web_image_browser_cases():
    from agent.web_search_provider import WebSearchProvider
    from agent import web_search_registry
    from tools import browser_tool, image_generation_tool, web_tools

    class FakeWebProvider(WebSearchProvider):
        def __init__(
            self,
            name: str,
            *,
            search: bool = True,
            extract: bool = False,
            available: bool = True,
            display_name: str | None = None,
        ):
            self._name = name
            self._search = search
            self._extract = extract
            self._available = available
            self._display_name = display_name or name

        @property
        def name(self) -> str:
            return self._name

        @property
        def display_name(self) -> str:
            return self._display_name

        def is_available(self) -> bool:
            return self._available

        def supports_search(self) -> bool:
            return self._search

        def supports_extract(self) -> bool:
            return self._extract

        def search(self, query: str, limit: int = 5):
            return {
                "success": True,
                "provider": self.name,
                "data": {
                    "web": [
                        {
                            "title": f"Result {limit}",
                            "url": "https://example.com/result",
                            "description": query,
                            "position": 1,
                        }
                    ]
                },
            }

        def extract(self, urls, **kwargs):
            return [
                {
                    "url": url,
                    "title": "Fixture Page",
                    "content": "Text before ![inline](data:image/png;base64,AAAA) text after.",
                    "raw_content": "raw content should be omitted",
                    "metadata": {"ignored": True},
                }
                for url in urls
            ]

    image_payload_cases = []
    for case in [
        {
            "label": "flux_defaults_seed_and_supported_overrides",
            "model": "fal-ai/flux-2/klein/9b",
            "prompt": "  Painted skyline  ",
            "aspect_ratio": "LANDSCAPE",
            "seed": 123,
            "overrides": {"guidance_scale": 9.9, "output_format": "jpeg", "unknown": "drop"},
        },
        {
            "label": "nano_banana_aspect_ratio_and_web_search_override",
            "model": "fal-ai/nano-banana-pro",
            "prompt": "portrait subject",
            "aspect_ratio": "portrait",
            "seed": None,
            "overrides": {"enable_web_search": True, "limit_generations": False},
        },
        {
            "label": "gpt_literal_square_and_unsupported_drop",
            "model": "fal-ai/gpt-image-1.5",
            "prompt": "logo",
            "aspect_ratio": "square",
            "seed": 42,
            "overrides": {"enable_web_search": True, "background": "transparent"},
        },
        {
            "label": "invalid_aspect_defaults_to_landscape",
            "model": "fal-ai/gpt-image-2",
            "prompt": "wide scene",
            "aspect_ratio": "panorama",
            "seed": None,
            "overrides": {},
        },
    ]:
        image_payload_cases.append(
            {
                **case,
                "payload": image_generation_tool._build_fal_payload(
                    case["model"],
                    case["prompt"],
                    case["aspect_ratio"],
                    seed=case["seed"],
                    overrides=case["overrides"],
                ),
            }
        )

    image_no_prompt = parsed(image_generation_tool.image_generate_tool("  "))

    original_search_backend = web_tools._get_search_backend
    original_extract_backend = web_tools._get_extract_backend
    web_search_registry._reset_for_tests()
    try:
        web_tools._get_search_backend = lambda: "fixture-search"
        web_search_registry.register_provider(
            FakeWebProvider("fixture-search", search=True, extract=False)
        )
        web_search_limit_clamp = parsed(web_tools.web_search_tool("rust parity", limit="250"))
        web_search_invalid_limit = parsed(web_tools.web_search_tool("rust parity", limit="bad"))
    finally:
        web_tools._get_search_backend = original_search_backend
        web_search_registry._reset_for_tests()

    web_extract_secret = parsed(
        asyncio.run(web_tools.web_extract_tool(["https://example.com/?key=sk-test-secret"], use_llm_processing=False))
    )
    web_extract_ssrf = parsed(
        asyncio.run(web_tools.web_extract_tool(["http://127.0.0.1/private"], use_llm_processing=False))
    )

    web_search_registry._reset_for_tests()
    try:
        web_tools._get_extract_backend = lambda: "fixture-search-only"
        web_search_registry.register_provider(
            FakeWebProvider(
                "fixture-search-only",
                search=True,
                extract=False,
                display_name="Fixture Search Only",
            )
        )
        web_extract_search_only = parsed(
            asyncio.run(web_tools.web_extract_tool(["https://example.com/page"], use_llm_processing=False))
        )
    finally:
        web_tools._get_extract_backend = original_extract_backend
        web_search_registry._reset_for_tests()

    web_search_registry._reset_for_tests()
    try:
        web_tools._get_extract_backend = lambda: "fixture-extract"
        web_search_registry.register_provider(
            FakeWebProvider("fixture-extract", search=False, extract=True)
        )
        web_extract_fake_provider = parsed(
            asyncio.run(web_tools.web_extract_tool(["https://example.com/page"], use_llm_processing=False))
        )
    finally:
        web_tools._get_extract_backend = original_extract_backend
        web_search_registry._reset_for_tests()

    browser_secret = parsed(
        browser_tool.browser_navigate("https://evil.example/?token=sk-ant-secret", task_id="parity")
    )

    original_navigation_session_key = browser_tool._navigation_session_key
    original_is_local_backend = browser_tool._is_local_backend
    original_is_always_blocked_url = browser_tool._is_always_blocked_url
    original_is_safe_url = browser_tool._is_safe_url
    original_allow_private_urls = browser_tool._allow_private_urls
    original_check_website_access = browser_tool.check_website_access
    try:
        browser_tool._navigation_session_key = lambda task_id, url: task_id or "default"
        browser_tool._is_local_backend = lambda: False
        browser_tool._is_always_blocked_url = lambda url: True
        browser_metadata = parsed(
            browser_tool.browser_navigate("http://169.254.169.254/latest/meta-data/", task_id="parity")
        )

        browser_tool._is_always_blocked_url = lambda url: False
        browser_tool._allow_private_urls = lambda: False
        browser_tool._is_safe_url = lambda url: False
        browser_private = parsed(
            browser_tool.browser_navigate("http://10.0.0.10/admin", task_id="parity")
        )

        browser_tool._is_safe_url = lambda url: True
        browser_tool.check_website_access = lambda url: {
            "message": "Blocked by fixture policy",
            "host": "blocked.example",
            "rule": "deny",
            "source": "fixture",
        }
        browser_policy = parsed(
            browser_tool.browser_navigate("https://blocked.example/path", task_id="parity")
        )
    finally:
        browser_tool._navigation_session_key = original_navigation_session_key
        browser_tool._is_local_backend = original_is_local_backend
        browser_tool._is_always_blocked_url = original_is_always_blocked_url
        browser_tool._is_safe_url = original_is_safe_url
        browser_tool._allow_private_urls = original_allow_private_urls
        browser_tool.check_website_access = original_check_website_access

    return [
        {"name": "image_fal_payload_cases", "cases": image_payload_cases},
        {"name": "image_generate_empty_prompt", "result": image_no_prompt},
        {"name": "web_search_limit_clamp", "result": web_search_limit_clamp},
        {"name": "web_search_invalid_limit", "result": web_search_invalid_limit},
        {"name": "web_extract_secret_url", "result": web_extract_secret},
        {"name": "web_extract_ssrf_block", "result": web_extract_ssrf},
        {"name": "web_extract_search_only_backend", "result": web_extract_search_only},
        {"name": "web_extract_fake_provider", "result": web_extract_fake_provider},
        {"name": "browser_navigate_secret_url", "result": browser_secret},
        {"name": "browser_navigate_metadata_url", "result": browser_metadata},
        {"name": "browser_navigate_private_url", "result": browser_private},
        {"name": "browser_navigate_policy_block", "result": browser_policy},
    ]


def deterministic_voice_tts_stt_cases():
    from hermes_cli import voice as voice_cli
    from tools import transcription_tools as stt
    from tools import tts_tool as tts

    voice_config_cases = []
    for cfg in [
        {},
        {"voice": True},
        {"voice": "ctrl+space"},
        {"voice": {"record_key": "ctrl+space"}},
        {"voice": {"record_key": "option+return"}},
        {"voice": {"record_key": ""}},
    ]:
        voice_config_cases.append(
            {
                "config": cfg,
                "record_key": voice_cli.voice_record_key_from_config(cfg),
            }
        )

    voice_key_cases = []
    for raw in [
        None,
        True,
        7,
        "",
        "ctrl+b",
        "control+o",
        "CTRL + SPACE",
        "alt+space",
        "option+return",
        "ctrl+c",
        "ctrl+d",
        "ctrl+l",
        "alt+c",
        "super+b",
        "windows+space",
        "ctrl+alt+r",
        "b",
        "space",
        "ctrl+spcae",
        "ctrl+delete",
    ]:
        voice_key_cases.append(
            {
                "raw": raw,
                "normalized": voice_cli.normalize_voice_record_key_for_prompt_toolkit(raw),
                "status": voice_cli.format_voice_record_key_for_status(raw),
            }
        )

    tts_config = {
        "provider": " Piper-Local ",
        "providers": {
            "piper-local": {
                "type": "command",
                "command": "piper --input {input_path} --output {output_path}",
                "output_format": "wav",
                "timeout": "2.5",
                "voice_compatible": "yes",
                "max_text_length": 1234,
            },
            "fallback-command": {
                "type": "command",
                "command": "synth {text_path} {output_path}",
            },
            "bad-command": {
                "type": "command",
                "command": "   ",
                "output_format": "bad",
                "timeout": -1,
                "voice_compatible": "no",
            },
            "edge": {
                "type": "command",
                "command": "shadow-built-in",
            },
        },
        "elevenlabs": {"model_id": "eleven_flash_v2_5"},
        "openai": {"max_text_length": 111},
    }
    tts_provider_cases = []
    for cfg in [
        {},
        {"provider": ""},
        {"provider": " OpenAI "},
        {"provider": " Piper-Local "},
    ]:
        tts_provider_cases.append({"config": cfg, "provider": tts._get_provider(cfg)})

    tts_length_cases = []
    for provider in [
        "openai",
        "elevenlabs",
        "piper-local",
        "fallback-command",
        "unknown",
        "",
        None,
    ]:
        tts_length_cases.append(
            {
                "provider": provider,
                "max_text_length": tts._resolve_max_text_length(provider, tts_config),
            }
        )

    tts_command_cases = {
        "provider_config": tts._get_named_provider_config(tts_config, "piper-local"),
        "builtin_shadow_config": tts._get_named_provider_config(tts_config, "edge"),
        "is_command": tts._is_command_provider_config(
            tts._get_named_provider_config(tts_config, "piper-local")
        ),
        "is_bad_command": tts._is_command_provider_config(
            tts._get_named_provider_config(tts_config, "bad-command")
        ),
        "iter_command_names": sorted(name for name, _ in tts._iter_command_providers(tts_config)),
        "timeout": tts._get_command_tts_timeout(
            tts._get_named_provider_config(tts_config, "piper-local")
        ),
        "bad_timeout": tts._get_command_tts_timeout(
            tts._get_named_provider_config(tts_config, "bad-command")
        ),
        "output_from_path": tts._get_command_tts_output_format(
            tts._get_named_provider_config(tts_config, "piper-local"),
            "/tmp/voice output.OGG",
        ),
        "output_from_config": tts._get_command_tts_output_format(
            tts._get_named_provider_config(tts_config, "piper-local")
        ),
        "bad_output": tts._get_command_tts_output_format(
            tts._get_named_provider_config(tts_config, "bad-command")
        ),
        "voice_compatible": tts._is_command_tts_voice_compatible(
            tts._get_named_provider_config(tts_config, "piper-local")
        ),
        "bad_voice_compatible": tts._is_command_tts_voice_compatible(
            tts._get_named_provider_config(tts_config, "bad-command")
        ),
    }

    placeholders = {
        "input_path": "/tmp/input text.txt",
        "text_path": "/tmp/input text.txt",
        "output_path": "/tmp/out file.mp3",
        "format": "mp3",
        "voice": "Amy O'Neil",
        "model": "model$`one",
        "speed": "1.0",
        "text": "Hello $USER",
    }
    tts_template_cases = []
    for template in [
        "tool --in {input_path} --out {output_path}",
        "tool --voice '{voice}' --model \"{model}\" --text {text}",
        "tool --literal {{format}} --skip ${model} --speed {speed}",
    ]:
        tts_template_cases.append(
            {
                "template": template,
                "rendered": tts._render_command_tts_template(template, placeholders),
            }
        )

    tts_markdown_cases = []
    for text in [
        "# Heading\n\nHello **bold** and *italic* with `code`.",
        "Read [docs](https://example.com/docs) and https://secret.example/token.",
        "- item one\n* item two\n---\n```python\nsecret()\n```",
    ]:
        tts_markdown_cases.append(
            {"input": text, "stripped": tts._strip_markdown_for_tts(text)}
        )

    original_fast = stt._HAS_FASTER_WHISPER
    original_openai = stt._HAS_OPENAI
    original_has_local_command = stt._has_local_command
    original_has_openai_audio_backend = stt._has_openai_audio_backend
    original_get_env_value = stt.get_env_value
    try:
        stt_provider_cases = []
        for case in [
            {
                "name": "disabled",
                "config": {"enabled": False},
                "fast": True,
                "local_command": True,
                "openai": True,
                "env": {"GROQ_API_KEY": "fake-groq"},
                "openai_backend": True,
            },
            {
                "name": "explicit_local_fast",
                "config": {"provider": "local"},
                "fast": True,
                "local_command": False,
                "openai": False,
                "env": {},
                "openai_backend": False,
            },
            {
                "name": "explicit_local_command",
                "config": {"provider": "local"},
                "fast": False,
                "local_command": True,
                "openai": False,
                "env": {},
                "openai_backend": False,
            },
            {
                "name": "explicit_local_unavailable",
                "config": {"provider": "local"},
                "fast": False,
                "local_command": False,
                "openai": False,
                "env": {},
                "openai_backend": False,
            },
            {
                "name": "explicit_local_command_fallback_fast",
                "config": {"provider": "local_command"},
                "fast": True,
                "local_command": False,
                "openai": False,
                "env": {},
                "openai_backend": False,
            },
            {
                "name": "explicit_groq_key",
                "config": {"provider": "groq"},
                "fast": False,
                "local_command": False,
                "openai": True,
                "env": {"GROQ_API_KEY": "fake-groq"},
                "openai_backend": False,
            },
            {
                "name": "explicit_groq_missing_key",
                "config": {"provider": "groq"},
                "fast": False,
                "local_command": False,
                "openai": True,
                "env": {},
                "openai_backend": False,
            },
            {
                "name": "explicit_openai_backend",
                "config": {"provider": "openai"},
                "fast": False,
                "local_command": False,
                "openai": True,
                "env": {},
                "openai_backend": True,
            },
            {
                "name": "auto_local_fast",
                "config": {},
                "fast": True,
                "local_command": True,
                "openai": True,
                "env": {"GROQ_API_KEY": "fake-groq"},
                "openai_backend": True,
            },
            {
                "name": "auto_local_command",
                "config": {},
                "fast": False,
                "local_command": True,
                "openai": True,
                "env": {"GROQ_API_KEY": "fake-groq"},
                "openai_backend": True,
            },
            {
                "name": "auto_groq",
                "config": {},
                "fast": False,
                "local_command": False,
                "openai": True,
                "env": {"GROQ_API_KEY": "fake-groq"},
                "openai_backend": True,
            },
            {
                "name": "auto_openai",
                "config": {},
                "fast": False,
                "local_command": False,
                "openai": True,
                "env": {},
                "openai_backend": True,
            },
            {
                "name": "auto_none",
                "config": {},
                "fast": False,
                "local_command": False,
                "openai": False,
                "env": {},
                "openai_backend": False,
            },
            {
                "name": "explicit_unknown_passthrough",
                "config": {"provider": "fixture"},
                "fast": False,
                "local_command": False,
                "openai": False,
                "env": {},
                "openai_backend": False,
            },
        ]:
            stt._HAS_FASTER_WHISPER = case["fast"]
            stt._HAS_OPENAI = case["openai"]
            stt._has_local_command = lambda case=case: case["local_command"]
            stt._has_openai_audio_backend = lambda case=case: case["openai_backend"]
            stt.get_env_value = lambda name, default=None, case=case: case["env"].get(name, default)
            stt_provider_cases.append(
                {
                    "name": case["name"],
                    "config": case["config"],
                    "provider": stt._get_provider(case["config"]),
                }
            )
    finally:
        stt._HAS_FASTER_WHISPER = original_fast
        stt._HAS_OPENAI = original_openai
        stt._has_local_command = original_has_local_command
        stt._has_openai_audio_backend = original_has_openai_audio_backend
        stt.get_env_value = original_get_env_value

    stt_enabled_cases = []
    for cfg in [
        {},
        {"enabled": True},
        {"enabled": False},
        {"enabled": "yes"},
        {"enabled": "no"},
        {"enabled": "0"},
        {"enabled": "unexpected"},
    ]:
        stt_enabled_cases.append({"config": cfg, "enabled": stt.is_stt_enabled(cfg)})

    stt_model_cases = []
    for model in [None, "", "whisper-1", "whisper-large-v3-turbo", "small", "large-v3"]:
        stt_model_cases.append(
            {"model": model, "normalized": stt._normalize_local_model(model)}
        )

    with tempfile.TemporaryDirectory(prefix="hermes-audio-fixture-") as tmp:
        root = Path(tmp)
        (root / "folder.wav").mkdir()
        (root / "note.txt").write_text("not audio", encoding="utf-8")
        (root / "ok.wav").write_bytes(b"RIFF")
        too_large = root / "huge.mp3"
        with too_large.open("wb") as fh:
            fh.truncate(stt.MAX_FILE_SIZE + 1)

        audio_paths = [
            root / "missing.wav",
            root / "folder.wav",
            root / "note.txt",
            root / "ok.wav",
            too_large,
        ]
        audio_validation_cases = []
        for path in audio_paths:
            result = stt._validate_audio_file(str(path))
            audio_validation_cases.append(
                {
                    "path": str(path).replace(str(root), "<AUDIO_ROOT>"),
                    "result": normalize_path_marker(result, root, "<AUDIO_ROOT>"),
                }
            )

    return [
        {"name": "voice_record_key_config", "cases": voice_config_cases},
        {"name": "voice_record_key_normalization", "cases": voice_key_cases},
        {"name": "tts_provider_resolution", "cases": tts_provider_cases},
        {"name": "tts_max_text_length", "config": tts_config, "cases": tts_length_cases},
        {"name": "tts_command_provider_helpers", "result": tts_command_cases},
        {"name": "tts_command_template_rendering", "cases": tts_template_cases},
        {"name": "tts_markdown_stripping", "cases": tts_markdown_cases},
        {"name": "stt_enabled_resolution", "cases": stt_enabled_cases},
        {"name": "stt_provider_resolution", "cases": stt_provider_cases},
        {"name": "stt_local_model_normalization", "cases": stt_model_cases},
        {"name": "stt_audio_file_validation", "cases": audio_validation_cases},
    ]


def main() -> int:
    out = parse_out_arg()
    with isolated_hermes_home() as home:
        from model_tools import handle_function_call
        from tools.clarify_tool import clarify_tool
        from tools.file_operations import ShellFileOperations
        import tools.file_tools as file_tools
        from tools.memory_tool import MemoryStore, memory_tool
        from tools.registry import tool_error
        from tools.skills_tool import skill_view, skills_list
        from tools.todo_tool import TodoStore, todo_tool

        cases = [
            {
                "name": "clarify_empty_question",
                "result": parsed(clarify_tool("  ")),
            },
            {
                "name": "clarify_no_callback",
                "result": parsed(
                    clarify_tool(
                        "Pick one",
                        choices=[" first ", "second", "", "third", "fourth", "fifth"],
                    )
                ),
            },
            {
                "name": "agent_loop_tool_block",
                "result": parsed(
                    handle_function_call(
                        "memory",
                        {"action": "add", "target": "memory", "content": "Remember this."},
                    )
                ),
            },
            {
                "name": "unknown_tool_error",
                "result": parsed(handle_function_call("__missing_tool__", {})),
            },
            {
                "name": "tool_error_with_extra",
                "result": parsed(tool_error("bad input", success=False, code=400)),
            },
        ]
        cases.extend(deterministic_web_image_browser_cases())
        cases.extend(deterministic_voice_tts_stt_cases())

        todo_store = TodoStore()
        cases.extend(
            [
                {
                    "name": "todo_handler_replace",
                    "result": parsed(
                        todo_tool(
                            todos=[
                                {
                                    "id": "plan",
                                    "content": "Write parity fixture",
                                    "status": "in_progress",
                                },
                                {
                                    "id": "verify",
                                    "content": "Run checks",
                                    "status": "pending",
                                },
                                {
                                    "id": "verify",
                                    "content": "Run full checks",
                                    "status": "bad-status",
                                },
                            ],
                            store=todo_store,
                        )
                    ),
                },
                {
                    "name": "todo_handler_merge",
                    "result": parsed(
                        todo_tool(
                            todos=[
                                {"id": "plan", "status": "completed"},
                                {
                                    "id": "commit",
                                    "content": "Commit result",
                                    "status": "pending",
                                },
                            ],
                            merge=True,
                            store=todo_store,
                        )
                    ),
                },
                {
                    "name": "todo_handler_read",
                    "result": parsed(todo_tool(store=todo_store)),
                },
            ]
        )

        memory_store = MemoryStore(memory_char_limit=500, user_char_limit=500)
        memory_store.load_from_disk()
        cases.extend(
            [
                {
                    "name": "memory_handler_add",
                    "result": parsed(
                        memory_tool(
                            "add",
                            target="memory",
                            content="Tool handler remembers durable facts.",
                            store=memory_store,
                        )
                    ),
                },
                {
                    "name": "memory_handler_replace",
                    "result": parsed(
                        memory_tool(
                            "replace",
                            target="memory",
                            old_text="durable facts",
                            content="Tool handler remembers Rust parity facts.",
                            store=memory_store,
                        )
                    ),
                },
                {
                    "name": "memory_handler_remove_missing",
                    "result": parsed(
                        memory_tool(
                            "remove",
                            target="memory",
                            old_text="not present",
                            store=memory_store,
                        )
                    ),
                },
            ]
        )

        skills_root = Path(os.environ["HERMES_HOME"]) / "skills"
        demo_skill = skills_root / "testing" / "demo-skill"
        demo_skill.mkdir(parents=True)
        (demo_skill / "references").mkdir()
        (demo_skill / "references" / "info.md").write_text(
            "Reference details.\n", encoding="utf-8"
        )
        (demo_skill / "scripts").mkdir()
        (demo_skill / "scripts" / "helper.sh").write_text(
            "#!/bin/sh\necho helper\n", encoding="utf-8"
        )
        (demo_skill / "SKILL.md").write_text(
            """---
name: Demo Skill
description: Demonstrates tool handler listing.
platforms: [linux, macos]
---
# Demo Skill
""",
            encoding="utf-8",
        )
        root_skill = skills_root / "root-skill"
        root_skill.mkdir(parents=True)
        (root_skill / "SKILL.md").write_text(
            """---
name: Root Skill
---
# Root Skill

Fallback description for root skill.
""",
            encoding="utf-8",
        )
        cases.extend(
            [
                {
                    "name": "skills_list_handler_all",
                    "result": parsed(skills_list()),
                },
                {
                    "name": "skills_list_handler_category",
                    "result": parsed(skills_list(category="testing")),
                },
                {
                    "name": "skill_view_handler_main",
                    "result": normalize_home(parsed(skill_view("demo-skill")), home),
                },
                {
                    "name": "skill_view_handler_reference",
                    "result": normalize_home(
                        parsed(skill_view("demo-skill", "references/info.md")), home
                    ),
                },
            ]
        )

        with tempfile.TemporaryDirectory(prefix="hermes-file-tools-") as tmp:
            workspace = Path(tmp)
            (workspace / "notes.txt").write_text(
                "alpha\nbeta\nalpha beta\n", encoding="utf-8"
            )
            (workspace / "patch.txt").write_text(
                "alpha\nbeta\nalpha beta\n", encoding="utf-8"
            )
            (workspace / "nested").mkdir()
            (workspace / "nested" / "alpha.md").write_text(
                "nested alpha\n", encoding="utf-8"
            )
            task_id = "file-tool-fixture"
            os.environ["TERMINAL_CWD"] = str(workspace)
            file_tools.clear_file_ops_cache(task_id)
            with file_tools._file_ops_lock:
                file_tools._file_ops_cache[task_id] = ShellFileOperations(
                    LocalFixtureEnv(str(workspace))
                )

            cases.extend(
                [
                    {
                        "name": "read_file_handler",
                        "result": parsed(
                            file_tools._handle_read_file(
                                {"path": "notes.txt", "offset": 2, "limit": 2},
                                task_id=task_id,
                            )
                        ),
                    },
                    {
                        "name": "write_file_handler_missing_content",
                        "result": parsed(
                            file_tools._handle_write_file(
                                {"path": "created.txt"}, task_id=task_id
                            )
                        ),
                    },
                    {
                        "name": "write_file_handler",
                        "result": parsed(
                            file_tools._handle_write_file(
                                {"path": "created.txt", "content": "created\n"},
                                task_id=task_id,
                            )
                        ),
                        "file_content": (workspace / "created.txt").read_text(
                            encoding="utf-8"
                        ),
                    },
                    {
                        "name": "patch_replace_handler",
                        "result": parsed(
                            file_tools._handle_patch(
                                {
                                    "mode": "replace",
                                    "path": "patch.txt",
                                    "old_string": "alpha beta",
                                    "new_string": "alpha BETA",
                                },
                                task_id=task_id,
                            )
                        ),
                        "file_content": (workspace / "patch.txt").read_text(
                            encoding="utf-8"
                        ),
                    },
                    {
                        "name": "search_files_files_handler",
                        "result": parsed(
                            file_tools._handle_search_files(
                                {
                                    "pattern": "*.md",
                                    "target": "files",
                                    "path": ".",
                                    "limit": 5,
                                },
                                task_id=task_id,
                            )
                        ),
                    },
                ]
            )

    write_fixture(out, fixture(SCRIPT, cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
