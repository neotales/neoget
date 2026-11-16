//! CLI validation and generated presentation from one static command schema.
use std::env;
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy)]
enum GlobalKind {
    Driver,
    Root,
    Config,
}
struct GlobalOption {
    long: &'static str,
    short: Option<&'static str>,
    value: &'static str,
    summary: &'static str,
    choices: &'static [&'static str],
    kind: GlobalKind,
}
const DRIVERS: &[&str] = &[
    "winget",
    "chocolatey",
    "apt",
    "pacman",
    "dnf",
    "yum",
    "github",
    "aqua",
];
const GLOBAL_OPTIONS: &[GlobalOption] = &[
    GlobalOption {
        long: "--driver",
        short: Some("-D"),
        value: "driver",
        summary: "Select a provider driver, never a directory",
        choices: DRIVERS,
        kind: GlobalKind::Driver,
    },
    GlobalOption {
        long: "--root",
        short: None,
        value: "directory",
        summary: "Select the Neoget configuration/data root",
        choices: &[],
        kind: GlobalKind::Root,
    },
    GlobalOption {
        long: "--config",
        short: None,
        value: "file",
        summary: "Select an explicit bundle file",
        choices: &[],
        kind: GlobalKind::Config,
    },
];
fn global_help() -> String {
    let mut output = String::from("\nGlobal options (once each, before or after the command):\n");
    for option in GLOBAL_OPTIONS {
        output.push_str(&format!(
            "  {}{} <{}> — {}\n",
            option.short.map(|s| format!("{s}/")).unwrap_or_default(),
            option.long,
            option.value,
            option.summary
        ));
        if !option.choices.is_empty() {
            output.push_str(&format!("    Choices: {}.\n", option.choices.join(", ")));
        }
    }
    output
}

const ALIASES: &[(&str, &str)] = &[
    ("i", "install"),
    ("add", "install"),
    ("ls", "list"),
    ("rm", "uninstall"),
    ("remove", "uninstall"),
    ("up", "upgrade"),
    ("find", "search"),
    ("info", "show"),
];

