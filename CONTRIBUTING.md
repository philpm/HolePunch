# Contributing

Please submit focused changes and reproducible issues. Start with the [documentation](docs/README.md), [architecture](docs/ARCHITECTURE.md), and [security policy](SECURITY.md). Reports should include the version, platform, deployment method, reproduction steps, and redacted logs.

## Language and localization

Use English for source comments, identifiers, documentation, commit/PR descriptions, CLI help, and diagnostics. English documentation is the source of truth. Non-English text belongs in locale catalogs or intentional Unicode test fixtures. Community members may ask questions in their preferred language; language fluency is not a prerequisite for contributing.

Every new web/GUI message must be localizable. Update English and Simplified Chinese catalogs together, preserve placeholders, and test both languages. Do not translate user-entered names, domains, API fields, commands, or raw logs. See the [localization guide](docs/I18N.md).

## Validation

```sh
python3 scripts/check-repository.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python3 scripts/sync-i18n.py --check
node --test tests/i18n.test.cjs
```

Data-plane or coordination changes also require `scripts/e2e-selfhost.py` and `scripts/e2e.py`, covering rate limits, revocation, reconnection, and recovery. Desktop changes require its separate crate checks and native build. Web changes require desktop/mobile, keyboard, theme, and locale checks, including dirty forms and open dialogs.

Database changes need migration tests and must preserve released data compatibility. Older binaries must reject newer schemas. State which platforms were not tested; an existing workflow does not prove CI passed.

Do not commit environment files, databases, generated TOML, keys, or unredacted logs. Keep generated evidence in ignored directories. Current guides describe current behavior; move obsolete material into Git history. Update links, Rust embedded paths, packaging, and localization assets when moving files.

Keep discussion respectful and focused on technical issues. Do not publish personal information or exploitable security details. Maintainers must enable private vulnerability reporting before opening the repository.

## Community and licensing

Follow the [code of conduct](CODE_OF_CONDUCT.md) and [support policy](SUPPORT.md). Submit focused feature branches to `main`; keep internal archive history outside public branches as described in [Maintaining](docs/MAINTAINING.md).

Unless explicitly stated otherwise, contributions intentionally submitted for inclusion are provided under Apache-2.0, as described in Section 5 of [LICENSE](LICENSE). Preserve third-party license and attribution notices. No additional contributor agreement is required by this repository.
