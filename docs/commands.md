# Neoget command reference

## `sources`

`neoget sources ` — Source catalog declarations (documented, not implemented in v0)

## `sources list`

`neoget sources list ` — List configured sources (documented, not implemented in v0)

## `sources add`

`neoget sources add <name> <url> ` — Add a source (persistence unavailable) (documented, not implemented in v0)

## `sources remove`

`neoget sources remove <name> ` — Remove a source (persistence unavailable) (documented, not implemented in v0)

## `sources refresh`

`neoget sources refresh ` — Refresh source metadata (resolver unavailable) (documented, not implemented in v0)

## `pins`

`neoget pins ` — Exact version pins (documented, not implemented in v0)

## `pins list`

`neoget pins list ` — List configured pins (documented, not implemented in v0)

## `pins add`

`neoget pins add <target> <version> ` — Pin an exact version (persistence unavailable) (documented, not implemented in v0)

## `pins remove`

`neoget pins remove <name> ` — Remove a pin (persistence unavailable) (documented, not implemented in v0)

## `config`

`neoget config ` — Strict configuration validation

## `config check`

`neoget config check <file> ` — Validate a bundle v1 without side effects

## `config check-neoenv`

`neoget config check-neoenv <file> ` — Validate a native tool-only Neoenv file

## `search`

`neoget search <query> ` — Search package catalogs

## `show`

`neoget show <target> ` — Show package details

## `install`

`neoget install <target> ` — Install without implicit upgrade

## `list`

`neoget list ` — List installed packages

## `outdated`

`neoget outdated ` — List available updates

## `upgrade`

`neoget upgrade <target> ` — Upgrade a package explicitly

## `uninstall`

`neoget uninstall <target> ` — Remove a package

## `refresh`

`neoget refresh ` — Refresh provider metadata only

## `providers`

`neoget providers ` — List configured providers

## `plan`

`neoget plan <action> [target] ` — Create a plan without executing

Choices for `action`: search, show, install, list, outdated, upgrade, uninstall, refresh.

## `apply`

`neoget apply <plan> ` — Authorize and apply a plan

## `status`

`neoget status ` — Show operation status

## `continue`

`neoget continue <operation> ` — Continue an interrupted operation

## `completion`

`neoget completion <shell> ` — Print shell completion text

Choices for `shell`: bash, zsh, fish, powershell, nushell.

## `tui`

`neoget tui ` — Interactive interface (not available in v0) (documented, not implemented in v0)

## `mcp`

`neoget mcp ` — MCP service (not available in v0) (documented, not implemented in v0)

## `mcp serve`

`neoget mcp serve ` — Start MCP service (not available in v0) (documented, not implemented in v0)

Targets use `[provider:]package[@exact-version]`; scoped npm-style names such as `@scope/name` are valid. Provider arguments after `--` are rejected until safe forwarding is implemented.

Command aliases: i/add=install, ls=list, rm/remove=uninstall, up=upgrade, find=search, info=show.

Global options (once each, before or after the command):
  -D/--driver <driver> — Select a provider driver, never a directory
    Choices: winget, chocolatey, apt, pacman, dnf, yum, github, aqua.
  --root <directory> — Select the Neoget configuration/data root
  --config <file> — Select an explicit bundle file

Attached `-Dapt` and `--driver=apt` / `--root=dir` / `--config=file` are accepted. `-D` selects a driver and never selects a directory. Conflicting explicit driver/target-provider selections fail. Root precedence: `--root`, `NEOGET_ROOT`, platform user data directory plus `neoget`. Relative roots and explicit config files resolve against the original working directory. A root may contain one of `neoget.toml`, `neoget.yaml`, `neoget.yml`; multiple candidates are an error, not a precedence rule. Help, reference, and completion do not read configuration.

Only config checks and presentation execute today. Other commands fail with `CAPABILITY_UNAVAILABLE`; no database, scripts, package mutations, or Windows shim publication are performed.
