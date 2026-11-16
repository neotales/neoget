//! CLI validation and generated presentation from one static command schema.
use std::env;

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
    }
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
    output
}

fn top_candidates() -> Vec<&'static str> {
    COMMANDS.iter().map(|command| command.name).collect()
}

fn candidates_for(tokens: &[&str]) -> Vec<&'static str> {
    if tokens.is_empty() {
        return top_candidates();
    }
    let Some(command) = COMMANDS.iter().find(|command| command.name == tokens[0]) else {
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
    output.push_str("extern neoget [command?: string@'nu-complete neoget command', ...args: string, --help(-h), --reference]\n");
    append(&mut output, COMMANDS, "neoget");
    output
}

fn completion(shell: &str) -> Result<String, String> {
    if !SHELLS.contains(&shell) {
        return Err(format!(
            "unsupported shell '{shell}'; choose {}",
            SHELLS.join(", ")
        ));
    }
    let top = top_candidates().join(" ");
    let plan = candidates_for(&["plan"]).join(" ");
    let shells = candidates_for(&["completion"]).join(" ");
    let mcp = candidates_for(&["mcp"]).join(" ");
    let output = match shell {
        "bash" => format!(
            "_neoget() {{\n  local cur prev\n  cur=\"${{COMP_WORDS[COMP_CWORD]}}\"; prev=\"${{COMP_WORDS[COMP_CWORD-1]}}\"\n  case \"${{COMP_CWORD}}:${{COMP_WORDS[1]}}\" in\n    1:*) COMPREPLY=( $(compgen -W '{top}' -- \"$cur\") ) ;;\n    2:plan) COMPREPLY=( $(compgen -W '{plan}' -- \"$cur\") ) ;;\n    2:completion) COMPREPLY=( $(compgen -W '{shells}' -- \"$cur\") ) ;;\n    2:mcp) COMPREPLY=( $(compgen -W '{mcp}' -- \"$cur\") ) ;;\n    *) COMPREPLY=() ;;\n  esac\n}}\ncomplete -F _neoget neoget\n"
        ),
        "zsh" => format!(
            "#compdef neoget\n_arguments '1:command:({top})' '2:argument:->argument'\nif (( CURRENT == 3 )); then\n  case $words[2] in\n    plan) _values action {plan} ;;\n    completion) _values shell {shells} ;;\n    mcp) _values subcommand {mcp} ;;\n  esac\nfi\n"
        ),
        "fish" => {
            let mut text = String::from("complete -c neoget -f\n");
            for command in COMMANDS {
                text.push_str(&format!(
                    "complete -c neoget -n '__fish_use_subcommand' -a '{}' -d '{}'\n",
                    command.name, command.summary
                ));
            }
            for choice in ACTIONS {
                text.push_str(&format!("complete -c neoget -n '__fish_seen_subcommand_from plan; and test (count (commandline -opc)) -eq 2' -a '{}'\n", choice));
            }
            for choice in SHELLS {
                text.push_str(&format!("complete -c neoget -n '__fish_seen_subcommand_from completion; and test (count (commandline -opc)) -eq 2' -a '{}'\n", choice));
            }
            for child in MCP_CHILDREN {
                text.push_str(&format!("complete -c neoget -n '__fish_seen_subcommand_from mcp; and test (count (commandline -opc)) -eq 2' -a '{}'\n", child.name));
            }
            text
        }
        "powershell" => format!(
            "Register-ArgumentCompleter -Native -CommandName neoget -ScriptBlock {{ param($wordToComplete,$commandAst,$cursorPosition) $words=$commandAst.ToString().Split(' '); $candidates=if ($words.Count -le 2) {{ '{top}'.Split(' ') }} elseif ($words[1] -eq 'plan' -and $words.Count -le 3) {{ '{plan}'.Split(' ') }} elseif ($words[1] -eq 'completion' -and $words.Count -le 3) {{ '{shells}'.Split(' ') }} elseif ($words[1] -eq 'mcp' -and $words.Count -le 3) {{ '{mcp}'.Split(' ') }} else {{ @() }}; $candidates | Where-Object {{ $_ -like \"$wordToComplete*\" }} }}\n"
        ),
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
    Ok(Invocation::Execute(path, cursor))
}

fn run(args: &[String]) -> Result<String, String> {
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
    if command.availability == Availability::Planned {
        return Err(format!(
            "'{}' is documented but not implemented in v0",
            command.name
        ));
    }
    Err(format!(
        "'{}' is specified but its operation is not implemented in v0",
        command.name
    ))
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
        assert!(nushell.contains("extern neoget [command?: string@'nu-complete neoget command', ...args: string, --help(-h), --reference]"));
        assert!(nushell.contains("extern \"neoget plan\" [action?: string@'nu-complete neoget plan action', target?: string, --help(-h)]"));
        assert!(nushell.contains("extern \"neoget completion\" [shell?: string@'nu-complete neoget completion shell', --help(-h)]"));
        assert!(nushell.contains("extern \"neoget mcp\" [subcommand?: string@'nu-complete neoget mcp subcommand', --help(-h)]"));
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
