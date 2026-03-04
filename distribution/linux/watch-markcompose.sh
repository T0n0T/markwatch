#!/usr/bin/env bash
set -euo pipefail

MARKWATCH_BIN="${MARKWATCH_BIN:-/usr/bin/markwatch}"
COMPOSE_DIR="${COMPOSE_DIR:-}"
ENV_FILE="${ENV_FILE:-}"
MARKDOWN_DIR="${MARKDOWN_DIR:-}"
DEBOUNCE_MS="${DEBOUNCE_MS:-800}"
RECONCILE_SEC="${RECONCILE_SEC:-600}"
LOG_LEVEL="${LOG_LEVEL:-info}"
WATCH_SHELL="${WATCH_SHELL:-sh}"
BUILD_CMD="${BUILD_CMD:-}"

die() {
  echo "ERROR: $*" >&2
  exit 1
}

[[ -x "${MARKWATCH_BIN}" ]] || die "markwatch binary not executable: ${MARKWATCH_BIN}"
[[ -n "${COMPOSE_DIR}" ]] || die "COMPOSE_DIR is required in /etc/default/markwatch"
[[ -d "${COMPOSE_DIR}" ]] || die "COMPOSE_DIR not found: ${COMPOSE_DIR}"

if [[ -z "${ENV_FILE}" ]]; then
  ENV_FILE="${COMPOSE_DIR}/.env.runtime"
fi

if [[ -z "${MARKDOWN_DIR}" ]]; then
  if [[ ! -f "${ENV_FILE}" ]]; then
    die "ENV_FILE not found and MARKDOWN_DIR not provided: ${ENV_FILE}"
  fi
  MARKDOWN_DIR="$(awk -F= '/^MARKDOWN_DIR=/{print substr($0, index($0,$2)); exit}' "${ENV_FILE}")"
fi

[[ -n "${MARKDOWN_DIR}" ]] || die "MARKDOWN_DIR is empty"
[[ -d "${MARKDOWN_DIR}" ]] || die "MARKDOWN_DIR not found: ${MARKDOWN_DIR}"
[[ "${DEBOUNCE_MS}" =~ ^[0-9]+$ ]] || die "DEBOUNCE_MS must be numeric: ${DEBOUNCE_MS}"
[[ "${RECONCILE_SEC}" =~ ^[0-9]+$ ]] || die "RECONCILE_SEC must be numeric: ${RECONCILE_SEC}"

if [[ -z "${BUILD_CMD}" ]]; then
  BUILD_CMD="docker compose --env-file \"${ENV_FILE}\" run --rm --no-deps hugo-builder"
fi

exec "${MARKWATCH_BIN}" \
  --root "${MARKDOWN_DIR}" \
  --workdir "${COMPOSE_DIR}" \
  --cmd "${BUILD_CMD}" \
  --shell "${WATCH_SHELL}" \
  --debounce-ms "${DEBOUNCE_MS}" \
  --reconcile-sec "${RECONCILE_SEC}" \
  --log-level "${LOG_LEVEL}"
