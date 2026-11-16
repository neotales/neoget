use neoget_config::{BinRootOverrides, BinRootPlatform, InstallScope, resolve_bin_root};
use std::path::Path;

#[test]
fn bin_root_precedence_is_explicit_environment_scope_config_then_platform() {
    let cwd = std::env::temp_dir();
    let explicit = Path::new("argument");
    let environment = Path::new("environment");
    let user = Path::new("user-config");
    let global = Path::new("global-config");
    for scope in [InstallScope::User, InstallScope::Global] {
        let platform = || BinRootPlatform::Unix { home: None };
        let overrides = || BinRootOverrides {
            explicit: Some(explicit),
            environment: Some(environment),
            user: Some(user),
            global: Some(global),
        };
        assert_eq!(
            resolve_bin_root(scope, overrides(), platform(), &cwd)
                .unwrap()
                .path(),
            cwd.join(explicit)
        );
        assert_eq!(
            resolve_bin_root(
                scope,
                BinRootOverrides {
                    explicit: None,
                    ..overrides()
                },
                platform(),
                &cwd
            )
            .unwrap()
            .path(),
            cwd.join(environment)
        );
        assert_eq!(
            resolve_bin_root(
                scope,
                BinRootOverrides {
                    explicit: None,
                    environment: None,
                    ..overrides()
                },
                platform(),
                &cwd
            )
            .unwrap()
            .path(),
            cwd.join(match scope {
                InstallScope::User => user,
                InstallScope::Global => global,
            })
        );
        assert!(
            resolve_bin_root(
                scope,
                BinRootOverrides {
                    explicit: Some(Path::new("")),
                    ..overrides()
                },
                platform(),
                &cwd
            )
            .is_err()
        );
        assert!(
            resolve_bin_root(
                scope,
                BinRootOverrides {
                    explicit: Some(Path::new("bad\npath")),
                    ..overrides()
                },
                platform(),
                &cwd
            )
            .is_err()
        );
    }
}

#[test]
fn user_and_global_platform_defaults_are_injected_and_missing_bases_fail() {
    let cwd = std::env::temp_dir();
    let home = cwd.join("home");
    let local = cwd.join("local");
    let system = cwd.join("system");
    for (scope, expected) in [
        (InstallScope::User, local.join("Neoget/bin")),
        (InstallScope::Global, system.join("Neoget/bin")),
    ] {
        assert_eq!(
            resolve_bin_root(
                scope,
                Default::default(),
                BinRootPlatform::Windows {
                    local_app_data: Some(&local),
                    program_data: Some(&system)
                },
                &cwd
            )
            .unwrap()
            .path(),
            expected
        );
        assert!(
            resolve_bin_root(
                scope,
                Default::default(),
                BinRootPlatform::Windows {
                    local_app_data: None,
                    program_data: None
                },
                &cwd
            )
            .is_err()
        );
    }
    assert_eq!(
        resolve_bin_root(
            InstallScope::User,
            Default::default(),
            BinRootPlatform::Unix { home: Some(&home) },
            &cwd
        )
        .unwrap()
        .path(),
        home.join(".local/bin")
    );
    assert!(
        resolve_bin_root(
            InstallScope::User,
            Default::default(),
            BinRootPlatform::Unix { home: None },
            &cwd
        )
        .is_err()
    );
    assert!(
        resolve_bin_root(
            InstallScope::User,
            Default::default(),
            BinRootPlatform::Unix {
                home: Some(Path::new(""))
            },
            &cwd
        )
        .is_err()
    );
    assert!(
        resolve_bin_root(
            InstallScope::Global,
            Default::default(),
            BinRootPlatform::Unix { home: None },
            Path::new("relative")
        )
        .is_err()
    );
    #[cfg(unix)]
    assert_eq!(
        resolve_bin_root(
            InstallScope::Global,
            Default::default(),
            BinRootPlatform::Unix { home: None },
            &cwd
        )
        .unwrap()
        .path(),
        Path::new("/usr/local/bin")
    );
}
