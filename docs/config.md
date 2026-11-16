# Configuration foundation

`neoget config check bundle.toml` (also `.yaml` / `.yml`) validates a strict
Neoget bundle v1. It does not resolve, fetch, install, or persist anything.
`config check-neoenv tools.toml` accepts the focused tool-only interchange
envelope below without `version`, `aliases`, `sources`, or `pins` at the root.
It is **not** a full Neoenv config loader: env, tasks, hooks, remote, runtime,
and scripts fail rather than disappearing silently. Compatibility with the
upstream Neoenv schema is not yet verified; see the implementation handoff.

```toml
version = 1

[aliases]
rg = "ripgrep"

[tools.ripgrep]
provider = "github"
package = "BurntSushi/ripgrep"
version = "14.1.0"
aliases = ["grep-tool"]
rename = "rg-owned"

[tools.ripgrep.asset]
template = "ripgrep-{version}-{arch}-{os}.tar.gz"

[tools.ripgrep.asset.member]
basename = "rg"

[sources.catalog]
provider = "aqua"
url = "https://example.org/catalog"

[pins]
ripgrep = "14.1.0"
```

The equivalent YAML uses the same mappings and scalar strings:

```yaml
version: 1
aliases: {rg: ripgrep}
tools:
  ripgrep:
    provider: github
    package: BurntSushi/ripgrep
    version: 14.1.0
    aliases: [grep-tool]
    rename: rg-owned
    asset:
      template: ripgrep-{version}-{arch}-{os}.tar.gz
      member: {basename: rg}
sources:
  catalog: {provider: aqua, url: "https://example.org/catalog"}
pins: {ripgrep: 14.1.0}
```

Unknown and duplicate keys are rejected at every level. Optional fields must be
omitted, not null. Only `version = 1` is supported. Omitted maps/lists are empty;
the tool-only envelope requires `tools`. Inputs are limited to 1 MiB and 1024
total bundle entries; each tool has at most 64 aliases. No config merging,
implicit imports, variable expansion, hooks, or evaluation occurs.

Tool names are portable ASCII basenames (128 bytes maximum). Provider IDs are
lowercase letters, digits, `_`, `-`; unknown provider capabilities are not
inferred. Packages and version strings are nonempty, whitespace/control-free,
at most 512 bytes. Versions are opaque exact values, not interpreted ranges.
Pins name canonical tools and may not contradict a declared version.

Aliases resolve directly to configured tools, never to shell commands, other
aliases, or arbitrary package targets. Aliases must be unique across both
declaration forms and cannot shadow canonical tool names. This lets a short
name reference a GitHub `owner/repository` without guessing a package provider.

GitHub tools require `asset`. Other providers may not declare asset or rename.
Asset selection requires exactly one of `exact` or `template`. Templates
substitute only `{version}`, `{os}`, `{arch}` into an exact asset filename;
these are explicit planner inputs, with no platform synonym normalization.
Unknown placeholders, shell syntax, regexes, globs, paths, and unsafe
substitutions fail. Filenames and expanded templates are capped at 128 bytes.
Selection is bounded to 4096 candidates, each at most 512 bytes; zero or multiple
matches are errors, including duplicate matching filenames.

`member` requires exactly one of `exact` (relative archive path) or `basename`
(literal basename, anywhere in the archive). This is deliberately constrained
grep-like filtering, not a regex/shell escape hatch. The pre-extraction API
requires complete member metadata. It rejects more than 4096 entries,
non-regular entries (including links/devices/directories), absolute paths,
backslashes, empty/`.`/`..` components, controls, and colon-prefixed drive
paths. Each path is at most 512 bytes and each component at most 128 bytes.
Exactly one matching regular file is required. This conservative foundation
does not yet accept directory metadata records. No extraction is implemented;
a future extractor must independently enforce no-follow and owned-root rules.

`plan_owned_binary` in `neoget-engine` checks tool/alias resolution, the configured
version, asset and optional member selection, then returns a proposal with the
`rename` basename (default: canonical tool name). Rename never affects native
packages. Publication basenames reject Windows device names, trailing dots,
leading hyphens, and path syntax. If no member is declared, the proposal is for
a raw asset, not permission to extract an archive. Archive recognition and
extraction are not implemented. Neither plans nor validation publish binaries
or Windows shims.

Root selection is documented in [the command reference](commands.md). Root
selection creates nothing. Config checks use their explicit file, independent
of selected-root config. Presentation never reads config. Other operations
validate an explicit config, or the one root bundle if present, then return
`CAPABILITY_UNAVAILABLE`; absence of a root bundle does not fabricate state.
An explicit missing/invalid file is an error, never a fallback. Default root
uses `LOCALAPPDATA/neoget` on Windows, `HOME/Library/Application Support/neoget`
on macOS, or `XDG_DATA_HOME/neoget` / `HOME/.local/share/neoget` elsewhere.
If no platform base or explicit/environment root is available, selection fails.

Sources and pins have grammar/help/completion contracts only. Their list/add/
remove operations do not claim working SQLite persistence. QuickJS, package
mutations, network resolution, archive extraction, and Windows shim publishing
remain explicitly unavailable.
