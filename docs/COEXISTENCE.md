# Sharing a VPS with other websites

[English](COEXISTENCE.md) | [简体中文](zh-CN/COEXISTENCE.md)

Use **Local websites** in the console to publish applications on the same server. Caddy manages HTTPS for the panel, web tunnels, and local websites. Local websites bypass the client and rathole, so they continue working after client revocation.

## Recommended layout for a new server

| Public endpoint | Backend |
|---|---|
| panel.example.com | Center at 127.0.0.1:3000 |
| blog.example.com | Local blog at 127.0.0.1:8080 |
| app.example.com | Local application at 127.0.0.1:8081 |
| nas.example.com | Web tunnel → rathole → private NAS |

Caddy owns public ports 80/443. Other web servers listen on loopback ports, which need no public firewall rule. Websites may share an application port if the backend routes by Host; domains must remain unique.

## Add a website

1. Start a local HTTP application, for example at `127.0.0.1:8080`, and check it on the VPS with `curl -I http://127.0.0.1:8080`.
2. Point `blog.example.com` A/AAAA records at the VPS. Do not add AAAA without working IPv6 connectivity.
3. Open **Local websites → Add website** and enter a name, domain, loopback address, and HTTP port.
4. Save, wait for **Configuration applied**, and open `https://blog.example.com`. Initial certificate issuance requires correct DNS, public ports 80/443, and an available CA.
5. Edit, disable, re-enable, or delete as needed. Disabling/deleting removes the route and denies new certificate authorization; existing certificates are not revoked. Application processes and files are unchanged.

Website records participate in database revisions, audit events, online backups, and restore. Caddy restarts, tunnel edits, and client revocation do not erase them.

## Container applications

Publish the container's HTTP port on host loopback and enter that host port in the console:

```yaml
services:
  app:
    image: your-verified-app:fixed-version
    ports:
      - "127.0.0.1:8080:3000"
```

Replace the image and internal port with real application values. The default Linux Compose stack uses host networking, allowing Caddy to reach this address. Do not enter a container IP or Docker service name in the console.

## PHP, Nginx, and existing control panels

HolePunch proxies HTTP; it does not execute PHP or speak FastCGI. Do not enter PHP-FPM port 9000. Serve the full PHP site through Nginx/Apache on a loopback HTTP port, then add that port in **Local websites**.

For example, change the site's Nginx listener to `listen 127.0.0.1:8080;` while preserving its `server_name`, document root, and FastCGI settings. Caddy owns certificates. Configure the application's public HTTPS URL and trusted proxy settings to avoid cookie errors, incorrect absolute links, and redirect loops. Trust forwarded headers only from the local entry point.

For an existing BaoTa/BT Panel server, back up configuration, verify each backend over HTTP, then schedule the public-entry migration. The panel may regenerate Nginx configuration. HolePunch neither modifies the panel nor imports its websites automatically. Default Caddy cannot bind public 80/443 while the existing Nginx owns those listeners.

## Limits and troubleshooting

- Exact domains map to loopback HTTP backends only: `127.0.0.1` / `::1`; API `localhost` normalizes to `127.0.0.1`. Applications retain responsibility for authentication, files, databases, and process operation.
- HTTPS and WebSockets use Caddy's standard reverse proxy. Coexistence integration evidence covers HTTP over TLS; separate WebSocket load testing is not established by that check.
- This UI does not expose wildcard routes, path routing, remote/HTTPS backends, or arbitrary Caddy JSON.
- Website, tunnel, and panel domains are mutually exclusive. Disabled records still reserve domains. Edit or delete the previous record before moving a domain.
- Backend ports must avoid Center, Caddy Admin, HTTP/HTTPS entry points, rathole control, and tunnel public listeners. Tunnel allocation also avoids registered website ports. Check unregistered external application listeners yourself.
- **Enabled** is configuration state; **Configuration applied** means the edge accepted it, not that the application is healthy. For HTTPS 502, inspect the application process, listener, and logs.
- Caddy is a dedicated managed instance. Manual changes are overwritten by reconciliation; register websites through the console.
- All applications share CPU, memory, disk, and bandwidth. Size the server for real combined load.

Current storage is schema 4. Back up before upgrading. Rollback requires a matching-version backup; older binaries must not open a newer database. See [deployment](DEPLOYMENT.md).

## Installation tooling

The [native installer](INSTALLATION.md) probes the selected service and tunnel ports and refuses conflicts without stopping existing applications. Both native setup and Compose setup accept custom panel, Caddy Admin, control, HTTP/HTTPS and tunnel-range ports. They generate matching internal URLs and bootstrap settings. Compose setup validates configuration but cannot inspect a remote Docker host's listeners. Neither path edits existing web servers or firewall rules.

Choosing alternate HTTP/HTTPS ports does not automatically make an existing edge proxy support HolePunch domain routing or certificate challenges. Complete the public edge topology described above before expecting standard-port public URLs.
