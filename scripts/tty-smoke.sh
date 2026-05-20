#!/usr/bin/env bash
set -euo pipefail

if ! command -v tmux >/dev/null 2>&1; then
  echo "tty smoke skipped; tmux is not installed"
  exit 0
fi

out_dir="${HERMES_TTY_SMOKE_DIR:-target/tty-smoke}"
mkdir -p "${out_dir}"
capture_path="${out_dir}/rust-cli-help.txt"
session="hermes-rust-tty-smoke-$$"
marker="__HERMES_TTY_DONE__"

cleanup() {
  tmux kill-session -t "${session}" >/dev/null 2>&1 || true
}
trap cleanup EXIT

cargo build -p hermes-cli --bin hermes >/dev/null

tmux new-session -d -s "${session}" -x 100 -y 30 \
  "target/debug/hermes --help; printf '\\n%s\\n' '${marker}'; sleep 2"

for _ in $(seq 1 50); do
  tmux capture-pane -p -t "${session}" >"${capture_path}" || true
  if grep -q "${marker}" "${capture_path}"; then
    break
  fi
  sleep 0.1
done

grep -q "${marker}" "${capture_path}" || {
  echo "tty smoke failed: command did not complete" >&2
  exit 1
}
grep -q "Hermes Agent" "${capture_path}" || {
  echo "tty smoke failed: missing Hermes Agent marker" >&2
  exit 1
}
grep -q "Commands:" "${capture_path}" || {
  echo "tty smoke failed: missing Commands marker" >&2
  exit 1
}
grep -q "config" "${capture_path}" || {
  echo "tty smoke failed: missing config marker" >&2
  exit 1
}
grep -q "gateway" "${capture_path}" || {
  echo "tty smoke failed: missing gateway marker" >&2
  exit 1
}

echo "tty smoke passed; captured output in ${capture_path}"
