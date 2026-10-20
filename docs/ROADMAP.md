# Self-hosted roadmap

See [EDITIONS](EDITIONS.md) for product boundaries. Cloud application plans belong in its own repository. Production operations and customer data stay outside Git.

## Implemented locally

Independent public core and Self-hosted builds; one-time CLI enrollment; sidebar console; custom and random domains; unlimited/optional tunnel rate limits; local websites; backup/restore; edition-specific data protection. Cloud business handlers have moved out of this repository.

English is now the canonical development and documentation language. The web console, compatibility page, and desktop GUI share English/Simplified Chinese localization, including persistent selection and desktop tray labels. Locale catalog validation is part of CI.

The three server deployment paths are implemented: interactive native install from a verified archive, clone/build/run, and Linux Docker Compose. Native setup generates private configuration and checks listener conflicts; Linux systemd can supervise the whole stack. The portable runner serves macOS/Windows foreground operation. Both Compose variants support custom ports and first-administrator bootstrap. See [installation](INSTALLATION.md) for the supported scope and release-availability requirements.

The console now has visible language controls and an authenticated four-step first-run guide. Completion is saved on the server; deployment commands report browser access and initial credentials with a redaction option for automation.

## P0: community release acceptance

- Verify independent clones, pinned dependencies, install/upgrade/rollback, and both repositories' CI.
- Exercise native Linux systemd, Linux Docker, Windows, and Intel/ARM Macs on real target systems.
- Validate public DNS/CA, outage recovery, 24-hour mixed load, and resource ceilings.
- Review source and Git history for operational information, establish private vulnerability reporting, then decide when to publish.
- Review English terminology and both locales with community contributors; keep locale parity checks mandatory.

## P1: usability

Add desktop one-time enrollment, diagnostic log export, automated native upgrades, and optional macOS/Windows background-service integration. Keep sharing the client core; desktop is maintained in this repository. Additional UI locales are welcome through the documented catalog workflow.

## Later

Define a remote node protocol and node health checks. Global route selection requires real link measurements and is not a current feature promise. Do not add cumulative traffic accounting or traffic caps.
