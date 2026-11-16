use neoget_config::{Format, parse_neoenv_tools};

#[test]
fn authoritative_native_neoenv_tool_fixtures_parse_in_both_formats() {
    let toml = include_str!("fixtures/neoenv-tools.toml");
    let yaml = include_str!("fixtures/neoenv-tools.yaml");
    let tools = parse_neoenv_tools(toml, Format::Toml).unwrap();
    assert_eq!(tools, parse_neoenv_tools(yaml, Format::Yaml).unwrap());
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.id.as_str())
            .collect::<Vec<_>>(),
        ["first", "demo", "aqua-demo"]
    );
    assert_eq!(tools[0].provider, None);
    assert_eq!(tools[1].provider.as_deref(), Some("github-release"));
    assert_eq!(
        tools[1].github_release.as_ref().unwrap().repository,
        "owner/demo"
    );
    assert_eq!(tools[1].platforms[0].arch.as_deref(), Some("amd64"));
    assert_eq!(
        tools[2].aqua.as_ref().unwrap().registry_revision,
        "a".repeat(40)
    );
}

#[test]
fn native_schema_rejects_synthetic_bundle_shape_typos_duplicates_and_other_domains() {
    for (format, fixture) in [
        (Format::Toml, include_str!("fixtures/neoenv-tools.toml")),
        (Format::Yaml, include_str!("fixtures/neoenv-tools.yaml")),
    ] {
        for bad in [
            fixture.replace("format_version", "format_versoin"),
            fixture.replace("checksum_asset", "checksum_assset"),
            fixture.replace("registry_revision", "registry_revisoin"),
            fixture.replace("aqua-demo", "first"),
            fixture
                .replace("format_version = 1", "format_version = 2")
                .replace("format_version: 1", "format_version: 2"),
        ] {
            assert!(parse_neoenv_tools(&bad, format).is_err(), "{bad}");
        }
    }
    for input in [
        "tools: {fd: {provider: github, package: sharkdp/fd}}",
        "tools: []\ntasks: {}",
        "tools: []\nenv: {}",
        "tools: []\nimports: []",
        "format_version: 1\nformat_version: 1\ntools: []",
        "tools: [{id: demo, id: other, version: '1'}]",
        "tools: [{id: demo, version: null}]",
        "tools: [{id: demo, version: 1.0}]",
        "tools: [{id: demo, version: '1', platforms: [{binaries: [true]}]}]",
    ] {
        assert!(parse_neoenv_tools(input, Format::Yaml).is_err(), "{input}");
    }
}
