# Local state store

`neoget-engine::StateStore` provides a SQLite-backed foundation for normalized
source metadata, exact observed-version pins, and desired-state bundle plan
records. Callers choose the database path; the library does not infer a home
directory. Relative paths resolve against the process working directory. The
caller must create and secure the parent directory (including private-user
permissions) before opening the database; the store does not create directories
or resolve/reject symbolic links. SQLite is bundled and compiled with Rust 1.100.0.
Concurrent writers are serialized by SQLite with a five-second busy timeout.
Database failures return `STATE_STORE_ERROR` and retain the SQLite cause.
Contention/timeout means the operation did not report success; idempotent
source and pin operations can safely be retried.

`open` applies schema migrations. Source imports validate every entry before
starting a transaction; duplicate source keys update metadata idempotently.
Source removal and pin removal are repeat-safe. Source updates alter metadata
only, and unsupported drivers return a typed `Unsupported` error. Pins bind a
provider/name to an observed exact version; attempting to replace it with a
different version returns `PIN_CONFLICT`.

Bundle plans store the same versioned deterministic JSON encoding whose bytes
are hashed with SHA-256 and persisted. Digest reuse with different bytes is
rejected, and load verifies the digest before decoding. Saving a plan does not
resolve packages, call providers, or mutate
package-manager state. This storage layer is not yet wired to CLI commands;
those commands continue to report their specific unavailable capabilities.
