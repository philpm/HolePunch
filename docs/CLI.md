# CLI manual

The CLI in `apps/cli` shares `crates/client-core` and connects to Self-hosted or Cloud using the same command; only the `--server` URL changes. It is the client itself, so no desktop app is required. Every device hosting a tunneled service needs a continuously running client process.

## Install and connect

Extract the native package supplied by your administrator for the correct platform. Keep `bin/holepunch-client` and `bin/rathole` together and add that directory to PATH, or use an absolute executable path. End users do not need Rust, Docker, or handwritten TOML. Public production downloads are not yet available; use the platforms actually provided by your administrator.

Create a client in the console, then run:

```sh
holepunch-client connect --server https://panel.example.com
```

Enter the one-time enrollment code at the prompt. It expires after 10 minutes and can be redeemed only once. The long-lived device token is saved locally, not included in the command or a download URL. Scripts can supply the enrollment code through `--token-stdin`. The compatibility `--enroll-token` option is discouraged on shared terminals.

After binding, `holepunch-client connect` reuses saved credentials. Create, edit, enable, disable, and delete tunnels in the web console. Ctrl-C stops the foreground process while retaining the binding. CLI help, prompts, and diagnostics use English; GUI language selection does not change shell output.

## Commands

| Command | Behavior |
|---|---|
| `connect [--server URL]` | Redeem an enrollment code or run a bound device |
| `status` | Show process, binding, last synchronization, and tunnel status without tokens |
| `logs [--follow]` | Read data-plane logs; retained core logs are approximately 4 MiB |
| `doctor` | Check API, device authorization, control port, and local TCP services without changing configuration; UDP needs a real request |
| `disconnect` | Ask the current CLI to stop and wait for confirmation; retain binding |
| `logout` | Stop and remove local binding/service configuration; server-side revocation is a web action |
| `version` / `--version` | Show version |
| `service install/start/stop/uninstall` | Explicitly manage a macOS LaunchAgent or Linux user systemd service |

All commands support `--data-dir PATH`. The default is `~/.holepunch`, also configurable with `HP_CLIENT_DATA`. Unix directories use 0700 and credential files 0600. The CLI uses protected local files; the desktop system keychain is separate. Avoid shared or synchronized data directories. A process lock prevents duplicate clients in the same directory.

`HP_RATHOLE_BIN` overrides the sidecar location; otherwise the CLI reads `rathole` beside its executable. Legacy `HP_API_URL` / `HP_CLIENT_TOKEN` enrollment remains compatible but is not the default onboarding path.

## Background operation

Bind once, stop the foreground process with Ctrl-C, then run:

```sh
holepunch-client service install
holepunch-client service start
holepunch-client status
```

macOS uses a current-user LaunchAgent. Linux uses `systemctl --user`; an administrator must enable lingering for operation after logout. Windows currently uses foreground `connect`; a verified service installer is not provided. Installation records absolute paths for the executable, data directory, and sidecar. Reinstall the service after moving them.

Use `service stop` to stop the background task and `service uninstall` to remove autostart. `disconnect` stops the current process without uninstalling the service. To upgrade, stop the service, replace client and sidecar in the same directory, then restart. Unbinding and removing autostart are separate operations.

## Recovery and troubleshooting

- Permanent device revocation removes the local binding; enroll again through the web console. Cloud account freeze/unfreeze follows that service's policy.
- Temporary outages retry automatically. Forwarding pauses after 90 seconds without renewed configuration authorization; the server has a separate 15-second policy lease.
- Self-hosted optionally limits each tunnel without an account total. Cloud connections follow the service's user limits.
- If `doctor` reports an unreachable local service, confirm the application is listening on the configured address/port. If the API works but the control port does not, check proxies and firewalls; the client does not change system-wide networking.
- Successful commands exit 0; command, configuration, and network failures exit nonzero. Share only versions, redacted errors, and reproduction steps, never enrollment codes, `binding.json`, or `client.toml`.
