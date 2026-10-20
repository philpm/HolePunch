# Repository and product boundaries

HolePunch targets self-hosting. Cloud is a separate open-source product with independent commits, tests, and releases. Both projects are licensed under Apache-2.0.

| Repository | Contents |
|---|---|
| HolePunch | Public server/client cores, Self-hosted, CLI, optional desktop, shared web assets and protocol |
| HolePunch-Cloud | SaaS assembly, registration/email/invitations, users and quotas, generic operator documentation and tenant tests |

The public repository builds independently. Cloud pins a full public commit through a Git submodule; upgrading that dependency is an explicit, tested Cloud change, not automatic branch tracking. Deployment credentials, CI secrets, and user data stay outside Git in both repositories.

## Public interfaces

The core provides identity, resource ownership, optional rate limits, storage, domains, and forwarding. Cloud assembles its business handlers through extension routes. SMTP transport and SaaS handlers are absent from the public product.

Shared web assets use `/api/v1/capabilities` to display menus. The server checks permissions independently. Cloud capability names in public code are shared interface contracts, not Cloud implementation dependencies. See the [Self-hosted API](API.md).

## Data and behavior

Self-hosted has one administrator, no public registration or product count quotas, unlimited bandwidth by default, and full custom domains. Cloud supports multiple users and per-tunnel allocations within user limits. Neither edition performs cumulative traffic accounting or monthly traffic capping.

Schema 4 records the owning edition. Each server refuses the other edition's data directory; automatic conversion is not implemented. See [deployment](DEPLOYMENT.md) for upgrade and restore behavior.

## Desktop

Desktop remains an optional public app sharing the client core; it is maintained in this repository. Server and CLI builds do not require GUI dependencies. The current desktop binding form uses a long-lived device token; use the CLI for one-time enrollment. See [desktop development](../apps/desktop/README.md).

English-first conventions and localization apply to public code and shared UI. Cloud adopts shared changes when it upgrades its pinned dependency.

Public source publication and stable releases still require the [release checks](RELEASE.md). Cleaning a working tree does not erase Git history.
