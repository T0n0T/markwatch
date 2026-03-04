#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"

LEGACY_VERSION="${1:-}"
TARGET="${2:-x86_64-unknown-linux-gnu}"
OUT_DIR="${3:-${PROJECT_DIR}/dist}"
# Kept for backward compatibility with older callers.

BIN_PATH="${PROJECT_DIR}/target/${TARGET}/release/markwatch"
if [[ ! -x "${BIN_PATH}" ]]; then
  BIN_PATH="${PROJECT_DIR}/target/release/markwatch"
fi
[[ -x "${BIN_PATH}" ]] || {
  echo "ERROR: release binary not found for target ${TARGET}: ${BIN_PATH}" >&2
  exit 1
}

STAGE_DIR="${OUT_DIR}/markwatch-${TARGET}"
ARCHIVE="${OUT_DIR}/markwatch-${TARGET}.tar.gz"

rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/bin" "${STAGE_DIR}/distribution/linux"

install -m 755 "${BIN_PATH}" "${STAGE_DIR}/bin/markwatch"
install -m 755 "${PROJECT_DIR}/distribution/linux/watch-markcompose.sh" "${STAGE_DIR}/distribution/linux/watch-markcompose.sh"
install -m 644 "${PROJECT_DIR}/distribution/linux/markwatch.service" "${STAGE_DIR}/distribution/linux/markwatch.service"
install -m 644 "${PROJECT_DIR}/distribution/linux/markwatch.env" "${STAGE_DIR}/distribution/linux/markwatch.env"
install -m 644 "${PROJECT_DIR}/README.md" "${STAGE_DIR}/README.md"

mkdir -p "${OUT_DIR}"
tar -C "${OUT_DIR}" -czf "${ARCHIVE}" "$(basename "${STAGE_DIR}")"
echo "Created ${ARCHIVE}"
