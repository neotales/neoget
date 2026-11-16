# Neoenv configuration attribution

`src/neoenv.rs` adapts the native tool/provider/platform deserialization shapes
from Neoenv's `lib/neoenv-config/src/structured.rs` (`StructuredTool`,
`StructuredGithubRelease`, `StructuredPlatform`, and related provider records).
The tool-only root retains native `format_version` and ordered `tools` syntax;
the rest of the project evaluator is intentionally not copied.

The fixtures in `tests/fixtures/neoenv-tools.{toml,yaml}` adapt the native
explicit-URL, GitHub-release, and Aqua examples in
`lib/neoenv-config/src/tests.rs` and exercise the structured platform fields.
Neoget adds strict string scalars, duplicate-ID rejection, and input bounds.
No private or external path dependency is used.

Neoenv is distributed under the following license:

MIT License

Copyright (c) 2026 Neotales

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
