#!/bin/bash
set -e

echo "=== DecentChat Room Host Setup ==="
echo ""

# Check if running as root for system-wide installation
if [ "$EUID" -ne 0 ]; then
    echo "Please run as root (sudo ./setup.sh)"
    exit 1
fi

# Get the actual user who invoked sudo
ACTUAL_USER=${SUDO_USER:-$USER}

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Install the latest verified release binary.
echo "Installing DecentChat release binary..."
DECENTCHAT_INSTALL_DIR=/usr/local/bin "$SCRIPT_DIR/../install.sh"

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
echo "To get each room ticket and join command, check the logs after starting."
