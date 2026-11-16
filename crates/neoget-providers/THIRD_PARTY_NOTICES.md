# Third-party notices

The process runner in `src/process.rs` adapts the design pattern of concurrent stdout/stderr draining, bounded pipe output, timeout polling, and child termination/reaping from Neoenv's `lib/neoenv-runtime/src/remote/client.rs`.

Neoenv is Copyright (c) Neotales and contributors and is distributed under the MIT License. The source is available in the Neoenv project. This Neoget implementation is a reimplementation of those patterns, not copied source code; no Neoenv crate or path dependency is used.
