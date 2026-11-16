//! Provider command adapters and a bounded, streamed process boundary.
mod process;
pub use process::{
    ExecutionContext, Invocation, OutputPolicy, ProcessEvent, ProcessRunner, SystemRunner,
};
mod apt;
mod aqua;
mod chocolatey;
mod dnf;
mod github;
mod pacman;
mod winget;
mod yum;
pub use apt::Apt;
pub use aqua::Aqua;
pub use chocolatey::Chocolatey;
pub use dnf::Dnf;
pub use github::Github;
pub use pacman::Pacman;
pub use winget::Winget;
pub use yum::Yum;
/// Host platform used for deterministic applicability checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Host {
    Windows,
    Linux,
    Macos,
}
/// Operations for which an adapter can construct a command, not authorize execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    Inventory,
    UpgradeAll,
}
/// A provider-specific command adapter. Commands are argv plans only and do not imply success.
pub trait Adapter {
    fn id(&self) -> &'static str;
    fn applies(&self, host: Host) -> bool;
    fn executable(&self) -> &'static str;
    /// Checks whether the provider executable is discoverable through the supplied probe.
    fn executable_available(&self, probe: &dyn ExecutableProbe) -> bool {
        probe.available(self.executable())
    }
    fn command(&self, operation: Operation, package: Option<&str>) -> Result<Invocation, String>;
    /// Executes only a constructed provider command through the explicit streamed process context.
    /// A zero exit code reports process completion, not verified package lifecycle success.
    fn execute(
        &self,
        execution: &mut ExecutionContext<'_>,
        operation: Operation,
        package: Option<&str>,
    ) -> Result<i32, String> {
        let invocation = self.command(operation, package)?;
        execution.run(&invocation)
    }
}
/// Injectable executable lookup, allowing detection without consulting the host in tests.
pub trait ExecutableProbe {
    /// Returns true when an executable name can be resolved.
    fn available(&self, executable: &str) -> bool;
}
/// Host PATH executable lookup used by production discovery.
pub struct PathExecutableProbe;
impl ExecutableProbe for PathExecutableProbe {
    fn available(&self, executable: &str) -> bool {
        std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths).any(|directory| {
                let candidate = directory.join(executable);
                candidate.is_file()
                    || cfg!(windows)
                        && ["exe", "cmd", "bat"]
                            .iter()
                            .any(|extension| candidate.with_extension(extension).is_file())
            })
        })
    }
}
/// Registered adapters, one distinct concrete type per provider, in stable order.
pub fn registry() -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(Winget),
        Box::new(Chocolatey),
        Box::new(Apt),
        Box::new(Pacman),
        Box::new(Dnf),
        Box::new(Yum),
        Box::new(Github),
        Box::new(Aqua),
    ]
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry_contains_distinct_provider_types_and_expected_operations() {
        let ps = registry();
        assert_eq!(
            ps.iter().map(|p| p.id()).collect::<Vec<_>>(),
            [
                "winget",
                "chocolatey",
                "apt",
                "pacman",
                "dnf",
                "yum",
                "github",
                "aqua"
            ]
        );
        let commands = [
            ps[0].command(Operation::Inventory, None).unwrap(),
            ps[1].command(Operation::Inventory, None).unwrap(),
            ps[2].command(Operation::Inventory, None).unwrap(),
            ps[3].command(Operation::Inventory, None).unwrap(),
            ps[4].command(Operation::Inventory, None).unwrap(),
            ps[5].command(Operation::Inventory, None).unwrap(),
            ps[6].command(Operation::Inventory, None).unwrap(),
            ps[7].command(Operation::Inventory, None).unwrap(),
        ];
        assert_eq!(commands[0].args, ["list"]);
        assert_eq!(commands[1].args, ["list", "--local-only"]);
        assert_eq!(commands[2].program, "dpkg-query");
        assert_eq!(commands[3].args, ["-Q"]);
        assert_eq!(commands[4].args, ["list", "--installed"]);
        assert_eq!(commands[5].args, ["list", "installed"]);
        assert_eq!(commands[6].args, ["extension", "list"]);
        assert_eq!(commands[7].args, ["list"]);
        let pacman_upgrade = ps[3].command(Operation::UpgradeAll, None).unwrap();
        assert_eq!(pacman_upgrade.args, ["-Syu"]);
        assert!(
            commands
                .iter()
                .all(|c| !c.args.iter().any(|a| a.contains(';')))
        );
    }
    #[test]
    fn applicability_and_unsupported_operations_are_explicit() {
        assert!(Apt.applies(Host::Linux));
        assert!(!Apt.applies(Host::Windows));
        assert!(Chocolatey.applies(Host::Windows));
        assert!(Dnf.applies(Host::Linux));
        assert!(Yum.applies(Host::Linux));
        assert!(Github.applies(Host::Windows));
        assert!(Aqua.applies(Host::Macos));
        assert!(
            Pacman
                .command(Operation::UpgradeAll, Some("x"))
                .unwrap_err()
                .contains("targeted")
        );
        for a in [
            Box::new(Winget) as Box<dyn Adapter>,
            Box::new(Chocolatey),
            Box::new(Apt),
            Box::new(Dnf),
            Box::new(Yum),
            Box::new(Github),
            Box::new(Aqua),
        ] {
            assert!(a.command(Operation::UpgradeAll, None).is_err());
        }
    }
    #[test]
    fn executable_detection_uses_an_injected_probe() {
        struct Probe;
        impl ExecutableProbe for Probe {
            fn available(&self, executable: &str) -> bool {
                executable == "pacman"
            }
        }
        assert!(Pacman.executable_available(&Probe));
        assert!(!Apt.executable_available(&Probe));
        assert!(Winget.applies(Host::Windows));
        assert!(!Winget.applies(Host::Linux));
        assert!(Github.applies(Host::Macos));
        assert!(Aqua.applies(Host::Linux));
    }
}
