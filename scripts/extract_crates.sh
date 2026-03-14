#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
CRATES_FILE="${SCRIPT_DIR}/crates.txt"
ARCHIVE_PATH="${ROOT_DIR}/bundle/apps.tar.gz"

if [[ ! -f "${ARCHIVE_PATH}" ]]; then
    echo "error: archive not found: ${ARCHIVE_PATH}"
    echo "hint: run scripts/compress_crates.sh first, or use a packaged crate that contains bundle/apps.tar.gz."
    exit 1
fi

if [[ ! -f "${CRATES_FILE}" ]]; then
    echo "error: crates list not found: ${CRATES_FILE}"
    exit 1
fi

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

for crate in "${crates[@]}"; do
    if [[ -e "${ROOT_DIR}/${crate}" ]]; then
        echo "remove existing: ${crate}"
        rm -rf "${ROOT_DIR:?}/${crate}"
    fi
done

echo "extracting ${ARCHIVE_PATH} -> ${ROOT_DIR}"
tar -xzf "${ARCHIVE_PATH}" -C "${ROOT_DIR}"

echo ""
echo "extraction complete. available crates:"
for crate in "${crates[@]}"; do
    if [[ -d "${ROOT_DIR}/${crate}" ]]; then
        echo "  - ${crate}"
    else
        echo "  - ${crate} (missing after extract)"
    fi
done

echo ""
echo "you can now enter any crate directory and run cargo commands."
