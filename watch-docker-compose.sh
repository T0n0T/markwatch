#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

COMPOSE_DIR="${1:-${COMPOSE_DIR:-}}"
ENV_FILE="${2:-${ENV_FILE:-}}"
DEBOUNCE_MS="${3:-${DEBOUNCE_MS:-}}"
RECONCILE_SEC="${4:-${RECONCILE_SEC:-}}"
LOG_LEVEL="${5:-${LOG_LEVEL:-info}}"

usage() {
  cat <<'EOF'
Usage:
  watch-docker-compose.sh [compose_dir] [env_file] [debounce_ms] [reconcile_sec] [log_level]

Examples:
  ./watch-docker-compose.sh /srv/blog/docker-compose /srv/blog/docker-compose/.env.runtime 1000 600
  COMPOSE_DIR=/srv/blog/docker-compose ENV_FILE=/srv/blog/docker-compose/.env.runtime DEBOUNCE_MS=1000 RECONCILE_SEC=600 ./watch-docker-compose.sh
EOF
}

die() {
  echo "ERROR: $*" >&2
  exit 1
}

if (( $# > 5 )); then
  usage
  exit 1
fi

if [[ -z "${COMPOSE_DIR}" ]]; then
  die "COMPOSE_DIR is required (arg1 or env COMPOSE_DIR)"
fi
if [[ -z "${ENV_FILE}" ]]; then
  ENV_FILE="${COMPOSE_DIR}/.env.runtime"
fi
if [[ -z "${DEBOUNCE_MS}" ]]; then
  die "DEBOUNCE_MS is required (arg3 or env DEBOUNCE_MS)"
fi
if [[ -z "${RECONCILE_SEC}" ]]; then
  die "RECONCILE_SEC is required (arg4 or env RECONCILE_SEC)"
fi

[[ "${DEBOUNCE_MS}" =~ ^[0-9]+$ ]] || die "DEBOUNCE_MS must be numeric: ${DEBOUNCE_MS}"
[[ "${RECONCILE_SEC}" =~ ^[0-9]+$ ]] || die "RECONCILE_SEC must be numeric: ${RECONCILE_SEC}"

[[ -d "${COMPOSE_DIR}" ]] || die "compose dir not found: ${COMPOSE_DIR}"
[[ -f "${ENV_FILE}" ]] || die "env file not found: ${ENV_FILE} (run start.sh first)"

MARKDOWN_DIR="$(awk -F= '/^MARKDOWN_DIR=/{print substr($0, index($0,$2)); exit}' "${ENV_FILE}")"
[[ -n "${MARKDOWN_DIR}" ]] || die "MARKDOWN_DIR not found in ${ENV_FILE}"
[[ -d "${MARKDOWN_DIR}" ]] || die "MARKDOWN_DIR does not exist: ${MARKDOWN_DIR}"

if [[ -x "${SCRIPT_DIR}/target/release/mdwatch" ]]; then
  BIN="${SCRIPT_DIR}/target/release/mdwatch"
elif [[ -x "${SCRIPT_DIR}/target/debug/mdwatch" ]]; then
  BIN="${SCRIPT_DIR}/target/debug/mdwatch"
else
  echo "Binary not found, building release binary..."
  (cd "${SCRIPT_DIR}" && cargo build --release)
  BIN="${SCRIPT_DIR}/target/release/mdwatch"
fi

echo "Starting mdwatch:"
echo "  binary:      ${BIN}"
echo "  root:        ${MARKDOWN_DIR}"
echo "  workdir:     ${COMPOSE_DIR}"
echo "  env file:    ${ENV_FILE}"
echo "  debounce ms: ${DEBOUNCE_MS}"
echo "  reconcile s: ${RECONCILE_SEC}"
echo "  log level:   ${LOG_LEVEL}"

exec "${BIN}" \
  --root "${MARKDOWN_DIR}" \
  --workdir "${COMPOSE_DIR}" \
  --cmd "./build.sh ${ENV_FILE}" \
  --debounce-ms "${DEBOUNCE_MS}" \
  --reconcile-sec "${RECONCILE_SEC}" \
  --log-level "${LOG_LEVEL}"
