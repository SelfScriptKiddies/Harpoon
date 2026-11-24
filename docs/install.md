# Installation with systemd and nginx

This guide installs Harpoon on Debian or Ubuntu as a systemd service and publishes its Web UI through nginx over HTTPS.

systemd starts Harpoon at boot, restarts it after a crash and runs it under a dedicated system user. nginx terminates TLS and forwards requests to the Web UI, which stays bound to loopback. The proxy rules are plain TCP and UDP listeners and do not pass through nginx.

Written and tested on Debian 13 with systemd 257, nginx 1.26 and the `harpoon-full-static` binary of release v1.0.1.

## Before you start

- A domain name that resolves to the server. The dashboard loads its scripts and API from the root of the host, so it needs a domain or subdomain of its own.
- TCP ports 80 and 443 open to the server, plus the ports your proxy rules listen on. The Web UI port stays closed, nginx reaches it over loopback.

## Step 1: Install the binary

The releases on GitHub ship `harpoon-full-static`, a statically linked x86-64 binary with the Web UI, TLS MITM, regex filters and transparent UDP compiled in. Nothing has to be built on the server. Download the latest release, check it against the digest that GitHub publishes for it and install it:

```bash
sudo apt-get install -y curl python3
curl -fsSL -o /tmp/harpoon \
    https://github.com/SelfScriptKiddies/Harpoon/releases/latest/download/harpoon-full-static
curl -fsSL https://api.github.com/repos/SelfScriptKiddies/Harpoon/releases/latest \
    | python3 -c 'import json,sys; print([a for a in json.load(sys.stdin)["assets"] if a["name"] == "harpoon-full-static"][0]["digest"].split(":")[1], "/tmp/harpoon")' \
    | sha256sum -c
sudo install -m 755 /tmp/harpoon /usr/local/bin/harpoon
harpoon --help
```

Install the file only when `sha256sum` answers `/tmp/harpoon: OK`.

