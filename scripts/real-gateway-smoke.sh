#!/usr/bin/env bash
set -euo pipefail

if [[ "${HERMES_REAL_GATEWAY_SMOKE:-0}" != "1" ]]; then
  echo "real-gateway smoke skipped; set HERMES_REAL_GATEWAY_SMOKE=1 to opt in"
  exit 0
fi

command -v curl >/dev/null 2>&1 || {
  echo "curl is required for real-gateway smoke" >&2
  exit 2
}

: "${HERMES_REAL_GATEWAY_WEBHOOK_URL:?set HERMES_REAL_GATEWAY_WEBHOOK_URL}"

out_dir="${HERMES_REAL_GATEWAY_SMOKE_DIR:-target/real-gateway-smoke}"
mkdir -p "${out_dir}"
request_path="${out_dir}/request.json"
response_path="${out_dir}/response.txt"
status_path="${out_dir}/status.txt"

cat >"${request_path}" <<JSON
{
  "source": "hermes-rust-smoke",
  "text": "hermes-smoke-ok",
  "timestamp": "1970-01-01T00:00:00Z"
}
JSON

curl_config="$(mktemp)"
trap 'rm -f "${curl_config}"' EXIT
cat >"${curl_config}" <<CURL
header = "Content-Type: application/json"
CURL
if [[ -n "${HERMES_REAL_GATEWAY_BEARER_TOKEN:-}" ]]; then
  printf 'header = "Authorization: Bearer %s"\n' "${HERMES_REAL_GATEWAY_BEARER_TOKEN}" >>"${curl_config}"
fi

status="$(
  curl -sS \
    -o "${response_path}" \
    -w "%{http_code}" \
    -X POST "${HERMES_REAL_GATEWAY_WEBHOOK_URL}" \
    --config "${curl_config}" \
    --data-binary @"${request_path}"
)"
printf '%s\n' "${status}" >"${status_path}"

case "${status}" in
  2*|3*) ;;
  *)
    echo "real-gateway smoke failed with HTTP ${status}; response saved to ${response_path}" >&2
    exit 1
    ;;
esac

echo "real-gateway smoke passed; sanitized artifacts in ${out_dir}"
