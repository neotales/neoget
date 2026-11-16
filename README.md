# Neoget

Neoget is a standalone, daemonless Rust package-management foundation. The workspace separates shared contracts (`neoget-core`), planning and authorization state (`neoget-engine`), provider interfaces (`neoget-providers`), and the `neoget` command-line presentation layer.

**Status:** initial scaffold plus eight provider adapter command-construction modules (winget, Chocolatey, apt, pacman, DNF, YUM, GitHub CLI, Aqua). They identify applicable platforms and construct selected inventory/upgrade argv vectors. These are pre-execution plans: there is no provider lifecycle execution, installed-package reconciliation, or claimed install/upgrade success. The provider library does expose a low-level streamed process API for constructed invocations; the CLI does not invoke it or execute package commands.

## Prerequisites

Install Rust 1.98.1 with Cargo. The repository pins this toolchain in `rust-toolchain.toml`. The current development environment may not have Rust installed.

## Quality commands

Run from the repository root, in this order:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

Rustfmt formatting is required before linting. `docs/commands.md` is generated from the static `CommandSpec` tree in `crates/neoget/src/main.rs`; `cargo test --workspace` checks that the checked-in document remains in sync.

## v0 command grammar

The documented commands are `search`, `show`, `install`, `list`, `outdated`, `upgrade`, `uninstall`, `refresh`, `providers`, `plan`, `apply`, `status`, `continue`, `completion`, `tui`, and `mcp serve`. They are listed with usage in the [command reference](docs/commands.md).

Package targets use `[provider:]package[@exact-version]`, including scoped names such as `@scope/name`. Version text remains provider-native. Bare ambiguous names will require explicit resolution; installation will be idempotent and will not implicitly upgrade. Refresh is metadata-only. Provider-specific arguments after `--` are rejected until safe handling is designed.

Provider adapters currently expose command-construction foundations only. Native manager mutation is not run by the CLI: authorization, preflight, journaling, and persistence are not implemented. GitHub CLI currently lists GitHub CLI extensions; Aqua exposes its CLI list command, not a Neoget config-aware inventory or network resolver. The low-level process API can run an invocation but does not make package lifecycle changes safe or verified. It streams bounded UTF-8 chunks and reports timeout, cancellation, and output limits; portable standard-library cleanup targets the direct child and cannot guarantee terminating descendants that inherit pipes. Chocolatey inventory assumes CLI v2+, where `choco list` lists local packages and the removed `--local-only` option must not be passed ([official command docs](https://docs.chocolatey.org/en-us/choco/commands/list/)). Networking, SQLite or other persistence, JSON/progress output, MCP, TUI, and GUI remain out of scope.
