#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
CRATES_FILE="${SCRIPT_DIR}/crates.txt"
BUNDLE_DIR="${ROOT_DIR}/bundle"
ARCHIVE_PATH="${BUNDLE_DIR}/apps.tar.gz"

# Allow maintainers to point to a source tree outside ROOT_DIR.
SOURCE_ROOT="${SOURCE_ROOT:-${ROOT_DIR}}"

if [[ ! -f "${CRATES_FILE}" ]]; then
    echo "error: crates list not found: ${CRATES_FILE}"
    exit 1
fi

mkdir -p "${BUNDLE_DIR}"

work_dir="$(mktemp -d)"
trap 'rm -rf "${work_dir}"' EXIT

declare -a crates=()
while IFS= read -r crate || [[ -n "${crate}" ]]; do
    crate="${crate%$'\r'}"
    [[ -z "${crate}" ]] && continue
    crates+=("${crate}")
done < "${CRATES_FILE}"

if [[ "${#crates[@]}" -eq 0 ]]; then
    echo "error: no crate names found in ${CRATES_FILE}"
    exit 1
fi

declare -a missing=()
for crate in "${crates[@]}"; do
    if [[ -d "${SOURCE_ROOT}/${crate}" ]]; then
        cp -a "${SOURCE_ROOT}/${crate}" "${work_dir}/${crate}"
    elif [[ -d "${ROOT_DIR}/${crate}" ]]; then
        cp -a "${ROOT_DIR}/${crate}" "${work_dir}/${crate}"
    else
        missing+=("${crate}")
    fi
done

if [[ "${#missing[@]}" -gt 0 ]]; then
    echo "error: missing crates:"
    for crate in "${missing[@]}"; do
        echo "  - ${crate}"
    done
    echo ""
    echo "hint: set SOURCE_ROOT to the directory containing app-* crates."
    exit 1
fi

rm -f "${ARCHIVE_PATH}"
(
    cd "${work_dir}"
    tar -czf "${ARCHIVE_PATH}" \
        --exclude='.git' \
        --exclude='target' \
        --exclude='*.o' \
        --exclude='*.a' \
        .
)

echo "created archive: ${ARCHIVE_PATH}"
du -h "${ARCHIVE_PATH}" | awk '{print "size: " $1}'
