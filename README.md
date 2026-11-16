# Neoget

Neoget is a standalone, daemonless Rust package-management foundation. The workspace separates shared contracts (`neoget-core`), planning and authorization state (`neoget-engine`), provider interfaces (`neoget-providers`), and the `neoget` command-line presentation layer.

**Status:** initial scaffold only. The CLI generates help, a command reference, and shell completion from a single command specification; package operations are not implemented. No OS package manager, network access, persistence, GUI, TUI, or MCP server is included.

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

This scaffold does not implement providers or package execution. The initial provider matrix, networking, SQLite or other persistence, JSON/progress output, MCP, TUI/GUI, and native package-manager integrations are out of scope.
