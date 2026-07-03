# Open Adapter Fixture Set

These fixtures demonstrate the public LDGR adapter workflow using only the
open manifest and lifecycle fields.

## Scenarios

- `valid-manifest/` contains a complete community adapter bundle with
  `adapter.toml`, a loop prompt, and templates referenced by the manifest.
- `malformed-manifest/` contains an intentionally broken `adapter.toml` for
  parser and discovery failure examples.
- `bundle-materialization/expected-files.txt` lists the files a minimal adapter
  installer should materialize under an adapter install root.
- `profile-discover/` models an adapter search root containing one valid
  manifest and one malformed manifest. Inspect it with `ldgr adapter list` and
  `ldgr adapter show community-sample` after setting `LDGR_ADAPTER_PATH` to the
  fixture's `adapters/` directory.
- `profile-apply/` contains a self-contained adapter bundle suitable for adapter
  root/introspection checks with `ldgr adapter list` and
  `ldgr adapter show community-sample`; command execution then uses the manifest
  namespace, for example `ldgr community-sample --help`, once the declared
  executable is installed.

The fixtures use only public manifest fields: adapter identity, profile file
paths, adapter-owned tools, command namespaces, target profiles, and probe
families. Core adapter workflows use `ldgr adapter install list`,
`ldgr adapter install <adapter>`, `ldgr adapter list`, `ldgr adapter show <slug>`,
and `ldgr <adapter-namespace> ...`; they do not use the removed core profile
discovery/application command surface.
