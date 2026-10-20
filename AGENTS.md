# Repository conventions

- Treat this repository as the independent Apache-2.0 Self-hosted product and shared core. Cloud-specific application code belongs in the separate Apache-2.0 HolePunch-Cloud repository. Operational credentials and customer data belong outside Git in both repositories.
- Use English as the canonical language for documentation, identifiers, comments, CLI help, developer diagnostics, and examples. Maintain Simplified Chinese documentation in `README-zh.md` and `docs/zh-CN/`, keeping commands and identifiers unchanged. Follow `CONTRIBUTING.md` and `docs/I18N.md`.
- Localize all web and desktop UI text. English is the default; keep supported locale catalogs complete and preserve interpolation, accessibility labels, user data, and form drafts.
- Edit `web/locales/*.json`, then run `python3 scripts/sync-i18n.py`. Do not edit the generated bundle or desktop copies by hand.
- Check catalog parity and `node --test tests/i18n.test.cjs` for UI changes. Use the normal Rust and relevant integration checks in `docs/DEVELOPMENT.md`.
- Keep examples generic and never commit runtime credentials, generated configuration, databases, or unredacted test evidence.
- Keep `main` suitable for public distribution. Never push local archive branches or merge private history into `main`; follow `docs/MAINTAINING.md`.
- Run the repository hygiene check in addition to the relevant build, test, and localization checks.
