#!/bin/bash
# Deploy MClaw Dispatcher Service

set -e

DISPATCHER_DIR="/opt/works/personal/github/mclaw/deploy/dispatcher"
CONFIG_DIR="/etc/mclaw"
SERVICE_FILE="/etc/systemd/system/mclaw-dispatcher.service"

echo "🚀 Deploying MClaw Dispatcher Service..."

# Create config directory
echo "Creating config directory..."
mkdir -p "$CONFIG_DIR"

# Copy configuration files
echo "Installing configuration files..."
cp "$DISPATCHER_DIR/dispatcher.toml" "$CONFIG_DIR/"
cp "$DISPATCHER_DIR/machines.toml" "$CONFIG_DIR/"

# Install systemd service
echo "Installing systemd service..."
cp "$DISPATCHER_DIR/mclaw-dispatcher.service" "$SERVICE_FILE"

# Reload systemd
echo "Reloading systemd..."
systemctl daemon-reload

echo "✅ Dispatcher service installed!"
echo ""
echo "To start the service:"
echo "  systemctl start mclaw-dispatcher"
echo ""
echo "To enable on boot:"
echo "  systemctl enable mclaw-dispatcher"
echo ""
echo "To check status:"
echo "  systemctl status mclaw-dispatcher"
echo ""
echo "To view logs:"
echo "  journalctl -u mclaw-dispatcher -f"
