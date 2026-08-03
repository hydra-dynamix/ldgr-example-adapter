# ldgr-example-adapter

Reference LDGR adapter used to exercise open adapter lifecycle extension points.

This adapter is intentionally unrestricted and does not contain license-gating or entitlement logic. It is the public reference for adapter-owned install, discovery, profile application, manifest integrity, prompt activation, templates, target profiles, and command-extension behavior.

## Install from GitHub

```sh
cargo install --git https://github.com/hydra-dynamix/ldgr-example-adapter ldgr-example-adapter
```

From a source checkout:

```sh
git clone https://github.com/hydra-dynamix/ldgr-example-adapter
cd ldgr-example-adapter
cargo install --path .
```

## Quick start

Install the bundled adapter manifest and files, then apply the profile to a project ledger:

```sh
ldgr-example-adapter adapter install      # writes ~/.ldgr/adapters/example/
ldgr-example-adapter profile discover
ldgr-example-adapter profile apply
```

`adapter install` also copies bundled prompts into configured harness prompt paths from `~/.ldgr/config.json`. Without config, prompts default to `~/.ldgr/prompts`.

`profile apply` initializes `.ldgr/ldgr.db` if needed, installs/updates the `example-loop` prompt from the adapter bundle, and marks it active.

## Commands

```sh
ldgr-example-adapter manifest-summary [--json]
ldgr-example-adapter adapter install [--adapter-root DIR | --install-root DIR] [--print-path]
ldgr-example-adapter profile discover
ldgr-example-adapter profile apply [--install-root DIR] [--ldgr-db PATH] [--ldgr-artifact-root DIR]
```

The adapter-owned command surface is intentionally separate from core `ldgr` commands.

## Numerical sequence protocol

The reference adapter demonstrates opt-in numerical sequence collection through LDGR Core only. It declares `/sequences/example-adapter-lifecycle/v1` with command states `8` manifest-summary, `9` adapter-install, `10` profile-discover, and `11` profile-apply. Normalized terminal codes keep the Core meanings: `3` completed-positive, `4` completed-negative, `5` completed-inconclusive, `6` operational-failure, and `7` cancelled.

The adapter uses `ldgr::telemetry::buffer::LocalSequenceBuffer`; it never reads consent, opens a telemetry connection, serializes an upload, or adds labels. Core persists only a bare integer array such as `[0,1,8,3]`; no adapter names discovered, install roots, prompt paths, database paths, manifest paths, arguments, errors, or user content are encoded.

## Repository layout

| Path | Purpose |
| --- | --- |
| `adapter.toml` | Bundled reference adapter manifest. |
| `prompts/ldgr-loop-next-work.md` | Loop prompt installed as `example-loop`. |
| `templates/` | Example artifact/readiness templates. |
| `src/main.rs` | Installer, discovery, apply, and manifest-summary CLI. |
| `tests/cli.rs` | End-to-end CLI smoke tests. |

## Development

```sh
cargo fmt --all -- --check
cargo clippy --all-targets
cargo test
```

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