const NAME_ARGUMENT: &[Argument] = &[Argument {
    name: "name",
    min: 1,
    max: 1,
    choices: &[],
}];
const SOURCE_CHILDREN: &[CommandSpec] = &[
    CommandSpec {
        name: "list",
        summary: "List configured sources",
        availability: Availability::Planned,
        arguments: NO_ARGUMENTS,
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "add",
        summary: "Add a source (persistence unavailable)",
        availability: Availability::Planned,
        arguments: &[
            Argument {
                name: "name",
                min: 1,
                max: 1,
                choices: &[],
            },
            Argument {
                name: "url",
                min: 1,
                max: 1,
                choices: &[],
            },
        ],
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "remove",
        summary: "Remove a source (persistence unavailable)",
        availability: Availability::Planned,
        arguments: NAME_ARGUMENT,
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "refresh",
        summary: "Refresh source metadata (resolver unavailable)",
        availability: Availability::Planned,
        arguments: NO_ARGUMENTS,
        children: NO_CHILDREN,
    },
];
const PIN_CHILDREN: &[CommandSpec] = &[
    CommandSpec {
        name: "list",
        summary: "List configured pins",
        availability: Availability::Planned,
        arguments: NO_ARGUMENTS,
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "add",
        summary: "Pin an exact version (persistence unavailable)",
        availability: Availability::Planned,
        arguments: &[
            Argument {
                name: "target",
                min: 1,
                max: 1,
                choices: &[],
            },
            Argument {
                name: "version",
                min: 1,
                max: 1,
                choices: &[],
            },
        ],
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "remove",
        summary: "Remove a pin (persistence unavailable)",
        availability: Availability::Planned,
        arguments: NAME_ARGUMENT,
        children: NO_CHILDREN,
    },
];
const CONFIG_CHILDREN: &[CommandSpec] = &[
    CommandSpec {
        name: "check",
        summary: "Validate a bundle v1 without side effects",
        availability: Availability::Available,
        arguments: &[Argument {
            name: "file",
            min: 1,
            max: 1,
            choices: &[],
        }],
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "check-neoenv",
        summary: "Validate a native tool-only Neoenv file",
        availability: Availability::Available,
        arguments: &[Argument {
            name: "file",
            min: 1,
            max: 1,
            choices: &[],
        }],
        children: NO_CHILDREN,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Availability {
    Available,
    Planned,
}

#[derive(Clone, Copy)]
struct Argument {
    name: &'static str,
    min: usize,
    max: usize,
    choices: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct CommandSpec {
    name: &'static str,
    summary: &'static str,
    availability: Availability,
    arguments: &'static [Argument],
    children: &'static [CommandSpec],
}

const NO_ARGUMENTS: &[Argument] = &[];
const NO_CHILDREN: &[CommandSpec] = &[];
const ACTIONS: &[&str] = &[
    "search",
    "show",
    "install",
    "list",
    "outdated",
    "upgrade",
    "uninstall",
    "refresh",
];
const SHELLS: &[&str] = &["bash", "zsh", "fish", "powershell", "nushell"];
const MCP_CHILDREN: &[CommandSpec] = &[CommandSpec {
    name: "serve",
    summary: "Start MCP service (not available in v0)",
    availability: Availability::Planned,
    arguments: NO_ARGUMENTS,
    children: NO_CHILDREN,
}];
const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        name: "sources",
        summary: "Source catalog declarations",
        availability: Availability::Planned,
        arguments: NO_ARGUMENTS,
        children: SOURCE_CHILDREN,
    },
    CommandSpec {
        name: "pins",
        summary: "Exact version pins",
        availability: Availability::Planned,
        arguments: NO_ARGUMENTS,
        children: PIN_CHILDREN,
    },
    CommandSpec {
        name: "config",
        summary: "Strict configuration validation",
        availability: Availability::Available,
        arguments: NO_ARGUMENTS,
        children: CONFIG_CHILDREN,
    },
    CommandSpec {
        name: "search",
        summary: "Search package catalogs",
        availability: Availability::Available,
        arguments: &[Argument {
            name: "query",
            min: 1,
            max: 1,
            choices: &[],
        }],
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "show",
        summary: "Show package details",
        availability: Availability::Available,
        arguments: &[Argument {
            name: "target",
            min: 1,
            max: 1,
            choices: &[],
        }],
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "install",
        summary: "Install without implicit upgrade",
        availability: Availability::Available,
        arguments: &[Argument {
            name: "target",
            min: 1,
            max: 1,
            choices: &[],
        }],
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "list",
        summary: "List installed packages",
        availability: Availability::Available,
        arguments: NO_ARGUMENTS,
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "outdated",
        summary: "List available updates",
        availability: Availability::Available,
        arguments: NO_ARGUMENTS,
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "upgrade",
        summary: "Upgrade a package explicitly",
        availability: Availability::Available,
        arguments: &[Argument {
            name: "target",
            min: 1,
            max: 1,
            choices: &[],
        }],
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "uninstall",
        summary: "Remove a package",
        availability: Availability::Available,
        arguments: &[Argument {
            name: "target",
            min: 1,
            max: 1,
            choices: &[],
        }],
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "refresh",
        summary: "Refresh provider metadata only",
        availability: Availability::Available,
        arguments: NO_ARGUMENTS,
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "providers",
        summary: "List configured providers",
        availability: Availability::Available,
        arguments: NO_ARGUMENTS,
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "plan",
        summary: "Create a plan without executing",
        availability: Availability::Available,
        arguments: &[
            Argument {
                name: "action",
                min: 1,
                max: 1,
                choices: ACTIONS,
            },
            Argument {
                name: "target",
                min: 0,
                max: 1,
                choices: &[],
            },
        ],
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "apply",
        summary: "Authorize and apply a plan",
        availability: Availability::Available,
        arguments: &[Argument {
            name: "plan",
            min: 1,
            max: 1,
            choices: &[],
        }],
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "status",
        summary: "Show operation status",
        availability: Availability::Available,
        arguments: NO_ARGUMENTS,
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "continue",
        summary: "Continue an interrupted operation",
        availability: Availability::Available,
        arguments: &[Argument {
            name: "operation",
            min: 1,
            max: 1,
            choices: &[],
        }],
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "completion",
        summary: "Print shell completion text",
        availability: Availability::Available,
        arguments: &[Argument {
            name: "shell",
            min: 1,
            max: 1,
            choices: SHELLS,
        }],
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "tui",
        summary: "Interactive interface (not available in v0)",
        availability: Availability::Planned,
        arguments: NO_ARGUMENTS,
        children: NO_CHILDREN,
    },
    CommandSpec {
        name: "mcp",
        summary: "MCP service (not available in v0)",
        availability: Availability::Planned,
        arguments: NO_ARGUMENTS,
        children: MCP_CHILDREN,
    },
];

fn command_help(path: &[&CommandSpec]) -> String {
    let command = path.last().copied();
    let mut output = String::from("neoget — daemonless package management\n\nUsage: neoget");
    for item in path {
        output.push_str(&format!(" {}", item.name));
    }
    if let Some(command) = command {
        for argument in command.arguments {
            output.push_str(&format!(
                " {}",
                if argument.min == 0 {
                    format!("[{}]", argument.name)
                } else {
                    format!("<{}>", argument.name)
                }
            ));
        }
    } else {
        output.push_str(" <command>");
    }
    output.push_str(" [--help]\n\n");
    if let Some(command) = command {
        output.push_str(&format!("{}\n\n", command.summary));
        if !command.children.is_empty() {
            output.push_str("Subcommands:\n");
            for child in command.children {
                output.push_str(&format!("  {:<12} {}\n", child.name, child.summary));
            }
        }
        if let Some(argument) = command
            .arguments
            .iter()
            .find(|argument| !argument.choices.is_empty())
        {
            output.push_str(&format!(
                "Choices for `{}`: {}.\n",
                argument.name,
                argument.choices.join(", ")
            ));
        }
        if command.availability == Availability::Planned {
            output.push_str("Availability: documented, not implemented in v0.\n");
        }
    } else {
        output.push_str("Commands:\n");
        for item in COMMANDS {
            output.push_str(&format!("  {:<12} {}\n", item.name, item.summary));
        }
        output.push_str("\nAliases: i/add=install, ls=list, rm/remove=uninstall, up=upgrade, find=search, info=show.\n");
    }
    output.push_str(&global_help());
    output.push_str("\nTarget: [provider:]package[@exact-version] (scoped names such as @scope/name are supported)\n");
    output
}

fn reference() -> String {
    fn append(output: &mut String, command: &CommandSpec, prefix: &str) {
        let name = format!("{prefix}{}", command.name);
        let usage = command
            .arguments
            .iter()
            .map(|argument| {
                if argument.min == 0 {
                    format!("[{}]", argument.name)
                } else {
                    format!("<{}>", argument.name)
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        output.push_str(&format!(
            "## `{name}`\n\n`neoget {name}{}{} ` — {}{}\n\n",
            if usage.is_empty() { "" } else { " " },
            usage,
            command.summary,
            if command.availability == Availability::Planned {
                " (documented, not implemented in v0)"
            } else {
                ""
            }
        ));
        for argument in command.arguments {
            if !argument.choices.is_empty() {
                output.push_str(&format!(
                    "Choices for `{}`: {}.\n\n",
                    argument.name,
                    argument.choices.join(", ")
                ));
            }
        }
        for child in command.children {
            append(output, child, &format!("{name} "));
        }
    }
    let mut output = String::from("# Neoget command reference\n\n");
    for command in COMMANDS {
        append(&mut output, command, "");
    }
    output.push_str("Targets use `[provider:]package[@exact-version]`; scoped npm-style names such as `@scope/name` are valid. Provider arguments after `--` are rejected until safe forwarding is implemented.\n");
    output.push_str("\nCommand aliases: i/add=install, ls=list, rm/remove=uninstall, up=upgrade, find=search, info=show.\n");
    output.push_str(&global_help());
    output.push_str("\nAttached `-Dapt` and `--driver=apt` / `--root=dir` / `--config=file` are accepted. `-D` selects a driver and never selects a directory. Conflicting explicit driver/target-provider selections fail. Root precedence: `--root`, `NEOGET_ROOT`, platform user data directory plus `neoget`. Relative roots and explicit config files resolve against the original working directory. A root may contain one of `neoget.toml`, `neoget.yaml`, `neoget.yml`; multiple candidates are an error, not a precedence rule. Help, reference, and completion do not read configuration.\n\nOnly config checks and presentation execute today. Other commands fail with `CAPABILITY_UNAVAILABLE`; no database, scripts, package mutations, or Windows shim publication are performed.\n");
    output
}

fn top_candidates() -> Vec<&'static str> {
    COMMANDS
        .iter()
        .map(|command| command.name)
        .chain(ALIASES.iter().map(|(alias, _)| *alias))
        .collect()
}

fn candidates_for(tokens: &[&str]) -> Vec<&'static str> {
    if tokens.is_empty() {
        return top_candidates();
    }
    let name = ALIASES
        .iter()
        .find(|(alias, _)| *alias == tokens[0])
        .map_or(tokens[0], |(_, name)| *name);
    let Some(command) = COMMANDS.iter().find(|command| command.name == name) else {
        return Vec::new();
    };
    if tokens.len() == 1 && !command.children.is_empty() {
        return command.children.iter().map(|child| child.name).collect();
    }
    if !command.children.is_empty() {
        let Some(child) = command
            .children
            .iter()
            .find(|child| tokens.get(1) == Some(&child.name))
        else {
            return Vec::new();
        };
        return choices_at(child, tokens.len().saturating_sub(2));
    }
    choices_at(command, tokens.len().saturating_sub(1))
}

fn choices_at(command: &CommandSpec, position: usize) -> Vec<&'static str> {
    command
        .arguments
        .get(position)
        .map(|argument| argument.choices.to_vec())
        .unwrap_or_default()
}

fn nushell_completion() -> String {
    fn global_parameters() -> Vec<String> {
        GLOBAL_OPTIONS
            .iter()
            .map(|option| {
                let flag = option
                    .short
                    .map(|short| format!("({short})"))
                    .unwrap_or_default();
                let value = if option.choices.is_empty() {
                    "path".into()
                } else {
                    format!("string@'nu-complete neoget {}'", option.value)
                };
                format!("{}{flag}: {value}", option.long)
            })
            .collect()
    }
    fn completer(output: &mut String, name: &str, choices: &[&str]) {
        let values = choices
            .iter()
            .map(|value| format!("'{value}'"))
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!("def \"nu-complete {name}\" [] {{ [{values}] }}\n"));
    }

    fn append(output: &mut String, commands: &[CommandSpec], prefix: &str) {
        for command in commands {
            let name = format!("{prefix} {}", command.name);
            let mut parameters = Vec::new();
            if !command.children.is_empty() {
                let choices = command
                    .children
                    .iter()
                    .map(|child| child.name)
                    .collect::<Vec<_>>();
                completer(output, &format!("{name} subcommand"), &choices);
                parameters.push(format!(
                    "subcommand?: string@'nu-complete {name} subcommand'"
                ));
            }
            for argument in command.arguments {
                let mut parameter = format!("{}?: string", argument.name);
                if !argument.choices.is_empty() {
                    let completer_name = format!("{name} {}", argument.name);
                    completer(output, &completer_name, argument.choices);
                    parameter.push_str(&format!("@'nu-complete {completer_name}'"));
                }
                parameters.push(parameter);
            }
            parameters.extend(global_parameters());
            parameters.push("--help(-h)".into());
            output.push_str(&format!("extern \"{name}\" [{}]\n", parameters.join(", ")));
            append(output, command.children, &name);
        }
    }

    // Optional positionals allow bare invocation and help without required values.
    // The executable remains responsible for validation; externs never wrap execution.
    let mut output =
        String::from("# Completion signatures only; argument validation belongs to neoget.\n");
    completer(&mut output, "neoget command", &top_candidates());
    for option in GLOBAL_OPTIONS
        .iter()
        .filter(|option| !option.choices.is_empty())
    {
        completer(
            &mut output,
            &format!("neoget {}", option.value),
            option.choices,
        );
    }
    output.push_str(&format!("extern neoget [command?: string@'nu-complete neoget command', ...args: string, {}, --help(-h), --reference]\n", global_parameters().join(", ")));
    append(&mut output, COMMANDS, "neoget");
    for (alias, canonical) in ALIASES {
        let command = COMMANDS
            .iter()
            .find(|command| command.name == *canonical)
            .expect("canonical alias");
        append(
            &mut output,
            &[CommandSpec {
                name: alias,
                ..*command
            }],
            "neoget",
        );
    }
    output
}

