use neoget_config::{
    ArchiveEntry, BinRootOverrides, BinRootPlatform, Format, InstallScope, parse_bundle,
    resolve_bin_root,
};
use neoget_engine::{OwnedBinaryRelease, plan_owned_binary};

#[test]
fn alias_asset_member_rename_and_resolved_destination_are_planned_without_publishing() {
    let config = r#"
version = 1
[bin]
user = "user-bin"
global = "global-bin"
[tools.fd]
provider = "github"
package = "sharkdp/fd"
aliases = ["find-files"]
rename = "fd-owned"
version = "10.0"
[tools.fd.asset]
exact = "fd.tar.gz"
[tools.fd.asset.member]
exact = "fd/bin/fd"
"#;
    let bundle = parse_bundle(config, Format::Toml).unwrap();
    let assets = vec!["fd.tar.gz".into()];
    let members = vec![ArchiveEntry {
        path: "fd/bin/fd".into(),
        regular: true,
    }];
    let cwd = std::env::temp_dir();
    for (scope, directory) in [
        (InstallScope::User, "user-bin"),
        (InstallScope::Global, "global-bin"),
    ] {
        let root = resolve_bin_root(
            scope,
            BinRootOverrides {
                user: bundle.bin.user.as_deref().map(std::path::Path::new),
                global: bundle.bin.global.as_deref().map(std::path::Path::new),
                ..Default::default()
            },
            BinRootPlatform::Unix { home: None },
            &cwd,
        )
        .unwrap();
        let release = || OwnedBinaryRelease {
            assets: &assets,
            members: &members,
            version: "10.0",
            os: "linux",
            arch: "x86_64",
        };
        let plan = plan_owned_binary(&bundle, "find-files", release(), &root).unwrap();
        assert_eq!(plan.executable, "fd-owned");
        assert_eq!(plan.destination, cwd.join(directory).join("fd-owned"));
        assert_eq!(plan.member.as_deref(), Some("fd/bin/fd"));
        assert_eq!(plan.tool, "fd");
        assert!(
            plan_owned_binary(
                &bundle,
                "fd",
                OwnedBinaryRelease {
                    version: "11.0",
                    ..release()
                },
                &root
            )
            .is_err()
        );
        assert!(
            plan_owned_binary(
                &bundle,
                "fd",
                OwnedBinaryRelease {
                    members: &[],
                    ..release()
                },
                &root
            )
            .is_err()
        );
        let mut default_name = bundle.clone();
        default_name.tools.get_mut("fd").unwrap().rename = None;
        assert_eq!(
            plan_owned_binary(&default_name, "fd", release(), &root)
                .unwrap()
                .destination,
            cwd.join(directory).join("fd")
        );
        default_name.tools.get_mut("fd").unwrap().rename = Some("../escape".into());
        assert!(plan_owned_binary(&default_name, "fd", release(), &root).is_err());
    }
}
