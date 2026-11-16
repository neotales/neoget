# Neoget

Neoget is a standalone, daemonless Rust package-management foundation. The workspace separates shared contracts (`neoget-core`), strict configuration (`neoget-config`), planning and authorization state (`neoget-engine`), provider interfaces (`neoget-providers`), and the `neoget` command-line presentation layer.

Bundle v1 TOML/YAML validation, tool aliases, root selection, and pure owned
GitHub binary selection/rename planning are available. See the
[config contract](docs/config.md) and [implementation handoff](docs/config-handoff.md)
for the native Neoenv tool-only profile and its explicit evaluator boundary.

**Status:** initial scaffold plus eight provider adapter command-construction modules (winget, Chocolatey, apt, pacman, DNF, YUM, GitHub CLI, Aqua). They identify applicable platforms and construct selected inventory/upgrade argv vectors. These are pre-execution plans: there is no provider lifecycle execution, installed-package reconciliation, or claimed install/upgrade success. The provider library does expose a low-level streamed process API for constructed invocations; the CLI does not invoke it or execute package commands.

## Prerequisites

Install Rust 1.100.0 with Cargo using global Mise (`mise use -g rust@1.100.0`).
The repository pins this toolchain in `rust-toolchain.toml`; run the gates
through the global Mise activation below.

## Quality commands

Run from the repository root, in this order:

```sh
mise exec -- cargo fmt --check
mise exec -- cargo clippy --workspace --all-targets -- -D warnings
mise exec -- cargo test --workspace
mise exec -- cargo build --workspace
```

Rustfmt formatting is required before linting. `docs/commands.md` is generated from the static `CommandSpec` tree in `crates/neoget/src/main.rs`; `cargo test --workspace` checks that the checked-in document remains in sync.

## v0 command grammar

The documented commands include `search`, `show`, `install`, `list`, `outdated`, `upgrade`, `uninstall`, `refresh`, `providers`, `sources`, `pins`, `config`, `plan`, `apply`, `status`, `continue`, `completion`, `tui`, and `mcp serve`. They are listed with usage in the [command reference](docs/commands.md). `-D` / `--driver` selects a driver; `--root` selects a directory.

Package targets use `[provider:]package[@exact-version]`, including scoped names such as `@scope/name`. Version text remains provider-native. Bare ambiguous names will require explicit resolution; installation will be idempotent and will not implicitly upgrade. Refresh is metadata-only. Provider-specific arguments after `--` are rejected until safe handling is designed.

Provider adapters currently expose command-construction foundations only. Native manager mutation is not run by the CLI: authorization, preflight, and journaling are not implemented. The engine now includes a SQLite-backed local state foundation for sources, exact pins, and persisted desired-state bundle plans; CLI source/pin operations are not yet wired to it. GitHub CLI currently lists GitHub CLI extensions; Aqua exposes its CLI list command, not a Neoget config-aware inventory or network resolver. The low-level process API can run an invocation but does not make package lifecycle changes safe or verified. It streams bounded UTF-8 chunks and reports timeout, cancellation, and output limits; portable standard-library cleanup targets the direct child and cannot guarantee terminating descendants that inherit pipes. Chocolatey inventory assumes CLI v2+, where `choco list` lists local packages and the removed `--local-only` option must not be passed ([official command docs](https://docs.chocolatey.org/en-us/choco/commands/list/)). Networking, JSON/progress output, MCP, TUI, and GUI remain out of scope. See the [state store contract](docs/state-store.md).
