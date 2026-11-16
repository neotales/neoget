use neoget_config::{ArchiveEntry, Format, parse_bundle, parse_neoenv_tools, select_root};
use std::path::Path;

const TOML: &str = r#"
version = 1
[aliases]
rg = "ripgrep"
[tools.ripgrep]
provider = "github"
package = "BurntSushi/ripgrep"
version = "14.1.0"
rename = "rg-owned"
aliases = ["grep-tool"]
[tools.ripgrep.asset]
template = "ripgrep-{version}-{arch}-{os}.tar.gz"
[tools.ripgrep.asset.member]
basename = "rg"
[pins]
ripgrep = "14.1.0"
[sources.internal]
provider = "aqua"
url = "https://example.org/catalog"
"#;

const YAML: &str = r#"
version: 1
aliases:
  rg: ripgrep
tools:
  ripgrep:
    provider: github
    package: BurntSushi/ripgrep
    version: 14.1.0
    rename: rg-owned
    aliases: [grep-tool]
    asset:
      template: ripgrep-{version}-{arch}-{os}.tar.gz
      member:
        basename: rg
pins:
  ripgrep: 14.1.0
sources:
  internal:
    provider: aqua
    url: https://example.org/catalog
"#;

#[test]
fn formats_share_strict_bundle_alias_pin_and_selection_contracts() {
    let bundle = parse_bundle(TOML, Format::Toml).unwrap();
    assert_eq!(bundle, parse_bundle(YAML, Format::Yaml).unwrap());
    for alias in ["rg", "ripgrep", "grep-tool"] {
        let (name, tool) = bundle.tool(alias).unwrap();
        assert_eq!(name, "ripgrep");
        let asset = tool.asset.as_ref().unwrap();
        let names = vec![
            "ripgrep-14.1.0-x86_64-linux.tar.gz".into(),
            "checksums".into(),
        ];
        assert_eq!(
            asset.select(&names, "14.1.0", "linux", "x86_64").unwrap(),
            names[0]
        );
        let entries = vec![ArchiveEntry {
            path: "ripgrep/rg".into(),
            regular: true,
        }];
        assert_eq!(
            asset
                .member
                .as_ref()
                .unwrap()
                .select(&entries)
                .unwrap()
                .path,
            "ripgrep/rg"
        );
    }
}

#[test]
fn strict_schema_rejects_unknown_duplicate_unsupported_and_conflicting_data() {
    for input in [
        TOML.replace("version = 1", "version = 2"),
        TOML.replace("version = 1", "version = 1\nunknown = true"),
        TOML.replace("rename =", "renmae ="),
        TOML.replace("template =", "grep ="),
        TOML.replace("basename =", "regex ="),
        TOML.replace("rg = \"ripgrep\"", "rg = \"other\""),
        TOML.replace("aliases = [\"grep-tool\"]", "aliases = [\"rg\"]"),
        TOML.replace("rg-owned", "../outside"),
        TOML.replace("rg-owned", "CON.exe"),
        TOML.replace("rg-owned", "unsafe."),
        TOML.replace("rg-owned", "-option"),
        TOML.replace("BurntSushi/ripgrep", "../repo"),
        TOML.replace("{arch}", "{unknown}"),
        TOML.replace("basename = \"rg\"", "basename = \"rg\"\nexact = \"rg\""),
        TOML.replace("template =", "exact = \"foo\"\ntemplate ="),
        TOML.replace("14.1.0\"\n[sources", "15.0.0\"\n[sources"),
        TOML.replace("provider = \"github\"", "provider = \"apt\""),
        TOML.replace("version = 1", "version = 1\nversion = 1"),
    ] {
        assert!(parse_bundle(&input, Format::Toml).is_err(), "{input}");
    }
    for input in [
        "version: 1\nversion: 1",
        "version: 1\nunknown: true",
        "version: 1\n---\nversion: 1",
        "version: 1\ntools: {x: {provider: github, package: a/b}}",
        "version: 1\ntools: {x: {provider: apt, package: x}, x: {provider: apt, package: y}}",
        "version: 1\naliases: {x: a, x: b}",
        "version: 1\ntools: {x: {provider: apt, package: x, version: null}}",
        "version: 1\ntools: {x: {provider: apt, package: x, version: 1.0}}",
        "version: 1\ntools: {x: {provider: true, package: x}}",
        "version: 1\naliases: {x: 1}",
        "version: 1\ntools: {1: {provider: apt, package: x}}",
        "version: 1\nsources: {x: {provider: aqua, url: true}}",
    ] {
        assert!(parse_bundle(input, Format::Yaml).is_err(), "{input}");
    }
}

