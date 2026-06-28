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
ldgr-example-adapter adapter install      # writes .ldgr/.example/
ldgr-example-adapter profile discover
ldgr-example-adapter profile apply
```

`profile apply` initializes `.ldgr/ldgr.db` if needed, installs/updates the `example-loop` prompt, and marks it active.

## Commands

```sh
ldgr-example-adapter manifest-summary [--json]
ldgr-example-adapter adapter install [--adapter-root DIR | --install-root DIR] [--print-path]
ldgr-example-adapter profile discover
ldgr-example-adapter profile apply [--install-root DIR] [--ldgr-db PATH] [--ldgr-artifact-root DIR]
```

The adapter-owned command surface is intentionally separate from core `ldgr` commands.

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
