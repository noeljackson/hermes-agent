#!/usr/bin/env bash
set -euo pipefail

if [[ "${HERMES_REAL_PROVIDER_SMOKE:-0}" != "1" ]]; then
  echo "real-provider smoke skipped; set HERMES_REAL_PROVIDER_SMOKE=1 to opt in"
  exit 0
fi

command -v curl >/dev/null 2>&1 || {
  echo "curl is required for real-provider smoke" >&2
  exit 2
}

: "${HERMES_REAL_PROVIDER_BASE_URL:?set HERMES_REAL_PROVIDER_BASE_URL, for example https://api.openai.com/v1}"
: "${HERMES_REAL_PROVIDER_MODEL:?set HERMES_REAL_PROVIDER_MODEL}"
: "${HERMES_REAL_PROVIDER_API_KEY:?set HERMES_REAL_PROVIDER_API_KEY}"

out_dir="${HERMES_REAL_PROVIDER_SMOKE_DIR:-target/real-provider-smoke}"
mkdir -p "${out_dir}"
request_path="${out_dir}/request.json"
response_path="${out_dir}/response.json"
status_path="${out_dir}/status.txt"

cat >"${request_path}" <<JSON
{
  "model": "${HERMES_REAL_PROVIDER_MODEL}",
  "messages": [
    {
      "role": "user",
      "content": "Reply with exactly: hermes-smoke-ok"
    }
  ],
  "max_tokens": 16,
  "temperature": 0
}
JSON

url="${HERMES_REAL_PROVIDER_BASE_URL%/}/chat/completions"
curl_config="$(mktemp)"
trap 'rm -f "${curl_config}"' EXIT
cat >"${curl_config}" <<CURL
header = "Authorization: Bearer ${HERMES_REAL_PROVIDER_API_KEY}"
header = "Content-Type: application/json"
CURL
status="$(
  curl -sS \
    -o "${response_path}" \
    -w "%{http_code}" \
    -X POST "${url}" \
    --config "${curl_config}" \
    --data-binary @"${request_path}"
)"
printf '%s\n' "${status}" >"${status_path}"

case "${status}" in
  2*) ;;
  *)
    echo "real-provider smoke failed with HTTP ${status}; response saved to ${response_path}" >&2
    exit 1
    ;;
esac

if ! grep -Eq '"choices"|"content"|"id"' "${response_path}"; then
  echo "real-provider smoke response did not look like a chat completion; response saved to ${response_path}" >&2
  exit 1
fi

echo "real-provider smoke passed; sanitized artifacts in ${out_dir}"
