# DecentChat Guardian super-peer deployment

Deploy one always-on, multi-room DecentChat host on a Linux VPS with systemd.

## Setup

```bash
git clone https://github.com/maslowalex/decentchat.git
cd decentchat
sudo ./deploy/setup.sh
sudo systemctl start decentchat-relay
journalctl -u decentchat-relay -f
```

The setup downloads the latest checksum-verified DecentChat release, creates `/var/lib/decentchat`, and installs the service. Guardian stores its node secret, blobs, iroh-docs state, and room stores beneath `/var/lib/decentchat/guardian/`.

## Configuration

Edit `/etc/decentchat/relay.env`:

| Variable | Default | Description |
|---|---|---|
| `RELAY_GROUPS` | `lobby` | Comma-separated rooms kept online by this process |
| `RELAY_PORT` | `4001` | Guardian/Iroh UDP endpoint port |

Then restart:

```bash
sudo systemctl restart decentchat-relay
```

The journal prints a raw Guardian ticket for each room. Share the whole string unchanged:

```bash
decentchat join '<guardian-doc-ticket>'
```

The first join prompts for a display name. For unattended use, pass `--name YourName`.

There is no external-IP or manual-peer setting. Guardian/Iroh performs discovery and relay selection; use the application's `--local` flag only for an mDNS-only LAN deployment.

Open the configured UDP port, for example:

```bash
sudo ufw allow 4001/udp
```

Useful commands:

```bash
sudo systemctl status decentchat-relay
sudo systemctl restart decentchat-relay
journalctl -u decentchat-relay -f
```

Persistent files:

| Path | Description |
|---|---|
| `/usr/local/bin/decentchat` | Binary |
| `/etc/decentchat/relay.env` | Service configuration |
| `/var/lib/decentchat/guardian/` | Guardian identity and replicated room data |
| `/etc/systemd/system/decentchat-relay.service` | systemd unit |
