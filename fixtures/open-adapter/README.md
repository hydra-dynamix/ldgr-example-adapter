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
  manifest and one malformed manifest.
- `profile-apply/` contains a self-contained adapter bundle suitable for
  `ldgr profile apply community-sample` after the bundle is copied into a
  configured adapter root.

The fixtures use only public manifest fields: adapter identity, profile file
paths, adapter-owned tools, target profiles, and probe families.
