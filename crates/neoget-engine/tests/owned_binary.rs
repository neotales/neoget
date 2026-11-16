use neoget_config::{ArchiveEntry, Format, parse_bundle};
use neoget_engine::plan_owned_binary;

#[test]
fn alias_asset_member_and_rename_are_resolved_without_publishing() {
    let config = r#"
version = 1
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
    let plan = plan_owned_binary(
        &bundle,
        "find-files",
        &assets,
        &members,
        "10.0",
        "linux",
        "x86_64",
    )
    .unwrap();
    assert_eq!(plan.executable, "fd-owned");
    assert_eq!(plan.member.as_deref(), Some("fd/bin/fd"));
    assert_eq!(plan.tool, "fd");
    assert!(
        plan_owned_binary(&bundle, "fd", &assets, &members, "11.0", "linux", "x86_64").is_err()
    );
    assert!(plan_owned_binary(&bundle, "fd", &assets, &[], "10.0", "linux", "x86_64").is_err());
}
