# Config/grammar review handoff

Diagnosis: 6bab5d8 guessed that `-D` selected a directory and guessed a Neoenv
tool map. Both assumptions were wrong. Regression tests reproduced `-D apt`
bypassing a malformed environment-root config and the native Neoenv tool-list
fixtures failing. CLI `.to_string()` also discarded typed parser causes; plans
had no resolved installation destination.

Fixes:

- `-D` / `--driver` now selects one of the eight driver IDs. Shared global-option
  metadata drives parsing, help, reference, and completion. Directory selection
  is only `--root`. Duplicate options and driver/target conflicts fail.
- Native tool-only Neoenv parsing is adapted from the authoritative
  `neoenv-config/src/structured.rs` shapes and related native tests, with full
  MIT attribution. Ordered list/provider/platform syntax is preserved; non-tool
  domains and unknown fields fail. No runtime/evaluator or private path
  dependency was imported.
- Bin-root resolution is pure and takes explicit, environment, per-scope config,
  and platform inputs in that order. Owned-binary planning requires the resolved
  root and returns a rename-aware absolute destination without creating it.
- CLI diagnostics render safe filenames, locations, and redacted source causes
  while preserving stable codes and library cause chains. Scalar values and
  source snippets are never printed.
- Config documentation and examples reflect the actual native profile; the
  public planner example compiles as a doctest.

Class: strictness remains shared across string/map boundaries. YAML duplicate
maps and scalar coercion still fail. Bundle values are revalidated at the
planning boundary. No actual regex, shell grep, or asset extraction runs.

The authorized toolchain update pins Rust 1.100.0. Its new warnings exposed an
unused provider dependency and a platform-cfg boolean branch; both were corrected
without suppression.

Out of scope: SQLite, source/pin mutations, lifecycle execution, QuickJS,
network resolution, archive extraction, and Windows shim publication. A
planner result is not evidence of an installation.

Verified through current global Mise: `rustc --version` and `cargo --version`
both report 1.100.0; `cargo fmt --check`, strict workspace/all-target Clippy,
`cargo test --workspace`, and `cargo build --workspace` pass without warnings.
The suite passes 38 tests and two doctests, including executed Bash driver
completion and checked configuration-document examples.
