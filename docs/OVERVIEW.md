# HolePunch Self-hosted requirements

[English](OVERVIEW.md) | [简体中文](zh-CN/OVERVIEW.md)

HolePunch serves individuals and small teams who want to run tunnels on their own VPS and domains. Cloud is a separate open-source product; see [repository boundaries](EDITIONS.md).

## Core experience

1. Deploy the server natively or with Docker, initialize one local administrator, and configure domains and DNS.
2. Create a device in **Clients**, then run the shared CLI and enter its one-time enrollment code.
3. In **Tunnels**, choose the local address, port, protocol, and public domain. The client synchronizes settings automatically; users do not edit tunnel configuration files.
4. Manage TCP, UDP, and HTTPS/WebSocket tunnels, inspect client status and configuration results, and publish other applications through **Local websites**.

## Product boundaries

- One administrator workspace; no public registration, email verification, invitations, user impersonation, or commercial plans.
- No product limits on client or tunnel counts and no cumulative traffic accounting. Port pools, hardware, and network capacity remain finite.
- Unlimited tunnel bandwidth by default. Optional per-tunnel limits aggregate both directions and all connections. Blank means unlimited; 0 disables the tunnel.
- Support full custom domains and optional random subdomains under a configured suffix. The shared core handles TLS authorization and conflicts.
- Password initialization, sessions, CSRF, token revocation, offline authorization leases, and backups are required capabilities.
- Keep sidebar navigation on desktop and a navigation drawer on mobile.
- The CLI is the primary client. The optional desktop application remains in this repository for now.
- English is the canonical language for development, documentation, comments, and diagnostics. Every maintained GUI supports English by default and a persistent language selector, initially English and Simplified Chinese. See [localization](I18N.md).

## Acceptance

A standalone clone builds and tests without Cloud source. Verify unlimited/limited transitions, custom-domain HTTPS, TCP/UDP, hot reload, revocation, recovery, and backups. Check both UI locales, interpolation, accessible labels, and preservation of user data and form drafts. Target-platform deployment and sustained-load acceptance remain release requirements.
