# Maintaining the public repository

[English](MAINTAINING.md) | [简体中文](zh-CN/MAINTAINING.md)

`main` is the public integration branch. Use focused feature branches and pull requests; require passing CI and review before merging. Keep releases tied to immutable tags and a clean, tested checkout. See [Contributing](../CONTRIBUTING.md), [Testing](TESTING.md), and [Release](RELEASE.md).

## Public and local material

Source, generic deployment tools, examples, documentation, and tests belong on `main`. Customer information, production endpoints, access credentials, private incident reports, raw test evidence, and internal acceptance diaries do not. Keep runtime data outside Git in protected, ignored directories. Ignore rules do not protect files already tracked by Git.

A local archive branch may retain historical planning and operator notes. Do not push archive branches, merge their history into `main`, or use `git push --all` / `git push --mirror`. A branch in a public GitHub repository is public even if its name contains “private”. Credentials belong in a secret store, never in any branch. Rotate any credential that was exposed; removing a file does not revoke it.

Before first publication, review all remote branches, tags, release assets, Actions artifacts, and reachable history. A clean local `main` does not remove material already stored on a remote. Publish only reviewed refs; if replacing existing remote history, coordinate the change and use an explicit ref with `--force-with-lease` only after approval. Preserve a protected local backup first.

## GitHub settings

Enable private vulnerability reporting, secret scanning/push protection, Dependabot alerts, and branch rules requiring CI and pull-request review where available. Use read-only workflow permissions by default. Do not expose deployment secrets to pull requests from forks. The repository's workflow files do not configure these account settings.

Verify the private reporting link in [Security](../SECURITY.md) before announcing the repository. Check clone/build/install instructions in a fresh checkout and confirm the license is detected as Apache-2.0.

## Dependency and release changes

Review lockfile updates and upstream licenses, run relevant tests, and record user-facing changes in [Changelog](../CHANGELOG.md). Commit lockfiles for reproducible application builds. Generated catalogs are committed so normal server builds need no frontend toolchain; regenerate them from the source JSON catalogs.

CI runs once per pull-request revision and on pushes to `main`; new revisions cancel superseded runs. Use `workflow_dispatch` to rerun a branch explicitly. Linux server jobs use Ubuntu 24.04 so runner-image rollouts do not silently change the build environment.

Dependabot groups compatible Cargo/npm updates and groups Actions updates, while keeping breaking Cargo/npm upgrades separate. The Rust compiler stays at the documented 1.91.1 baseline: update it deliberately after checking that the compiler is released and reviewing the minimum supported Rust version. `dtolnay/rust-toolchain` tags may exist before a compiler release, so automatic version-tag bumps are excluded.

Never label an Actions artifact as a published stable release. Validate the actual archive, checksum, version, platform, license inventory, and installation flow before uploading release assets.

References: [GitHub repository practices](https://docs.github.com/en/repositories/creating-and-managing-repositories/best-practices-for-repositories), [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0).
