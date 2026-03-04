#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"

VERSION="${1:-}"
TARGET="${2:-x86_64-unknown-linux-gnu}"
OUT_DIR="${3:-${PROJECT_DIR}/dist}"
DEB_ARCH="${4:-amd64}"

if [[ -z "${VERSION}" ]]; then
  VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "${PROJECT_DIR}/Cargo.toml" | head -n1)"
fi
VERSION="${VERSION#v}"
[[ -n "${VERSION}" ]] || {
  echo "ERROR: failed to resolve package version" >&2
  exit 1
}

command -v cargo >/dev/null 2>&1 || {
  echo "ERROR: cargo command not found" >&2
  exit 1
}

if ! cargo deb --help >/dev/null 2>&1; then
  cargo install cargo-deb --locked
fi

mkdir -p "${OUT_DIR}"
cd "${PROJECT_DIR}"

cargo build --release --locked --target "${TARGET}"

TARGET_BIN="target/${TARGET}/release/markwatch"
[[ -x "${TARGET_BIN}" ]] || {
  echo "ERROR: built binary missing: ${TARGET_BIN}" >&2
  exit 1
}

# cargo-deb asset path is static. Sync selected target binary to target/release for packaging.
mkdir -p target/release
cp "${TARGET_BIN}" target/release/markwatch

OUTPUT_PATH="${OUT_DIR}/markwatch_${VERSION}_${DEB_ARCH}.deb"
cargo deb \
  --target "${TARGET}" \
  --no-build \
  --deb-version "${VERSION}" \
  --output "${OUTPUT_PATH}"

echo "Created ${OUTPUT_PATH}"
