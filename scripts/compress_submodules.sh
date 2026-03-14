#!/usr/bin/env bash
# Compress all submodule directories into a single tar.gz archive
# Excludes .git directories to reduce size and avoid git-related issues
#
# Prerequisites:
#   - Run 'git submodule update --init --recursive' first to fetch all submodules
#   - Or ensure all submodule directories contain actual source code

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUNDLE_DIR="${ROOT_DIR}/bundle"
ARCHIVE_NAME="submodules.tar.gz"
ARCHIVE_PATH="${BUNDLE_DIR}/${ARCHIVE_NAME}"
CRATES_FILE="${SCRIPT_DIR}/crates.txt"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Compressing Submodules ===${NC}"
echo "Root directory: ${ROOT_DIR}"

# Create bundle directory if it doesn't exist
mkdir -p "${BUNDLE_DIR}"

# Check if crates.txt exists
if [[ ! -f "${CRATES_FILE}" ]]; then
    echo -e "${RED}Error: crates.txt not found at ${CRATES_FILE}${NC}"
    exit 1
fi

# Read crate names from crates.txt (handle both Unix and Windows line endings)
CRATES=()
while IFS= read -r crate || [[ -n "${crate}" ]]; do
    # Remove trailing carriage return (Windows line ending)
    crate="${crate%$'\r'}"
    [[ -z "${crate}" ]] && continue
    CRATES+=("${crate}")
done < "${CRATES_FILE}"

echo "Found ${#CRATES[@]} crates listed in crates.txt"

# Verify all crate directories exist and have content
MISSING=()
EMPTY=()
READY=()
for crate in "${CRATES[@]}"; do
    if [[ ! -d "${ROOT_DIR}/${crate}" ]]; then
        MISSING+=("${crate}")
    elif [[ -z "$(ls -A "${ROOT_DIR}/${crate}" 2>/dev/null | grep -v '^\.git$')" ]]; then
        EMPTY+=("${crate}")
    else
        READY+=("${crate}")
    fi
done

# Report status
echo ""
echo -e "${BLUE}Status Summary:${NC}"
echo "  Ready to compress: ${#READY[@]}"
echo "  Empty directories: ${#EMPTY[@]}"
echo "  Missing directories: ${#MISSING[@]}"

# If there are missing or empty directories, provide guidance
if [[ ${#MISSING[@]} -gt 0 ]] || [[ ${#EMPTY[@]} -gt 0 ]]; then
    echo ""
    if [[ ${#MISSING[@]} -gt 0 ]]; then
        echo -e "${YELLOW}Missing directories:${NC}"
        for crate in "${MISSING[@]}"; do
            echo "  - ${crate}"
        done
    fi
    if [[ ${#EMPTY[@]} -gt 0 ]]; then
        echo -e "${YELLOW}Empty directories (submodule not checked out):${NC}"
        for crate in "${EMPTY[@]}"; do
            echo "  - ${crate}"
        done
    fi
    
    echo ""
    echo -e "${YELLOW}Hint: Please run the following command to fetch submodules:${NC}"
    echo "  cd ${ROOT_DIR} && git submodule update --init --recursive"
    echo ""
    echo -e "${YELLOW}Continuing with ${#READY[@]} available crates...${NC}"
fi

# Check if we have anything to compress
if [[ ${#READY[@]} -eq 0 ]]; then
    echo -e "${RED}Error: No crates available to compress${NC}"
    echo "Please ensure submodules are checked out before running this script."
    exit 1
fi

# Remove old archive if exists
if [[ -f "${ARCHIVE_PATH}" ]]; then
    echo -e "${YELLOW}Removing existing archive...${NC}"
    rm -f "${ARCHIVE_PATH}"
fi

# Create temporary file list
TEMP_FILE_LIST=$(mktemp)
trap "rm -f ${TEMP_FILE_LIST}" EXIT

# Build the list of directories to compress (only ready ones)
for crate in "${READY[@]}"; do
    echo "${crate}" >> "${TEMP_FILE_LIST}"
done

# Create the archive using tar with exclude for .git directories
echo ""
echo -e "${GREEN}Creating archive: ${ARCHIVE_PATH}${NC}"
echo "Excluding: .git, *.o, *.a, target/"

# Use tar to create archive, excluding .git directories
# We need to change to ROOT_DIR and use relative paths
cd "${ROOT_DIR}"

# Create tarball excluding .git directories
tar -czvf "${ARCHIVE_PATH}" \
    --exclude='.git' \
    --exclude='*.o' \
    --exclude='*.a' \
    --exclude='target' \
    -T "${TEMP_FILE_LIST}" 2>&1 | tail -5

# Get archive size
ARCHIVE_SIZE=$(du -h "${ARCHIVE_PATH}" | cut -f1)

echo ""
echo -e "${GREEN}=== Compression Complete ===${NC}"
echo "Archive: ${ARCHIVE_PATH}"
echo "Size: ${ARCHIVE_SIZE}"
echo "Crates included: ${#READY[@]} / ${#CRATES[@]}"

if [[ ${#READY[@]} -lt ${#CRATES[@]} ]]; then
    echo ""
    echo -e "${YELLOW}Note: Some crates were not included.${NC}"
    echo "Run 'git submodule update --init --recursive' to fetch missing submodules,"
    echo "then re-run this script to include all crates."
fi
