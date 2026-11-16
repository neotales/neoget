use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "neoget-cli-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_neoget"))
            .args(args)
            .current_dir(&self.0)
            .env("NEOGET_ROOT", self.0.join("environment"))
            .env("XDG_DATA_HOME", self.0.join("default"))
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn error(output: Output) -> String {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    String::from_utf8(output.stderr).unwrap()
}

#[test]
fn config_checks_are_real_strict_format_explicit_and_do_not_create_state() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("bundle.toml"), "version = 1\n").unwrap();
    let output = fixture.run(&["config", "check", "bundle.toml"]);
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("no operations executed")
    );
    fs::write(fixture.0.join("bundle.yaml"), "version: 1\nunknown: true").unwrap();
    assert!(error(fixture.run(&["config", "check", "bundle.yaml"])).contains("CONFIG_SYNTAX"));
    fs::write(fixture.0.join("tools.yaml"), "tools: {}\n").unwrap();
    assert!(
        fixture
            .run(&["config", "check-neoenv", "tools.yaml"])
            .status
            .success()
    );
    assert!(error(fixture.run(&["config", "check", "tools.yaml"])).contains("CONFIG_SYNTAX"));
    assert!(error(fixture.run(&["config", "check", "missing.toml"])).contains("CONFIG_IO"));
    assert!(error(fixture.run(&["config", "check", "bundle.json"])).contains("CONFIG_FORMAT"));
    assert!(!fixture.0.join("environment").exists());
    assert!(!fixture.0.join("default").exists());
}

#[test]
fn roots_use_explicit_over_environment_and_ambiguous_configs_never_fall_back() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.0.join("environment")).unwrap();
    fs::write(fixture.0.join("environment/neoget.toml"), "typo = 1").unwrap();
    assert!(error(fixture.run(&["list"])).contains("CONFIG_SYNTAX"));
    assert!(error(fixture.run(&["-D", "explicit", "list"])).contains("CAPABILITY_UNAVAILABLE"));
    assert!(error(fixture.run(&["list", "-Dexplicit"])).contains("CAPABILITY_UNAVAILABLE"));
    assert!(error(fixture.run(&["list", "--root=explicit"])).contains("CAPABILITY_UNAVAILABLE"));
    assert!(!fixture.0.join("explicit").exists());
    fs::write(fixture.0.join("selected.toml"), "version = 1").unwrap();
    assert!(
        error(fixture.run(&["list", "--config=selected.toml"])).contains("CAPABILITY_UNAVAILABLE")
    );
    fs::write(fixture.0.join("environment/neoget.yaml"), "version: 1").unwrap();
    assert!(error(fixture.run(&["list"])).contains("CONFIG_AMBIGUOUS"));
    // Presentation never opens even an explicit missing or malformed config.
    assert!(
        fixture
            .run(&["list", "--help", "--config", "missing.toml"])
            .status
            .success()
    );
    assert!(
        fixture
            .run(&["completion", "bash", "--config", "missing.toml"])
            .status
            .success()
    );
}

#[test]
fn aliases_options_sources_pins_and_unsupported_features_have_truthful_errors() {
    let fixture = Fixture::new();
    for args in [
        vec!["i", "github:owner/repo"],
        vec!["rm", "pkg"],
        vec!["ls"],
        vec!["sources", "list"],
        vec!["sources", "add", "test", "https://example.org"],
        vec!["sources", "remove", "test"],
        vec!["sources", "refresh"],
        vec!["pins", "list"],
        vec!["pins", "add", "pkg", "1.0"],
        vec!["pins", "remove", "pkg"],
        vec!["apply", "plan"],
        vec!["continue", "operation"],
        vec!["tui"],
        vec!["mcp", "serve"],
    ] {
        assert!(
            error(fixture.run(&args)).contains("CAPABILITY_UNAVAILABLE"),
            "{args:?}"
        );
    }
    for args in [
        vec!["-D"],
        vec!["-D", "--help"],
        vec!["--root="],
        vec!["-Dx", "list", "--root=y"],
        vec!["--config=x", "--config=y", "list"],
        vec!["sources", "add", "only-name"],
        vec!["pins", "add", "only-name"],
        vec!["sources", "unknown"],
        vec!["pins", "list", "extra"],
        vec!["i", "pkg", "--", "--force"],
    ] {
        assert!(
            !error(fixture.run(&args)).contains("CAPABILITY_UNAVAILABLE"),
            "{args:?}"
        );
    }
    for (alias, canonical) in [
        ("i", "install"),
        ("ls", "list"),
        ("up", "upgrade"),
        ("rm", "uninstall"),
    ] {
        let output = fixture.run(&[alias, "--help"]);
        assert!(output.status.success());
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains(&format!("Usage: neoget {canonical}"))
        );
    }
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
}
