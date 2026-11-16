use neoget_config::{Format, HostPlatform, parse_manifest};

#[test]
fn selects_os_specific_dependencies_and_normalizes_deterministically() {
    let text = r#"
version = 1
roots = ["app"]
[packages.app]
provider = "aqua"
package = "example"
dependencies = ["linux-lib", "mac-lib"]
[packages."linux-lib"]
provider = "apt"
package = "libexample"
version = "1.2"
platforms = ["linux"]
[packages."mac-lib"]
provider = "homebrew"
package = "libexample"
platforms = ["macos"]
"#;
    let manifest = parse_manifest(text, Format::Toml).unwrap();
    let linux = manifest.resolve(HostPlatform::Linux).unwrap();
    assert!(linux.packages.contains_key("linux-lib"));
    assert!(!linux.packages.contains_key("mac-lib"));
    assert_eq!(linux, manifest.resolve(HostPlatform::Linux).unwrap());
    let mac = manifest.resolve(HostPlatform::Macos).unwrap();
    assert!(mac.packages.contains_key("mac-lib"));
    assert!(!mac.packages.contains_key("linux-lib"));
}

#[test]
fn unsupported_hosts_cycles_and_identity_conflicts_are_rejected() {
    let unsupported = r#"version=1
roots=["app"]
[packages.app]
provider="apt"
package="app"
platforms=["linux"]
"#;
    assert!(
        parse_manifest(unsupported, Format::Toml)
            .unwrap()
            .resolve(HostPlatform::Windows)
            .is_err()
    );

    let cycle = r#"version=1
roots=["a"]
[packages.a]
provider="apt"
package="a"
dependencies=["b"]
[packages.b]
provider="apt"
package="b"
dependencies=["a"]
"#;
    assert!(
        parse_manifest(cycle, Format::Toml)
            .unwrap()
            .resolve(HostPlatform::Linux)
            .is_err()
    );

    let conflict = r#"version=1
roots=["a","b"]
[packages.a]
provider="apt"
package="same"
version="1"
[packages.b]
provider="apt"
package="same"
version="2"
"#;
    assert!(
        parse_manifest(conflict, Format::Toml)
            .unwrap()
            .resolve(HostPlatform::Linux)
            .is_err()
    );
}

#[test]
fn rejects_unknown_nested_keys_and_versions() {
    let unknown = r#"version=1
roots=["app"]
[packages.app]
provider="apt"
package="app"
script="run arbitrary code"
"#;
    assert!(parse_manifest(unknown, Format::Toml).is_err());
    assert!(
        parse_manifest(
            "version=2\nroots=[\"app\"]\n[packages.app]\nprovider=\"apt\"\npackage=\"app\"",
            Format::Toml
        )
        .is_err()
    );
}
