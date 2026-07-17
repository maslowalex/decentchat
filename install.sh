#!/bin/sh
set -eu

REPOSITORY="${DECENTCHAT_REPOSITORY:-maslowalex/decentchat}"
INSTALL_DIR="${DECENTCHAT_INSTALL_DIR:-$HOME/.local/bin}"

case "$(uname -s)" in
    Linux) os="unknown-linux-gnu" ;;
    Darwin) os="apple-darwin" ;;
    *)
        echo "decentchat: unsupported operating system: $(uname -s)" >&2
        exit 1
        ;;
esac

case "$(uname -m)" in
    x86_64|amd64) arch="x86_64" ;;
    arm64|aarch64) arch="aarch64" ;;
    *)
        echo "decentchat: unsupported CPU architecture: $(uname -m)" >&2
        exit 1
        ;;
esac

target="${arch}-${os}"
archive="decentchat-${target}.tar.gz"
release_url="https://github.com/${REPOSITORY}/releases/latest/download"
temp_dir="$(mktemp -d 2>/dev/null || mktemp -d -t decentchat)"
trap 'rm -rf "$temp_dir"' EXIT HUP INT TERM

echo "Downloading DecentChat for ${target}..."
curl --proto '=https' --tlsv1.2 -fLsS \
    "${release_url}/${archive}" -o "${temp_dir}/${archive}"
curl --proto '=https' --tlsv1.2 -fLsS \
    "${release_url}/SHA256SUMS" -o "${temp_dir}/SHA256SUMS"

expected="$(awk -v asset="$archive" '$2 == asset { print $1 }' "${temp_dir}/SHA256SUMS")"
if [ -z "$expected" ]; then
    echo "decentchat: release checksum is missing for ${archive}" >&2
    exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "${temp_dir}/${archive}" | awk '{ print $1 }')"
elif command -v shasum >/dev/null 2>&1; then
    actual="$(shasum -a 256 "${temp_dir}/${archive}" | awk '{ print $1 }')"
else
    echo "decentchat: sha256sum or shasum is required to verify the download" >&2
    exit 1
fi

if [ "$actual" != "$expected" ]; then
    echo "decentchat: checksum verification failed for ${archive}" >&2
    exit 1
fi

tar -xzf "${temp_dir}/${archive}" -C "$temp_dir"
mkdir -p "$INSTALL_DIR"
install -m 755 "${temp_dir}/decentchat" "${INSTALL_DIR}/decentchat"

echo "Installed DecentChat to ${INSTALL_DIR}/decentchat"
case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
        echo "Add ${INSTALL_DIR} to PATH, then run: decentchat --version"
        ;;
esac