fn completion(shell: &str) -> Result<String, String> {
    if !SHELLS.contains(&shell) {
        return Err(format!(
            "unsupported shell '{shell}'; choose {}",
            SHELLS.join(", ")
        ));
    }
    let top = top_candidates()
        .into_iter()
        .chain(
            GLOBAL_OPTIONS
                .iter()
                .flat_map(|option| std::iter::once(option.long).chain(option.short)),
        )
        .collect::<Vec<_>>()
        .join(" ");
    let choices: Vec<_> = COMMANDS
        .iter()
        .map(|command| (command.name, candidates_for(&[command.name]).join(" ")))
        .filter(|(_, choices)| !choices.is_empty())
        .collect();
    let output = match shell {
        "bash" => {
            let globals = GLOBAL_OPTIONS
                .iter()
                .map(|option| {
                    let names = option
                        .short
                        .map(|short| format!("|{short}"))
                        .unwrap_or_default();
                    let values = if option.choices.is_empty() {
                        match option.kind {
                            GlobalKind::Root => "-d".into(),
                            _ => "-f".into(),
                        }
                    } else {
                        format!("-W '{}'", option.choices.join(" "))
                    };
                    format!(
                        "    {}{names}) COMPREPLY=( $(compgen {values} -- \"$cur\") ); return ;;\n",
                        option.long
                    )
                })
                .collect::<String>();
            let attached = GLOBAL_OPTIONS.iter().filter(|option| !option.choices.is_empty()).map(|option| {
                let long = option.choices.iter().map(|value| format!("{}={value}", option.long)).collect::<Vec<_>>().join(" ");
                let mut rules = format!("    {}=*) COMPREPLY=( $(compgen -W '{long}' -- \"$cur\") ); return ;;\n", option.long);
                if let Some(short) = option.short {
                    let values = option.choices.iter().map(|value| format!("{short}{value}")).collect::<Vec<_>>().join(" ");
                    rules.push_str(&format!("    {short}*) COMPREPLY=( $(compgen -W '{values}' -- \"$cur\") ); return ;;\n"));
                }
                rules
            }).collect::<String>();
            let rules = choices
                .iter()
                .map(|(name, values)| {
                    format!("    2:{name}) COMPREPLY=( $(compgen -W '{values}' -- \"$cur\") ) ;;\n")
                })
                .collect::<String>();
            format!(
                "_neoget() {{\n  local cur\n  cur=\"${{COMP_WORDS[COMP_CWORD]}}\"\n  case \"${{COMP_WORDS[COMP_CWORD-1]}}\" in\n{globals}  esac\n  case \"$cur\" in\n{attached}  esac\n  case \"${{COMP_CWORD}}:${{COMP_WORDS[1]}}\" in\n    1:*) COMPREPLY=( $(compgen -W '{top}' -- \"$cur\") ) ;;\n{rules}    *) COMPREPLY=() ;;\n  esac\n}}\ncomplete -F _neoget neoget\n"
            )
        }
        "zsh" => {
            let globals = GLOBAL_OPTIONS
                .iter()
                .map(|option| {
                    let values = if option.choices.is_empty() {
                        "_files".into()
                    } else {
                        format!("({})", option.choices.join(" "))
                    };
                    let long = format!(
                        "'{}[{}]:{}:{values}'",
                        option.long, option.summary, option.value
                    );
                    match option.short {
                        Some(short) => format!(
                            "'{short}[{}]:{}:{values}' {long}",
                            option.summary, option.value
                        ),
                        None => long,
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            let rules = choices
                .iter()
                .map(|(name, values)| format!("    {name}) _values argument {values} ;;\n"))
                .collect::<String>();
            format!(
                "#compdef neoget\n_arguments {globals} '1:command:({top})' '2:argument:->argument'\nif (( CURRENT == 3 )); then\n  case $words[2] in\n{rules}  esac\nfi\n"
            )
        }
        "fish" => {
            let mut text = String::from("complete -c neoget -f\n");
            for option in GLOBAL_OPTIONS {
                let short = option
                    .short
                    .map(|short| format!(" -s {}", &short[1..]))
                    .unwrap_or_default();
                text.push_str(&format!(
                    "complete -c neoget -l {}{short} -r -d '{}'{}\n",
                    &option.long[2..],
                    option.summary,
                    if option.choices.is_empty() {
                        String::new()
                    } else {
                        format!(" -f -a '{}'", option.choices.join(" "))
                    }
                ));
            }
            for command in COMMANDS {
                text.push_str(&format!(
                    "complete -c neoget -n '__fish_use_subcommand' -a '{}' -d '{}'\n",
                    command.name, command.summary
                ));
            }
            for (alias, _) in ALIASES {
                text.push_str(&format!(
                    "complete -c neoget -n '__fish_use_subcommand' -a '{alias}'\n"
                ));
            }
            for (name, values) in &choices {
                text.push_str(&format!("complete -c neoget -n '__fish_seen_subcommand_from {name}; and test (count (commandline -opc)) -eq 2' -a '{values}'\n"));
            }
            text
        }
        "powershell" => {
            let globals = GLOBAL_OPTIONS.iter().filter(|option| !option.choices.is_empty()).map(|option| {
                let short = option.short.map(|short| format!(" -or $previous -eq '{short}'")).unwrap_or_default();
                format!("if ($previous -eq '{}'{short}) {{ $candidates='{}'.Split(' '); $candidates | Where-Object {{ $_ -like \"$wordToComplete*\" }}; return }}; ", option.long, option.choices.join(" "))
            }).collect::<String>();
            let attached = GLOBAL_OPTIONS.iter().filter(|option| !option.choices.is_empty()).flat_map(|option| {
                std::iter::once(format!("{}=", option.long)).chain(option.short.map(str::to_owned)).map(|prefix| {
                    let values = option.choices.iter().map(|choice| format!("{prefix}{choice}")).collect::<Vec<_>>().join(" ");
                    format!("if ($wordToComplete -like '{prefix}*') {{ '{values}'.Split(' ') | Where-Object {{ $_ -like \"$wordToComplete*\" }}; return }}; ")
                }).collect::<Vec<_>>()
            }).collect::<String>();
            let rules = choices.iter().map(|(name, values)| format!(" elseif ($words[1] -eq '{name}' -and $words.Count -le 3) {{ '{values}'.Split(' ') }}")).collect::<String>();
            format!(
                "Register-ArgumentCompleter -Native -CommandName neoget -ScriptBlock {{ param($wordToComplete,$commandAst,$cursorPosition) $words=$commandAst.ToString().Split(' '); $previous=if ($words.Count -ge 2) {{ $words[$words.Count-2] }} else {{ '' }}; {globals}{attached}$candidates=if ($words.Count -le 2) {{ '{top}'.Split(' ') }}{rules} else {{ @() }}; $candidates | Where-Object {{ $_ -like \"$wordToComplete*\" }} }}\n"
            )
        }
        "nushell" => nushell_completion(),
        _ => unreachable!("shell validated above"),
    };
    Ok(output)
}

enum Invocation<'a> {
    Help(Vec<&'static CommandSpec>),
    Execute(Vec<&'static CommandSpec>, &'a [String]),
}

fn validate(args: &[String]) -> Result<Invocation<'_>, String> {
    if args.iter().any(|arg| arg == "--") {
        return Err("provider argument forwarding after '--' is not supported yet".into());
    }
    let mut path = Vec::new();
    let mut cursor = args;
    let mut commands = COMMANDS;
    while let Some(token) = cursor.first() {
        let Some(command) = commands.iter().find(|command| command.name == token) else {
            break;
        };
        path.push(command);
        cursor = &cursor[1..];
        if command.children.is_empty() {
            break;
        }
        commands = command.children;
    }
    let command = path.last().ok_or_else(|| {
        format!(
            "unknown command '{}'; run 'neoget --help'",
            args.first().map(String::as_str).unwrap_or("")
        )
    })?;
    if cursor
        .first()
        .is_some_and(|token| token == "-h" || token == "--help")
    {
        if cursor.len() != 1 {
            return Err(format!("unexpected argument '{}' after help", cursor[1]));
        }
        return Ok(Invocation::Help(path));
    }
    if !command.children.is_empty() {
        let child = cursor
            .first()
            .ok_or_else(|| format!("'{}' requires a subcommand", command.name))?;
        return Err(format!(
            "unknown subcommand '{child}' for '{}'",
            command.name
        ));
    }
    let mut remaining = cursor;
    for argument in command.arguments {
        let consumed = remaining.len().min(argument.max);
        if consumed < argument.min {
            return Err(format!(
                "missing <{}> for '{}'",
                argument.name, command.name
            ));
        }
        for value in &remaining[..consumed] {
            if value.starts_with('-') {
                return Err("unknown option; run 'neoget --help'".into());
            }
            if !argument.choices.is_empty() && !argument.choices.contains(&value.as_str()) {
                return Err(format!(
                    "invalid {} '{}'; choose {}",
                    argument.name,
                    value,
                    argument.choices.join(", ")
                ));
            }
        }
        remaining = &remaining[consumed..];
    }
    if !remaining.is_empty() {
        return Err(format!(
            "unexpected argument '{}' for '{}'",
            remaining[0], command.name
        ));
    }
    if command.name == "plan" {
        let action = cursor.first().map(String::as_str).unwrap_or_default();
        let has_target = cursor.len() == 2;
        let needs_target = matches!(
            action,
            "search" | "show" | "install" | "upgrade" | "uninstall"
        );
        if needs_target && !has_target {
            return Err(format!("plan action '{action}' requires a target"));
        }
        if !needs_target && has_target {
            return Err(format!("plan action '{action}' does not accept a target"));
        }
    }
    let target = if command.name == "plan" {
        match cursor.first().map(String::as_str) {
            Some("show" | "install" | "upgrade" | "uninstall") => cursor.get(1),
            _ => None,
        }
    } else if matches!(command.name, "show" | "install" | "upgrade" | "uninstall")
        || (path.first().is_some_and(|command| command.name == "pins") && command.name == "add")
    {
        cursor.first()
    } else {
        None
    };
    if let Some(target) = target {
        neoget_core::PackageTarget::parse(target).map_err(|error| error.to_string())?;
    }
    Ok(Invocation::Execute(path, cursor))
}

fn run_command(args: &[String], options: &Options) -> Result<String, String> {
    if args.is_empty() || args == ["--help"] || args == ["-h"] {
        return Ok(command_help(&[]));
    }
    if args == ["--reference"] {
        return Ok(reference());
    }
    // Presentation is resolved by the schema before any command execution.
    let (path, arguments) = match validate(args)? {
        Invocation::Help(path) => return Ok(command_help(&path)),
        Invocation::Execute(path, arguments) => (path, arguments),
    };
    let command = path.last().expect("validated command");
    if command.name == "completion" {
        return completion(&arguments[0]);
    }
    if path.first().is_some_and(|command| command.name == "config") {
        check_config_file(Path::new(&arguments[0]), command.name == "check-neoenv")?;
        return Ok("Configuration valid (no operations executed).\n".into());
    }
    if let Some(driver) = &options.driver {
        let target = if command.name == "plan"
            && arguments.first().is_some_and(|action| {
                matches!(
                    action.as_str(),
                    "show" | "install" | "upgrade" | "uninstall"
                )
            }) {
            arguments.get(1)
        } else if matches!(command.name, "show" | "install" | "upgrade" | "uninstall")
            || (path.first().is_some_and(|command| command.name == "pins") && command.name == "add")
        {
            arguments.first()
        } else {
            None
        };
        if let Some(target) = target
            && neoget_core::PackageTarget::parse(target)
                .map_err(|error| error.to_string())?
                .provider
                .is_some_and(|provider| provider.as_str() != driver)
        {
            return Err(
                "DRIVER_CONFLICT: explicit target provider differs from selected driver".into(),
            );
        }
    }
    validate_selected_config(options)?;
    let driver = options
        .driver
        .as_ref()
        .map(|driver| format!(" (driver {driver})"))
        .unwrap_or_default();
    if command.availability == Availability::Planned {
        return Err(format!(
            "CAPABILITY_UNAVAILABLE: '{}' is documented but not implemented in v0{driver}",
            command.name
        ));
    }
    Err(format!(
        "CAPABILITY_UNAVAILABLE: '{}' is specified but its operation is not implemented in v0{driver}",
        command.name
    ))
}

#[derive(Default)]
struct Options {
    driver: Option<String>,
    root: Option<PathBuf>,
    config: Option<PathBuf>,
}

fn grammar(args: &[String]) -> Result<(Vec<String>, Options), String> {
    let mut options = Options::default();
    let mut tokens = Vec::new();
    let mut cursor = args.iter();
    while let Some(token) = cursor.next() {
        if token == "--" {
            return Err("provider argument forwarding after '--' is not supported yet".into());
        }
        let option = GLOBAL_OPTIONS.iter().find_map(|option| {
            if token == option.long || option.short.is_some_and(|short| token == short) {
                Some((option, None))
            } else {
                token
                    .strip_prefix(&format!("{}=", option.long))
                    .or_else(|| option.short.and_then(|short| token.strip_prefix(short)))
                    .map(|value| (option, Some(value)))
            }
        });
        if let Some((option, attached)) = option {
            let value = attached.or_else(|| cursor.next().map(String::as_str));
            let value = value
                .filter(|v| !v.is_empty() && !v.starts_with('-'))
                .ok_or("global option requires a nonempty value")?;
            if !option.choices.is_empty() && !option.choices.contains(&value) {
                return Err(format!(
                    "DRIVER_INVALID: choose {}",
                    option.choices.join(", ")
                ));
            }
            let duplicate = match option.kind {
                GlobalKind::Driver => options.driver.replace(value.into()).is_some(),
                GlobalKind::Root => options.root.replace(PathBuf::from(value)).is_some(),
                GlobalKind::Config => options.config.replace(PathBuf::from(value)).is_some(),
            };
            if duplicate {
                return Err("duplicate global option".into());
            }
        } else {
            tokens.push(token.clone());
        }
    }
    if let Some(first) = tokens.first_mut()
        && let Some((_, canonical)) = ALIASES.iter().find(|(alias, _)| *alias == first)
    {
        *first = (*canonical).into();
    }
    Ok((tokens, options))
}

fn read_config(path: &Path) -> Result<(String, neoget_config::Format), String> {
    let format = match path.extension().and_then(|extension| extension.to_str()) {
        Some("toml") => neoget_config::Format::Toml,
        Some("yaml" | "yml") => neoget_config::Format::Yaml,
        _ => {
            return Err(format!(
                "CONFIG_FORMAT: {}: expected .toml, .yaml, or .yml",
                safe_config_name(path)
            ));
        }
    };
    let file = File::open(path).map_err(|cause| {
        render_config_error(
            path,
            "",
            &neoget_core::CoreError::provider_failure_with_source(
                "CONFIG_IO",
                "cannot open configuration",
                cause,
            ),
        )
    })?;
    let mut input = String::new();
    file.take(1024 * 1024 + 1)
        .read_to_string(&mut input)
        .map_err(|cause| {
            render_config_error(
                path,
                "",
                &neoget_core::CoreError::provider_failure_with_source(
                    "CONFIG_IO",
                    "cannot read UTF-8 configuration",
                    cause,
                ),
            )
        })?;
    if input.len() > 1024 * 1024 {
        return Err(format!(
            "CONFIG_INVALID: {}: configuration exceeds 1 MiB",
            safe_config_name(path)
        ));
    }
    Ok((input, format))
}

fn safe_config_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| {
            !name.is_empty()
                && name.len() <= 128
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        })
        .unwrap_or("<config>")
        .to_owned()
}

