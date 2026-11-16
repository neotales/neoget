# Declarative package manifests

Neoget manifest v1 is a strict, side-effect-free package selection format. It
declares provider-qualified package identities, optional exact versions,
platform allow-lists (`linux`, `macos`, `windows`), dependency references, and
one or more bundle roots. Resolution returns a deterministic lock-shaped
selection; it does not install, download, execute, or mutate any native provider.
Unknown fields and unsupported schema versions are errors.

Resolution includes only the transitive dependency closure of declared roots.
An unsupported root is an error; a platform-inapplicable dependency branch is
omitted. Two selected declarations for the same provider/package identity must
carry identical optional versions: a missing version is not a wildcard and
conflicts with a specified version.

```rust
use neoget_config::{Format, HostPlatform, parse_manifest};

let manifest = parse_manifest(input, Format::Toml)?;
let selected = manifest.resolve(HostPlatform::Linux)?;
# Ok::<(), neoget_core::CoreError>(())
```

QuickJS remains a possible future constrained selector only. It is **not** a
manifest execution engine, and manifest resolution does not run scripts.
