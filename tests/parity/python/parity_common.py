from __future__ import annotations

import argparse
import json
import os
import shutil
import tempfile
from contextlib import contextmanager
from pathlib import Path
from typing import Any


REFERENCE_REPO = os.environ.get(
    "REFERENCE_REPO", "https://github.com/NousResearch/hermes-agent.git"
)
REFERENCE_REF = os.environ.get("REFERENCE_REF", "main")

FAKE_ENV = {
    "OPENAI_API_KEY": "sk-hermes-parity-openai",
    "ANTHROPIC_API_KEY": "sk-ant-hermes-parity",
    "OPENROUTER_API_KEY": "sk-or-hermes-parity",
    "HERMES_PARITY": "1",
    "NO_COLOR": "1",
    "TERM": "dumb",
}


def parse_out_arg() -> Path:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    return Path(args.out)


def source(script: str) -> dict[str, str]:
    return {
        "repository": REFERENCE_REPO,
        "ref": REFERENCE_REF,
        "script": f"/parity/{script}",
    }


def fixture(script: str, cases: list[dict[str, Any]]) -> dict[str, Any]:
    return {"source": source(script), "cases": stable(cases)}


def write_fixture(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, indent=2, sort_keys=True)
        handle.write("\n")


def stable(value: Any) -> Any:
    if isinstance(value, dict):
        return {str(k): stable(value[k]) for k in sorted(value)}
    if isinstance(value, list):
        return [stable(item) for item in value]
    if isinstance(value, tuple):
        return [stable(item) for item in value]
    return value


def redact(value: Any) -> Any:
    if value is None:
        return None
    text = str(value)
    if not text:
        return ""
    return "<redacted>"


def normalize_timestamps(value: Any) -> Any:
    if isinstance(value, dict):
        out = {}
        for key, item in value.items():
            if key.endswith("_at") or key in {"timestamp", "last_active"}:
                out[key] = "<timestamp>" if item is not None else None
            else:
                out[key] = normalize_timestamps(item)
        return out
    if isinstance(value, list):
        return [normalize_timestamps(item) for item in value]
    return value


@contextmanager
def isolated_hermes_home():
    temp_dir = Path(tempfile.mkdtemp(prefix="hermes-parity-"))
    old_env = os.environ.copy()
    try:
        os.environ.update(FAKE_ENV)
        os.environ["HERMES_HOME"] = str(temp_dir)
        yield temp_dir
    finally:
        os.environ.clear()
        os.environ.update(old_env)
        shutil.rmtree(temp_dir, ignore_errors=True)
