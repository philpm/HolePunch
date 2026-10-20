# Security and trust boundaries

## Identity and administration

Self-hosted initializes one administrator with email/password authentication. Sessions use HttpOnly, Secure, SameSite=Strict cookies; writes require CSRF validation. Password changes and session revocation are supported. Public registration, email recovery, MFA, and multi-tenancy are not part of this product.

Administrators can configure nodes and publish local services, including Caddy API, Ask URLs, and local websites. Share administrative access only with trusted operators. Devices redeem one-time enrollment codes for separate long-lived tokens; the server stores token digests. Changing an administrator password does not revoke devices.

`HP_ADMIN_TOKEN` is compatibility configuration, not the normal web login credential. `HP_LEGACY_ADMIN=1` explicitly enables Bearer compatibility on supported operations endpoints; it does not restore legacy resource writes.

## Network and data

- Use HTTPS for public panels. The server and Caddy Admin listen on loopback by default; do not expose ports 3000/2019 publicly.
- Clients reject public HTTP and token-bearing redirects. HTTP is allowed only for local loopback use.
- Client-to-edge transport uses Noise. Public TCP/UDP services still need application authentication; Noise does not replace it.
- Local websites proxy loopback HTTP; they do not execute PHP or manage application processes. Administrators must decide which services are safe to publish. Port conflict checks cannot identify every sensitive application.
- Unix runtime directories use 0700 and credential files 0600. The CLI uses protected files; desktop can use the system credential store. Manage Windows directory ACLs separately.
- Databases, generated TOML/Caddy configuration, backups, and test directories may contain service keys. Do not publish them. Back up Caddy certificates and environment configuration separately.
- Web responses set CSP, framing restrictions, nosniff, and no-referrer, and do not expose an open CORS API. Deployment access controls remain necessary.
- Locale bundles are reviewed, trusted application assets, never remote translation services. Templates retain existing escaping for user data. Do not insert unescaped user content into localized HTML.

## Operational limits

The client pauses forwarding if its configuration authorization has not refreshed for 90 seconds. Server gateways have an independent 15-second policy lease. Revocation and updates still require successful coordination. Server exit closes gateway connections; new connections recover after restart, but existing connections are not preserved.

Rotating the Ask key may briefly affect new certificate requests before Caddy synchronizes. Disabling a domain does not revoke existing certificates.

Audit storage retains at most 1000 events and returns the latest 100; it is neither tamper-proof nor a long-term archive. Client logs retain approximately 4 MiB and may contain service addresses. Host interface rates are not billable per-tunnel traffic accounting.

## Vulnerabilities and dependencies

Scan and review the current lockfiles before release; see [dependency review](docs/RELEASE.md#dependency-review). Historical scan results do not establish current safety.

Follow the private reporting process below.

## Supported versions and reporting

Security fixes currently target `main` and the latest development release. Older previews have no maintenance guarantee; there is no stable support window yet.

Use [GitHub private vulnerability reporting](https://github.com/philpm/HolePunch/security/advisories/new). Include the affected revision, impact, minimal reproduction, and sanitized diagnostics. Do not post exploit details, tokens, database dumps, or customer information in public issues.

If private reporting is unavailable, contact the owner using a private contact option on [their GitHub profile](https://github.com/philpm) to arrange a reporting channel. Do not disclose the vulnerability in a public support request. Maintainers must enable and verify private reporting before public launch. We coordinate remediation and disclosure with reporters; no response-time guarantee is currently offered.
