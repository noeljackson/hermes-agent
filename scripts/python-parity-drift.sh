#!/usr/bin/env bash
set -euo pipefail

REFERENCE_REPO="${REFERENCE_REPO:-https://github.com/NousResearch/hermes-agent.git}"
REFERENCE_REF="${REFERENCE_REF:-main}"
PARITY_FIXTURES_DIR="${PARITY_FIXTURES_DIR:-tests/fixtures/python-parity}"
PARITY_IMAGE="${PARITY_IMAGE:-hermes-python-parity}"
DRIFT_DIR="${DRIFT_DIR:-target/python-parity-drift}"
FRESH_FIXTURES_DIR="${DRIFT_DIR}/fixtures"
DIFF_FILE="${DRIFT_DIR}/fixture.diff"
BRIEF_FILE="${DRIFT_DIR}/brief.md"

rm -rf "${DRIFT_DIR}"
mkdir -p "${FRESH_FIXTURES_DIR}"

docker build -f Dockerfile.python-parity \
	--build-arg REFERENCE_REPO="${REFERENCE_REPO}" \
	--build-arg REFERENCE_REF="${REFERENCE_REF}" \
	-t "${PARITY_IMAGE}" .

docker run --rm \
	-v "${PWD}/${FRESH_FIXTURES_DIR}:/fixtures" \
	"${PARITY_IMAGE}"

if diff -ru "${PARITY_FIXTURES_DIR}" "${FRESH_FIXTURES_DIR}" > "${DIFF_FILE}"; then
	cat > "${BRIEF_FILE}" <<EOF
No Python parity drift detected.

Reference:
${REFERENCE_REPO}@${REFERENCE_REF}
EOF
	exit 0
fi

cat > "${BRIEF_FILE}" <<EOF
Python parity drift detected.

Reference:
${REFERENCE_REPO}@${REFERENCE_REF}

Rules:
- Do not run Python on host.
- Regenerate fixtures only through Docker.
- Update Rust behavior and committed fixtures together.
- Use fake credentials only.

Diff:
EOF

cat "${DIFF_FILE}" >> "${BRIEF_FILE}"

echo "Python parity drift detected. See ${DIFF_FILE} and ${BRIEF_FILE}." >&2
exit 1