- Do not take `harpoon-minimal` from the same release. It has no Web UI, and the nginx site in Step 6 would have nothing to forward to.
- On an `arm64` host, take `harpoon-full` out of `harpoon-linux-arm64.tar.gz` of the same release. That binary is static too, but without transparent UDP.
- To pin a version, replace `latest/download` in the URL with `download/v1.0.1`.
- To build the binary yourself, see [Building from source](#building-from-source).

## Step 2: Create the service user

Harpoon runs as the system user `harpoon`. Its configuration lives in `/etc/harpoon`.

```bash
sudo useradd --system --user-group --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin harpoon
sudo mkdir /etc/harpoon
sudo chown root:harpoon /etc/harpoon
sudo chmod 750 /etc/harpoon
```

## Step 3: Write the configuration

Create `/etc/harpoon/config.toml` with the Web UI settings and a first rule. The password is generated as you write the file:

```bash
HARPOON_WEB_PASSWORD=$(openssl rand -hex 16)
sudo tee /etc/harpoon/config.toml >/dev/null <<EOF
[global]
web_bind = "127.0.0.1:8888"
web_password = "$HARPOON_WEB_PASSWORD"

[[rules]]
name = "http"
protocol = "tcp"
listen = "0.0.0.0:8080"
target = "10.0.0.1:80"
EOF
sudo chown root:harpoon /etc/harpoon/config.toml
sudo chmod 660 /etc/harpoon/config.toml
echo "$HARPOON_WEB_PASSWORD"
```

- Keep `web_bind` on `127.0.0.1`. nginx is the only client that should reach the Web UI.
- The login is `admin` with the password printed by the last command.
- Replace the `http` rule with your own rules. The format is described in [config.md](config.md).
- The file is group-writable because the dashboard saves rule changes back into it, and drops comments when it does. If you edit the file only by hand, `sudo chmod 640` is enough.

## Step 4: Create the systemd unit

```bash
sudo tee /etc/systemd/system/harpoon.service >/dev/null <<'EOF'
[Unit]
Description=Harpoon traffic-analysis proxy
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=harpoon
Group=harpoon
ExecStart=/usr/local/bin/harpoon run -c /etc/harpoon/config.toml --socket /run/harpoon/harpoon.sock --pid-file /run/harpoon/harpoon.pid
RuntimeDirectory=harpoon
# harpoon shuts down cleanly only on SIGINT; SIGTERM kills it and leaves nftables rules behind
KillSignal=SIGINT
Restart=on-failure
RestartSec=3
NoNewPrivileges=yes
ProtectHome=yes

[Install]
WantedBy=multi-user.target
EOF
```

- The service runs in the foreground. Do not add `-d`, systemd has to stay the parent of the process.
- The control socket and the PID file live in `/run/harpoon`, which systemd creates for the service user at start and removes at stop. Only `root` can connect to the socket.
- `KillSignal=SIGINT` is required. Harpoon handles only SIGINT. With the default SIGTERM it is killed without cleanup and leaves its `harpoon` nftables table in the kernel.
- `ProtectHome=yes` hides `/home`, `/root` and `/run/user` from the service. Keep the files it reads under `/etc/harpoon`.

### Ports below 1024 and nftables

The service user has no special rights. Grant what your configuration needs by adding one line to `[Service]`, then run `sudo systemctl daemon-reload` and `sudo systemctl restart harpoon`:

```ini
AmbientCapabilities=CAP_NET_BIND_SERVICE
```

- Add `CAP_NET_BIND_SERVICE` when a rule listens on a port below 1024. Without it that rule is skipped.
- Add `CAP_NET_ADMIN` when `[global.nft]` is enabled, as described in [nftables.md](nftables.md). Without it Harpoon does not start. The `nftables` package has to be installed as well.
- When you need both, write `AmbientCapabilities=CAP_NET_BIND_SERVICE CAP_NET_ADMIN`.

## Step 5: Start the service

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now harpoon
systemctl status harpoon --no-pager
sudo journalctl -u harpoon --no-pager -n 30
```

`systemctl status` must report `active (running)`. In the log every rule has a `tcp pipeline started` or `udp pipeline started` line with its listen address, and the Web UI reports `web UI listening`. A rule whose port cannot be bound is skipped without a log line, so compare the `pipeline started` lines with your configuration.

The command-line client talks to the running service over its control socket:

```bash
sudo harpoon --socket /run/harpoon/harpoon.sock status
```

```
Running:     true
Uptime:      5s
Rules:       1
Config:      /etc/harpoon/config.toml
```

## Step 6: Put nginx in front of the Web UI

```bash
sudo apt-get install -y nginx
sudo tee /etc/nginx/sites-available/harpoon >/dev/null <<'EOF'
server {
    listen 80;
    listen [::]:80;
    server_name harpoon.example.com;

    location / {
        proxy_pass http://127.0.0.1:8888;
        proxy_set_header Host $host;
    }

    location = /api/capture/ws {
        proxy_pass http://127.0.0.1:8888;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_read_timeout 3600s;
    }
}
EOF
sudo ln -s /etc/nginx/sites-available/harpoon /etc/nginx/sites-enabled/harpoon
sudo nginx -t
sudo systemctl reload nginx
```

Replace `harpoon.example.com` with your domain. Debian's `default` site keeps answering for other host names; remove `/etc/nginx/sites-enabled/default` if this server has no other use for port 80.

The second `location` serves the WebSocket endpoint `/api/capture/ws`. The upgrade only passes with the `Upgrade` and `Connection` headers, and `proxy_read_timeout` stops nginx from closing an idle stream after its default of 60 seconds.

Check the site from the server:

```bash
curl -s -o /dev/null -w '%{http_code}\n' -H 'Host: harpoon.example.com' http://127.0.0.1/
```

The answer is `200`.

## Step 7: Enable HTTPS

The login sends the password and the session token with every request, so the Web UI should not be reachable over plain HTTP. With Let's Encrypt:

```bash
sudo apt-get install -y certbot python3-certbot-nginx
sudo certbot --nginx -d harpoon.example.com --redirect
```

certbot finds the server block by its name, adds the TLS listener and the certificate paths to it and reloads nginx. The `certbot` package brings a systemd timer that renews the certificate, and `--redirect` makes port 80 forward to HTTPS.

With a certificate from another CA, make the same change by hand. Add this block above the existing one, so that port 80 forwards to HTTPS:

```nginx
server {
    listen 80;
    listen [::]:80;
    server_name harpoon.example.com;
    return 301 https://$host$request_uri;
}
```

In the existing block replace the two `listen 80` lines and add the certificate below `server_name`:

```nginx
    listen 443 ssl;
    listen [::]:443 ssl;
```

```nginx
    ssl_certificate     /etc/nginx/ssl/harpoon.crt;
    ssl_certificate_key /etc/nginx/ssl/harpoon.key;
```

Then run `sudo nginx -t` and `sudo systemctl reload nginx`.

## Step 8: Log in

Open `https://harpoon.example.com` and log in as `admin` with the password from Step 3.

## Running the service

| Command | Purpose |
|---|---|
| `sudo journalctl -u harpoon -f` | Follow the log |
| `sudo systemctl restart harpoon` | Apply configuration changes |
| `sudo harpoon --socket /run/harpoon/harpoon.sock status` | Ask the service for its state |
| `sudo harpoon --socket /run/harpoon/harpoon.sock stats` | Per-rule traffic counters |
| `sudo harpoon --socket /run/harpoon/harpoon.sock reload` | Replace rules without a restart |

- `restart` applies everything: rules, Web UI settings and nftables rules.
- `reload` replaces rules and pipelines only. Changes to `web_bind`, `web_password` or `[global.nft]` need a `restart`. The command returns before the result is known, so look for `config reloaded successfully` or `config reload failed` in the log.
- To upgrade, download the new release and install it as in Step 1, then run `sudo systemctl restart harpoon`.
- The Shutdown button in the dashboard and `harpoon stop` end the service with a clean exit, so systemd does not start it again. Bring it back with `sudo systemctl start harpoon`.

## Building from source

Only needed when the releases do not provide the binary you want. The build machine needs `rustup`, `make`, Node.js 22 and `musl-tools`. In a checkout of this repository:

```bash
rustup target add x86_64-unknown-linux-musl
sudo apt-get install -y musl-tools
make static-full
sudo install -m 755 dist/harpoon-full-static /usr/local/bin/harpoon
```

`make static-full` builds the dashboard, embeds it in the binary and writes `dist/harpoon-full-static`, statically linked with the same features as the release binary. You can build it on a workstation and copy it to the server. `make static-standard` builds the same binary without TLS MITM, regex filters and transparent UDP. The Web UI has to be compiled in with the `web` feature: a plain `cargo build --release` leaves it out, and nothing is left for nginx to forward to.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| The service keeps restarting or ends up `failed`, and the log shows `nft failed` with `Operation not permitted` | `[global.nft]` is enabled but the service lacks the right | Add `CAP_NET_ADMIN` to `AmbientCapabilities` |
| The log repeats `nftables enabled in config but 'nft' command not found` | The `nft` binary is missing | `sudo apt-get install nftables` |
| A rule accepts no connections and has no `pipeline started` line | Its port is taken, or below 1024 without the capability | Free the port, or add `CAP_NET_BIND_SERVICE` |
| nginx answers `502 Bad Gateway` | Nothing listens where `proxy_pass` points | Check that the installed binary is `harpoon-full-static`, or another build with the `web` feature, and that `web_bind` matches the port in `proxy_pass` |
| The rule editor answers `failed to save config` | `config.toml` is not writable by the service | `sudo chown root:harpoon /etc/harpoon/config.toml` and `sudo chmod 660 /etc/harpoon/config.toml` |
| The login page answers `Invalid credentials` for the right password | 10 failed logins within 5 minutes lock the one address Harpoon sees, for everyone | Wait 5 minutes |
| The log shows `Permission denied` or `No such file or directory` for a file that exists under `/home`, `/root` or `/run/user` | `ProtectHome=yes` hides those directories from the service | Move the file under `/etc/harpoon` |
| The service is `inactive (dead)` after a shutdown from the dashboard | A shutdown is a clean exit, systemd does not restart the service | `sudo systemctl start harpoon` |