fn render_config_error(path: &Path, input: &str, error: &neoget_core::CoreError) -> String {
    use std::error::Error;
    let name = safe_config_name(path);
    let mut output = format!("{error}\n  config: {name}");
    let mut cause = error.source();
    // Bound presentation even if a third-party error ever contains a cycle.
    for _ in 0..8 {
        let Some(current) = cause else { break };
        if let Some(detail) = neoget_config::parse_diagnostic(current, input) {
            output.push_str(&format!(
                "\n  caused by: {name}:{}:{}: {}",
                detail.line, detail.column, detail.message
            ));
        } else if let Some(io) = current.downcast_ref::<std::io::Error>() {
            output.push_str(&format!("\n  caused by: {io}"));
        } else {
            output.push_str("\n  caused by: configuration processing failed (details redacted)");
        }
        cause = current.source();
    }
    output
}

fn check_config_file(path: &Path, neoenv: bool) -> Result<(), String> {
    let (input, format) = read_config(path)?;
    let result = if neoenv {
        neoget_config::parse_neoenv_tools(&input, format).map(|_| ())
    } else {
        neoget_config::parse_bundle(&input, format).map(|_| ())
    };
    result.map_err(|error| render_config_error(path, &input, &error))
}

fn validate_selected_config(options: &Options) -> Result<(), String> {
    let cwd = env::current_dir().map_err(|_| "CONFIG_IO: cannot determine working directory")?;
    let environment = env::var_os("NEOGET_ROOT");
    let base = if cfg!(windows) {
        env::var_os("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library/Application Support")
                .into_os_string()
        })
    } else {
        env::var_os("XDG_DATA_HOME").or_else(|| {
            env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".local/share").into_os_string())
        })
    };
    let default = base.map(PathBuf::from).map(|base| base.join("neoget"));
    if default.is_none() && options.root.is_none() && environment.is_none() {
        return Err("CONFIG_ROOT_UNAVAILABLE: supply --root or NEOGET_ROOT".into());
    }
    let root = neoget_config::select_root(
        options.root.as_deref(),
        environment.as_deref().map(Path::new),
        default.as_deref().unwrap_or(&cwd),
        &cwd,
    )
    .map_err(|error| error.to_string())?;
    let selected = if let Some(config) = &options.config {
        Some(config.clone())
    } else {
        let mut candidates = Vec::new();
        for name in ["neoget.toml", "neoget.yaml", "neoget.yml"] {
            let path = root.join(name);
            match path.try_exists() {
                Ok(true) => candidates.push(path),
                Ok(false) => {}
                Err(cause) => {
                    return Err(render_config_error(
                        &path,
                        "",
                        &neoget_core::CoreError::provider_failure_with_source(
                            "CONFIG_IO",
                            "cannot inspect root configuration",
                            cause,
                        ),
                    ));
                }
            }
        }
        if candidates.len() > 1 {
            return Err("CONFIG_AMBIGUOUS: multiple root configuration files".into());
        }
        candidates.pop()
    };
    if let Some(path) = selected {
        check_config_file(&path, false)?;
    }
    Ok(())
}

