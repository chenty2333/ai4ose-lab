#!/usr/bin/env bash
# Extract all submodule directories from the tar.gz archive
# Restores the submodule contents for development

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUNDLE_DIR="${ROOT_DIR}/bundle"
ARCHIVE_NAME="submodules.tar.gz"
ARCHIVE_PATH="${BUNDLE_DIR}/${ARCHIVE_NAME}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Extracting Submodules ===${NC}"
echo "Root directory: ${ROOT_DIR}"

# Check if archive exists
if [[ ! -f "${ARCHIVE_PATH}" ]]; then
    echo -e "${RED}Error: Archive not found at ${ARCHIVE_PATH}${NC}"
    echo ""
    echo "Please ensure the bundle archive exists. You may need to:"
    echo "  1. Run scripts/compress_submodules.sh to create the archive, or"
    echo "  2. Download the crate from crates.io which includes the bundle"
    exit 1
fi

# Show archive info
ARCHIVE_SIZE=$(du -h "${ARCHIVE_PATH}" | cut -f1)
echo -e "${BLUE}Archive: ${ARCHIVE_PATH}${NC}"
echo -e "${BLUE}Size: ${ARCHIVE_SIZE}${NC}"

# Ask for confirmation if any crate directories already exist
EXISTING=()
while IFS= read -r line; do
    [[ -z "${line}" ]] && continue
    # Remove leading ./ from tar listing if present
    crate="${line#./}"
    if [[ -d "${ROOT_DIR}/${crate}" ]]; then
        EXISTING+=("${crate}")
    fi
done < <(tar -tzf "${ARCHIVE_PATH}" | grep -E '^[^/]+/?$' | sed 's|/$||')

if [[ ${#EXISTING[@]} -gt 0 ]]; then
    echo ""
    echo -e "${YELLOW}Warning: The following crate directories already exist:${NC}"
    for crate in "${EXISTING[@]}"; do
        echo "  - ${crate}"
    done
    echo ""
    echo -e "${YELLOW}These directories will be overwritten.${NC}"
    
    # Remove existing directories
    for crate in "${EXISTING[@]}"; do
        echo "Removing: ${crate}"
        rm -rf "${ROOT_DIR}/${crate}"
    done
fi

# Extract archive
echo ""
echo -e "${GREEN}Extracting archive...${NC}"
cd "${ROOT_DIR}"
tar -xzf "${ARCHIVE_PATH}"

echo ""
echo -e "${GREEN}=== Extraction Complete ===${NC}"
echo "All submodules have been extracted to: ${ROOT_DIR}"
echo ""
echo "You can now:"
echo "  - Build individual chapters: cd tg-rcore-tutorial-ch1 && cargo build"
echo "  - Develop components in their respective directories"
echo "  - Run tests in each crate directory"
