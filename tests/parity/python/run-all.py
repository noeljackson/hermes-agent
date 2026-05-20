from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)

    parity_dir = Path(__file__).resolve().parent
    scripts = sorted(
        path
        for path in parity_dir.glob("*-fixture.py")
        if path.is_file()
    )

    env = os.environ.copy()
    env.setdefault("PYTHONPATH", "/reference/hermes-agent")
    env.setdefault("HERMES_PARITY_DOCKER", "1")

    for script in scripts:
        fixture_path = out_dir / f"{script.stem}.json"
        cmd = [sys.executable, str(script), "--out", str(fixture_path)]
        subprocess.run(cmd, cwd="/reference/hermes-agent", env=env, check=True)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
