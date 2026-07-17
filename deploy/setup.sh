#!/bin/bash
set -e

echo "=== DecentChat Guardian Super-Peer Setup ==="
echo ""

# Check if running as root for system-wide installation
if [ "$EUID" -ne 0 ]; then
    echo "Please run as root (sudo ./setup.sh)"
    exit 1
fi

# Guardian DB 0.19 requires the toolchain pinned by the repository.
echo "Installing Rust 1.97 toolchain..."
su - "$ACTUAL_USER" -c "$ACTUAL_HOME/.cargo/bin/rustup toolchain install 1.97.0 --profile minimal"

# Get the actual user who invoked sudo
ACTUAL_USER=${SUDO_USER:-$USER}
ACTUAL_HOME=$(getent passwd "$ACTUAL_USER" | cut -d: -f6)

# Install Rust if missing
if ! su - "$ACTUAL_USER" -c "command -v cargo" &> /dev/null; then
    echo "Installing Rust..."
    su - "$ACTUAL_USER" -c "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
    CARGO_PATH="$ACTUAL_HOME/.cargo/bin/cargo"
else
    CARGO_PATH=$(su - "$ACTUAL_USER" -c "which cargo")
fi

# Build release binary
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"

echo "Building decentchat in release mode..."
cd "$REPO_DIR"
su - "$ACTUAL_USER" -c "cd '$REPO_DIR' && $CARGO_PATH build --release"

# Install binary
echo "Installing binary to /usr/local/bin/..."
cp "$REPO_DIR/target/release/decentchat" /usr/local/bin/
chmod 755 /usr/local/bin/decentchat

# Create config directory
echo "Setting up configuration..."
mkdir -p /etc/decentchat
if [ ! -f /etc/decentchat/relay.env ]; then
    cp "$SCRIPT_DIR/relay.env" /etc/decentchat/
fi
chown -R "$ACTUAL_USER:$ACTUAL_USER" /etc/decentchat

# Create persistent Guardian data directory (identity, blobs, docs, and stores)
mkdir -p /var/lib/decentchat
chown -R "$ACTUAL_USER:$ACTUAL_USER" /var/lib/decentchat

# Install systemd service
echo "Installing systemd service..."
sed "s/%USER%/$ACTUAL_USER/g" "$SCRIPT_DIR/decentchat-relay.service" > /etc/systemd/system/decentchat-relay.service
systemctl daemon-reload
systemctl enable decentchat-relay

# Generate identity if not present
echo "Generating node identity..."
su - "$ACTUAL_USER" -c "DECENTCHAT_CONFIG=/var/lib/decentchat /usr/local/bin/decentchat identity"

echo ""
echo "=== Setup Complete ==="
echo ""
echo "Next steps:"
echo "  1. Edit configuration: sudo nano /etc/decentchat/relay.env"
echo "  2. Start the relay:    sudo systemctl start decentchat-relay"
echo "  3. Check status:       sudo systemctl status decentchat-relay"
echo "  4. View logs:          journalctl -u decentchat-relay -f"
echo ""
echo "To get each raw Guardian ticket, check the logs after starting."
