# DecentChat Relay Deployment

Deploy a DecentChat relay node on any Linux VPS with systemd.

## Quick Start

1. Clone the repository to your VPS:
   ```bash
   git clone https://github.com/user/decentchat.git
   cd decentchat
   ```

2. Run the setup script:
   ```bash
   sudo ./deploy/setup.sh
   ```

3. Edit configuration (optional):
   ```bash
   sudo nano /etc/decentchat/relay.env
   ```

4. Start the relay:
   ```bash
   sudo systemctl start decentchat-relay
   ```

5. Get the connection ticket from logs:
   ```bash
   journalctl -u decentchat-relay | grep "dchat:"
   ```

## Configuration

Edit `/etc/decentchat/relay.env`:

| Variable | Default | Description |
|----------|---------|-------------|
| `RELAY_GROUPS` | `lobby` | Comma-separated list of groups to host |
| `RELAY_PORT` | `4433` | UDP port for QUIC connections |

After changing configuration, restart the service:
```bash
sudo systemctl restart decentchat-relay
```

## Commands

| Command | Description |
|---------|-------------|
| `sudo systemctl start decentchat-relay` | Start the relay |
| `sudo systemctl stop decentchat-relay` | Stop the relay |
| `sudo systemctl restart decentchat-relay` | Restart the relay |
| `sudo systemctl status decentchat-relay` | Check status |
| `journalctl -u decentchat-relay -f` | Follow logs |

## Connecting Clients

Share the connection ticket with users:

```bash
decentchat join --ticket "dchat:..." --name "YourName"
```

## Firewall

Ensure the relay port is open:

```bash
# UFW
sudo ufw allow 4433/udp

# firewalld
sudo firewall-cmd --add-port=4433/udp --permanent
sudo firewall-cmd --reload

# iptables
sudo iptables -A INPUT -p udp --dport 4433 -j ACCEPT
```

## Files

| Path | Description |
|------|-------------|
| `/usr/local/bin/decentchat` | Binary |
| `/etc/decentchat/relay.env` | Configuration |
| `/var/lib/decentchat/` | Identity and state |
| `/etc/systemd/system/decentchat-relay.service` | Systemd unit |

## Uninstall

```bash
sudo systemctl stop decentchat-relay
sudo systemctl disable decentchat-relay
sudo rm /etc/systemd/system/decentchat-relay.service
sudo rm /usr/local/bin/decentchat
sudo rm -rf /etc/decentchat
sudo rm -rf /var/lib/decentchat
sudo systemctl daemon-reload
```
