# Local state store

`neoget-engine::StateStore` provides a SQLite-backed foundation for normalized
source metadata, exact observed-version pins, and desired-state bundle plan
records. Callers choose the database path; the library does not infer a home
directory. Relative paths resolve against the process working directory. The
caller must create and secure the parent directory (including private-user
permissions) before opening the database; the store does not create directories
or resolve/reject symbolic links. SQLite is bundled and compiled with Rust 1.100.0.
Concurrent writers are serialized by SQLite with a five-second busy timeout.
Use a separate handle for each overlapping operation: a handle is not `Sync`.
Ordering between connections is unspecified, and multiple calls do not form a
single transaction. The timeout is SQLite's lock-wait policy, not a whole-call
deadline. Dropping a handle closes its connection.
Database failures return `STATE_STORE_ERROR` and retain the SQLite cause.
Contention/timeout means the operation did not report success; idempotent
source and pin operations can safely be retried.

`open` atomically applies schema migrations to an empty database. Existing
versioned stores must have the exact ordered columns, declared types, nullability,
and primary-key/unique-index semantics required by this schema, including
migration history. Incompatible or preexisting unversioned schemas return
`STATE_SCHEMA_INVALID`; newer schemas return `STATE_SCHEMA_FUTURE`. Rejection
does not rewrite existing schema/data.

Schema v1 deliberately accepts only the exact SQLite-stored `CREATE TABLE` text
emitted by its migration, shared internally between creation and validation.
Equivalent hand-authored rewrites (including whitespace/casing/quoting changes)
are not supported. The only CHECK is `schema_migrations.version=1`; no other
CHECK, foreign-key, default, explicit collation, generated-column, custom conflict,
STRICT, or WITHOUT ROWID clauses are supported. Store tables may have only their
migration-created primary-key indexes: externally created indexes (even
non-unique ones) and triggers are rejected too. These rules prevent constraints
that are invisible to column/key PRAGMAs from changing valid write behavior.
Do not alter schema externally while a handle is open.

Opening does not attest to all row contents:
source/pin reads validate every reconstructed record with the same source-driver
rules and public identity constructors used for writes. Invalid fields, invalid
text, or wrong SQLite storage types return `STATE_RECORD_CORRUPT` with a cause;
list reads never return partial results. Pins follow `ProviderId` rather than
the source-driver allowlist, and preserve provider-native names/versions.

Source imports validate every entry before
starting a transaction; duplicate source keys update metadata idempotently.
Source removal and pin removal are repeat-safe. Source updates alter metadata
only, and unsupported drivers return a typed `Unsupported` error. Pins bind a
provider/name to an observed exact version; attempting to replace it with a
different version returns `PIN_CONFLICT`.
The `StateStore` rustdoc includes a compiled open/pin/conflict/cleanup example.

Bundle plans store the same versioned deterministic JSON encoding whose bytes
are hashed with SHA-256 and persisted. Digest reuse with different bytes is
rejected, and load verifies the digest before decoding. Saving a plan does not
resolve packages, call providers, or mutate
package-manager state. This storage layer is not yet wired to CLI commands;
those commands continue to report their specific unavailable capabilities.
