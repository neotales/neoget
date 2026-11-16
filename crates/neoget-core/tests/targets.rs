use neoget_core::PackageTarget;

#[test]
fn targets_preserve_scopes_and_exact_versions_without_resolving_providers() {
    let target = PackageTarget::parse("aqua:@scope/name@1.2+build").unwrap();
    assert_eq!(target.provider.unwrap().as_str(), "aqua");
    assert_eq!(target.name, "@scope/name");
    assert_eq!(target.exact_version.as_deref(), Some("1.2+build"));
    assert_eq!(
        PackageTarget::parse("@scope/name").unwrap().exact_version,
        None
    );
    assert_eq!(PackageTarget::parse("owner/repo").unwrap().provider, None);
    for input in [
        "",
        "-force",
        "pkg@",
        ":pkg",
        "gh:",
        "pkg\n",
        "@",
        "@scope",
        "@/name",
        "@scope/",
        "pkg@@1",
        "gh:repo:other",
        "gh:-pkg",
    ] {
        assert!(PackageTarget::parse(input).is_err(), "{input}");
    }
}
