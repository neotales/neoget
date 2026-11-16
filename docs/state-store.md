# Local state store

`neoget-engine::StateStore` provides a SQLite-backed foundation for normalized
source metadata, exact observed-version pins, and desired-state bundle plan
records. Callers choose the database path; the library does not infer a home
directory. SQLite is bundled and compiled with Rust 1.100.0.

`open` applies schema migrations. Source imports validate every entry before
starting a transaction; duplicate source keys update metadata idempotently.
Source removal and pin removal are repeat-safe. Source updates alter metadata
only, and unsupported drivers return a typed `Unsupported` error. Pins bind a
provider/name to an observed exact version; attempting to replace it with a
different version returns `PIN_CONFLICT`.

Bundle plans store a SHA-256 digest of deterministically sorted normalized
intents. Saving a plan does not resolve packages, call providers, or mutate
package-manager state. This storage layer is not yet wired to CLI commands;
those commands continue to report their specific unavailable capabilities.
