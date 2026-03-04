#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"

VERSION="${1:-}"
TARGET="${2:-x86_64-unknown-linux-gnu}"
OUT_DIR="${3:-${PROJECT_DIR}/dist}"

if [[ -z "${VERSION}" ]]; then
  VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "${PROJECT_DIR}/Cargo.toml" | head -n1)"
fi
VERSION="${VERSION#v}"
[[ -n "${VERSION}" ]] || {
  echo "ERROR: failed to resolve package version" >&2
  exit 1
}

BIN_PATH="${PROJECT_DIR}/target/${TARGET}/release/mdwatch"
if [[ ! -x "${BIN_PATH}" ]]; then
  BIN_PATH="${PROJECT_DIR}/target/release/mdwatch"
fi
[[ -x "${BIN_PATH}" ]] || {
  echo "ERROR: release binary not found for target ${TARGET}: ${BIN_PATH}" >&2
  exit 1
}

STAGE_DIR="${OUT_DIR}/mdwatch-${VERSION}-${TARGET}"
ARCHIVE="${OUT_DIR}/mdwatch-${VERSION}-${TARGET}.tar.gz"

rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/bin" "${STAGE_DIR}/distribution/linux"

install -m 755 "${BIN_PATH}" "${STAGE_DIR}/bin/mdwatch"
install -m 755 "${PROJECT_DIR}/distribution/linux/watch-docker-compose.sh" "${STAGE_DIR}/distribution/linux/watch-docker-compose.sh"
install -m 644 "${PROJECT_DIR}/distribution/linux/mdwatch.service" "${STAGE_DIR}/distribution/linux/mdwatch.service"
install -m 644 "${PROJECT_DIR}/distribution/linux/mdwatch.env" "${STAGE_DIR}/distribution/linux/mdwatch.env"
install -m 644 "${PROJECT_DIR}/README.md" "${STAGE_DIR}/README.md"

mkdir -p "${OUT_DIR}"
tar -C "${OUT_DIR}" -czf "${ARCHIVE}" "$(basename "${STAGE_DIR}")"
echo "Created ${ARCHIVE}"
