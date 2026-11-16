# Config/grammar implementation handoff

Diagnosis: the reverted partial attempts did not leave reusable code in the
worktree. Baseline reproduction `neoget install gh:owner/repo -D` returned
`unexpected argument '-D'`; there was no config parser, root resolution,
sources/pins schema, or executable selection plan.

The fix centralizes parsing/validation in `neoget-config`, keeps root selection
pure, normalizes command aliases/global options before static-schema validation,
and keeps the owned-binary planner side-effect-free. YAML map duplicates need a
custom Serde visitor: standard `BTreeMap` deserialization otherwise overwrites
duplicate declarations. This applies to every configuration map, not only tools.
Publicly constructible bundles are revalidated at the planning boundary.
The new negative tests also reproduced YAML string coercion of null. String-only
visitors now reject null/numeric/boolean values at every string boundary rather
than accidentally turning them into provider names, versions, or aliases.

The escalation brief did not include the earlier full grammar/schema or junior
handoff beyond two reverted incomplete attempts. This implementation defines
`-D` as `--root`, and documents its exact behavior. Review this assumption
against the original accepted specification before calling compatibility done.

The available Neoenv checkout has no commits or source files. HTTPS discovery
failed because authentication was unavailable; SSH fetch failed with
`Permission denied (publickey)`. No credentials were opened. Consequently no
Neoenv config code was copied, no path dependency was added, and upstream
Neoenv tool-schema compatibility is unverified. The tool-only interchange
parser is intentionally explicit and strict rather than claiming to support
unseen upstream behavior. Restore source access before adapting the real pure
config code and recording its MIT attribution.

Out of scope: SQLite, source/pin mutations, lifecycle execution, QuickJS,
network resolution, archive extraction, and Windows shim publication. A
planner result is not evidence of an installation.

Verification: all four README gates ran via global Mise (Rust 1.98.1):
`cargo fmt --check`, strict workspace/all-target Clippy, `cargo test --workspace`,
and `cargo build --workspace`. The test run passed 30 tests plus one doctest.
