#!/usr/bin/env bash
set -euo pipefail

REPO="al-ula/repin"
GITHUB_BASE="https://github.com/${REPO}"
GITHUB_API="https://api.github.com/repos/${REPO}/releases/latest"

echo "Repin Installer & Setup"
echo "======================="

# Detect the host target supported by the release channel.
OS="$(uname -s)"
ARCH="$(uname -m)"

detect_target() {
    case "${OS}:${ARCH}" in
        Linux:x86_64)
            printf '%s\n' 'x86_64-unknown-linux-gnu'
            ;;
        *)
            echo "Error: Repin has no default release target for ${OS} ${ARCH}." >&2
            echo "Set REPIN_TARGET to a published compatible target archive." >&2
            return 1
            ;;
    esac
}

TARGET="${REPIN_TARGET:-$(detect_target)}"

# Check required utilities
for cmd in curl tar gzip; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "Error: Required command '$cmd' is not installed." >&2
        exit 1
    fi
done

# Resolve release version
if [ -n "${REPIN_VERSION:-}" ]; then
    TAG="${REPIN_VERSION}"
    if [[ "$TAG" != v* ]]; then
        TAG="v${TAG}"
    fi
    echo "Using specified version: ${TAG}"
else
    echo "Fetching latest release information..."
    TAG="$(curl -sSfL -H "Accept: application/vnd.github.v3+json" -H "User-Agent: repin-installer" "${GITHUB_API}" 2>/dev/null | grep '"tag_name":' | head -n 1 | sed -E 's/.*"tag_name":\s*"([^"]+)".*/\1/' || true)"
    if [ -z "$TAG" ]; then
        echo "Error: Failed to fetch latest release tag from GitHub." >&2
        exit 1
    fi
    echo "Latest release: ${TAG}"
fi

ARCHIVE_NAME="repin-${TAG}-${TARGET}.tar.gz"
DOWNLOAD_URL="${GITHUB_BASE}/releases/download/${TAG}/${ARCHIVE_NAME}"

# Create temporary directory for download and extraction
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TEMP_DIR}"' EXIT

echo "Downloading ${ARCHIVE_NAME}..."
curl -sSfL -H "User-Agent: repin-installer" -o "${TEMP_DIR}/${ARCHIVE_NAME}" "${DOWNLOAD_URL}"

echo "Extracting package..."
tar -xzf "${TEMP_DIR}/${ARCHIVE_NAME}" -C "${TEMP_DIR}"

NEW_BIN=""
if [ -f "${TEMP_DIR}/repin" ]; then
    NEW_BIN="${TEMP_DIR}/repin"
else
    NEW_BIN="$(find "${TEMP_DIR}" -maxdepth 2 -type f -name "repin" | head -n 1)"
fi

if [ -z "${NEW_BIN}" ] || [ ! -f "${NEW_BIN}" ]; then
    echo "Error: Could not locate 'repin' binary in extracted archive." >&2
    exit 1
fi

chmod +x "${NEW_BIN}"

echo "Validating target ${TARGET}..."
VERSION_INFO="$("${NEW_BIN}" version --json 2>/dev/null)" || {
    echo "Error: Downloaded Repin binary cannot run on ${OS} ${ARCH}." >&2
    exit 1
}
if ! grep -Fq "\"target\": \"${TARGET}\"" <<<"${VERSION_INFO}"; then
    echo "Error: Downloaded binary target does not match ${TARGET}." >&2
    exit 1
fi

# Stop active daemon if running
if command -v repin >/dev/null 2>&1; then
    echo "Stopping active daemon if running..."
    repin stop 2>/dev/null || true
fi

echo "Installing Repin..."
"${NEW_BIN}" install

echo ""
echo "Installation complete!"