#[test]
fn member_selection_is_bounded_literal_unique_and_rejects_unsafe_metadata() {
    let bundle = parse_bundle(TOML, Format::Toml).unwrap();
    let selection = bundle.tools["ripgrep"].asset.as_ref().unwrap();
    let filter = selection.member.as_ref().unwrap();
    for path in [
        "../rg", "/rg", "a/../rg", "C:/rg", "a\\rg", "a//rg", "a/./rg", "a/rg\n",
    ] {
        assert!(
            filter
                .select(&[ArchiveEntry {
                    path: path.into(),
                    regular: true
                }])
                .is_err()
        );
    }
    assert!(
        filter
            .select(&[ArchiveEntry {
                path: "rg".into(),
                regular: false
            }])
            .is_err()
    );
    assert!(filter.select(&[]).is_err());
    assert!(
        filter
            .select(&[
                ArchiveEntry {
                    path: "a/rg".into(),
                    regular: true
                },
                ArchiveEntry {
                    path: "b/rg".into(),
                    regular: true
                },
            ])
            .is_err()
    );
    let names = vec!["ripgrep-14.1.0-x86_64-linux.tar.gz".into(); 4097];
    assert!(
        selection
            .select(&names, "14.1.0", "linux", "x86_64")
            .is_err()
    );
    assert!(
        selection
            .select(&names[..2], "14.1.0", "linux", "x86_64")
            .is_err()
    );
    assert!(selection.select(&[], "14.1.0", "linux", "x86_64").is_err());
    assert!(selection.select(&[], "../v", "linux", "x86_64").is_err());
    assert!(parse_bundle(&" ".repeat(1024 * 1024 + 1), Format::Yaml).is_err());
}

#[test]
fn tool_only_interchange_never_swallows_other_neoenv_domains() {
    let input = r#"[[tools]]
id = "fd"
version = "v10.0"
provider = "github-release"
[tools.github_release]
repository = "sharkdp/fd"
asset = "fd.tar.gz"
"#;
    assert_eq!(parse_neoenv_tools(input, Format::Toml).unwrap()[0].id, "fd");
    for suffix in [
        "\n[env]\nFOO = 'bar'",
        "\n[tasks]\nx = 'echo hi'",
        "\n[hooks]\nx = 'echo hi'",
    ] {
        assert!(parse_neoenv_tools(&format!("{input}{suffix}"), Format::Toml).is_err());
    }
    assert!(parse_neoenv_tools("tools: {}\nremote: {}", Format::Yaml).is_err());
    assert!(parse_neoenv_tools("tools: []", Format::Yaml).is_ok());
    assert!(parse_neoenv_tools("tools: {}", Format::Yaml).is_err());
}

#[test]
fn root_resolution_is_pure_and_has_one_precedence_order() {
    let cwd = Path::new("workspace");
    let default = Path::new("default");
    let environment = Path::new("environment");
    let explicit = Path::new("explicit");
    assert_eq!(
        select_root(Some(explicit), Some(environment), default, cwd).unwrap(),
        cwd.join(explicit)
    );
    assert_eq!(
        select_root(None, Some(environment), default, cwd).unwrap(),
        cwd.join(environment)
    );
    assert_eq!(
        select_root(None, None, default, cwd).unwrap(),
        cwd.join(default)
    );
    assert!(select_root(Some(Path::new("")), None, default, cwd).is_err());
}

#[test]
fn published_config_examples_are_checked_against_their_actual_schema() {
    let document = include_str!("../../../docs/config.md");
    let mut checked = 0;
    for (label, format) in [("toml", Format::Toml), ("yaml", Format::Yaml)] {
        for block in document.split(&format!("```{label}\n")).skip(1) {
            let input = block.split_once("```").unwrap().0;
            if input.contains("format_version") {
                parse_neoenv_tools(input, format).unwrap();
            } else {
                parse_bundle(input, format).unwrap();
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 4);
}