fn run(args: &[String]) -> Result<String, String> {
    let (tokens, options) = grammar(args)?;
    run_command(&tokens, &options)
}

fn main() {
    match run(&env::args().skip(1).collect::<Vec<_>>()) {
        Ok(output) => print!("{output}"),
        Err(message) => {
            eprintln!("neoget: {message}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }

    #[test]
    fn parser_enforces_schema_cardinality_choices_subcommands_and_help() {
        assert!(
            run(&strings(&["install", "pkg"]))
                .unwrap_err()
                .contains("not implemented")
        );
        assert!(
            run(&strings(&["list", "extra"]))
                .unwrap_err()
                .contains("unexpected argument")
        );
        assert!(
            run(&strings(&["completion", "ruby"]))
                .unwrap_err()
                .contains("invalid shell")
        );
        assert!(
            run(&strings(&["completion"]))
                .unwrap_err()
                .contains("missing <shell>")
        );
        assert!(
            run(&strings(&["completion", "bash", "extra"]))
                .unwrap_err()
                .contains("unexpected argument")
        );
        assert!(
            run(&strings(&["plan", "invented"]))
                .unwrap_err()
                .contains("invalid action")
        );
        assert!(
            run(&strings(&["plan", "install"]))
                .unwrap_err()
                .contains("requires a target")
        );
        assert!(
            run(&strings(&["plan", "refresh", "pkg"]))
                .unwrap_err()
                .contains("does not accept")
        );
        assert!(
            run(&strings(&["mcp", "serve"]))
                .unwrap_err()
                .contains("not implemented")
        );
        assert!(
            run(&strings(&["mcp", "invalid"]))
                .unwrap_err()
                .contains("unknown subcommand")
        );
        assert!(
            run(&strings(&["install", "--help"]))
                .unwrap()
                .contains("Usage: neoget install <target>")
        );
        assert!(
            run(&strings(&["completion", "--help"]))
                .unwrap()
                .contains("Choices for `shell`:")
        );
        assert!(run(&strings(&["mcp", "--help"])).unwrap().contains("serve"));
    }

    #[test]
    fn help_is_available_for_every_schema_command_path() {
        fn check_paths(commands: &[CommandSpec], prefix: &[&str]) {
            for command in commands {
                let mut path = prefix.to_vec();
                path.push(command.name);
                let mut expected_usage = format!("Usage: neoget {}", path.join(" "));
                for argument in command.arguments {
                    expected_usage.push_str(&if argument.min == 0 {
                        format!(" [{}]", argument.name)
                    } else {
                        format!(" <{}>", argument.name)
                    });
                }
                expected_usage.push_str(" [--help]");
                for flag in ["--help", "-h"] {
                    let mut args = strings(&path);
                    args.push(flag.into());
                    let rendered = run(&args)
                        .unwrap_or_else(|error| panic!("{} {flag}: {error}", path.join(" ")));
                    assert!(
                        rendered.contains(&expected_usage),
                        "missing usage for {}",
                        path.join(" ")
                    );
                    assert!(rendered.contains(command.summary));
                    args.push("extra".into());
                    assert!(run(&args).unwrap_err().contains("after help"));
                }
                check_paths(command.children, &path);
            }
        }
        check_paths(COMMANDS, &[]);
    }

    #[test]
    fn completion_candidates_follow_argument_position_and_shell_support() {
        assert_eq!(candidates_for(&[]), top_candidates());
        assert_eq!(candidates_for(&["plan"]), ACTIONS);
        assert_eq!(candidates_for(&["completion"]), SHELLS);
        assert_eq!(candidates_for(&["mcp"]), vec!["serve"]);
        assert!(candidates_for(&["install"]).is_empty());
        assert!(completion("ruby").is_err());
        for shell in SHELLS {
            let text = run(&strings(&["completion", shell])).unwrap();
            assert!(text.contains("plan") && text.contains("completion") && text.contains("serve"));
        }
        assert!(completion("nushell").unwrap().contains("extern neoget ["));
    }

    #[test]
    fn generated_reference_and_help_cover_nested_schema() {
        let text = reference();
        assert!(text.contains("## `mcp serve`"));
        assert!(text.contains(&format!("Choices for `shell`: {}.", SHELLS.join(", "))));
        assert!(
            run(&strings(&["completion", "--help"]))
                .unwrap()
                .contains(&format!("Choices for `shell`: {}.", SHELLS.join(", ")))
        );
        assert!(command_help(&[]).contains("mcp"));
        assert_eq!(text, include_str!("../../../docs/commands.md"));
    }

    #[test]
    fn shell_artifacts_bind_native_and_position_specific_completers() {
        let powershell = run(&strings(&["completion", "powershell"])).unwrap();
        assert!(powershell.contains("Register-ArgumentCompleter -Native -CommandName neoget"));

        let nushell = run(&strings(&["completion", "nushell"])).unwrap();
        assert!(nushell.contains("extern neoget [command?: string@'nu-complete neoget command', ...args: string, --driver(-D):"));
        assert!(nushell.contains("extern \"neoget plan\" [action?: string@'nu-complete neoget plan action', target?: string, --driver(-D):"));
        assert!(nushell.contains("extern \"neoget completion\" [shell?: string@'nu-complete neoget completion shell', --driver(-D):"));
        assert!(nushell.contains("extern \"neoget mcp\" [subcommand?: string@'nu-complete neoget mcp subcommand', --driver(-D):"));
        assert!(nushell.contains("extern \"neoget i\" [target?: string, --driver(-D):"));
        for shell in SHELLS {
            let artifact = completion(shell).unwrap();
            for option in GLOBAL_OPTIONS {
                assert!(
                    artifact.contains(option.long.trim_start_matches('-')),
                    "{shell} missing {}",
                    option.long
                );
                for driver in option.choices {
                    assert!(artifact.contains(driver), "{shell} missing driver choice");
                }
            }
        }
        assert!(!nushell.contains("^neoget"));
        assert!(
            nushell
                .lines()
                .filter(|line| line.starts_with("def "))
                .all(|line| line.starts_with("def \"nu-complete "))
        );

        fn check_externs(text: &str, commands: &[CommandSpec], prefix: &str) {
            for command in commands {
                let name = format!("{prefix} {}", command.name);
                let declaration = text
                    .lines()
                    .find(|line| line.starts_with(&format!("extern \"{name}\" [")))
                    .expect("schema command has an extern");
                assert!(
                    declaration.ends_with("--help(-h)]"),
                    "{name} must forward native help"
                );
                for argument in command.arguments {
                    let parameter = format!("{}?: string", argument.name);
                    assert!(declaration.contains(&parameter));
                    if !argument.choices.is_empty() {
                        let completer_name = format!("nu-complete {name} {}", argument.name);
                        assert!(declaration.contains(&format!("@'{completer_name}'")));
                        let definition = text
                            .lines()
                            .find(|line| line.starts_with(&format!("def \"{completer_name}\" []")))
                            .expect("attached completer is defined");
                        for choice in argument.choices {
                            assert!(definition.contains(&format!("'{choice}'")));
                        }
                    }
                }
                for option in GLOBAL_OPTIONS {
                    assert!(
                        declaration.contains(option.long),
                        "{name} missing {}",
                        option.long
                    );
                }
                check_externs(text, command.children, &name);
            }
        }
        check_externs(&nushell, COMMANDS, "neoget");
    }

    #[test]
    fn unsafe_forwarding_and_invalid_extra_arguments_are_rejected() {
        assert!(
            run(&strings(&["install", "pkg", "--", "--force"]))
                .unwrap_err()
                .contains("not supported")
        );
        assert!(
            run(&strings(&["completion", "--"]))
                .unwrap_err()
                .contains("not supported")
        );
        assert!(
            run(&strings(&["refresh", "now"]))
                .unwrap_err()
                .contains("unexpected argument")
        );
    }
}
