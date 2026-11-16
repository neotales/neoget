# Neoget command reference

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
