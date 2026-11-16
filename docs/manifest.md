# Declarative package manifests

Neoget manifest v1 is a strict, side-effect-free package selection format. It
declares provider-qualified package identities, optional exact versions,
platform allow-lists (`linux`, `macos`, `windows`), dependency references, and
one or more bundle roots. Resolution returns a deterministic lock-shaped
selection; it does not install, download, execute, or mutate any native provider.
Unknown fields and unsupported schema versions are errors.

QuickJS remains a possible future constrained selector only. It is **not** a
manifest execution engine, and manifest resolution does not run scripts.
