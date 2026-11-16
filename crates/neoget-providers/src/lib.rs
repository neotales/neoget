//! Provider command adapters and a bounded, streamed process boundary.
//!
//! # Use the streamed process context
//!
//! The context owns neither runner nor sink; the caller controls both and may clone its
//! cancellation token before starting `run`. An error can follow already delivered output.
//!
//! ```rust
//! use neoget_providers::{
//!     CancellationToken, ExecutionContext, Invocation, ProcessEvent, ProcessRunner,
//!     ProcessError,
//! };
//! struct PlannedRunner;
//! impl ProcessRunner for PlannedRunner {
//!     fn run(
//!         &self,
//!         invocation: &Invocation,
//!         _cancellation: &CancellationToken,
//!         sink: &mut dyn FnMut(ProcessEvent),
//!     ) -> Result<i32, ProcessError> {
//!         assert_eq!(invocation.args, ["--version"]);
//!         sink(ProcessEvent::Stdout("version output".into()));
//!         Ok(0)
//!     }
//! }
//! let mut events = Vec::new();
//! let runner = PlannedRunner;
//! let mut sink = |event| events.push(event);
//! let mut context = ExecutionContext::streaming(&runner, &mut sink);
//! let invocation = Invocation { program: "tool".into(), args: vec!["--version".into()] };
//! assert_eq!(context.run(&invocation)?, 0);
//! assert_eq!(events, [ProcessEvent::Stdout("version output".into())]);
//! # Ok::<(), ProcessError>(())
//! ```
mod process;
pub use process::{
    CancellationToken, ExecutionContext, Invocation, ProcessError, ProcessErrorKind, ProcessEvent,
    ProcessRunner, SystemRunner,
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
    /// Checks all executable prerequisites for the specific operation.
    fn executable_available(
        &self,
        operation: Operation,
        probe: &dyn ExecutableProbe,
    ) -> Result<bool, String> {
        self.command(operation, None)
            .map(|invocation| probe.available(&invocation.program))
    }
    /// Constructs an argv vector. `package=None` means provider-wide inventory/upgrade;
    /// `Some(name)` is a native package target. Inventory filters are unsupported and must
    /// be rejected; adapters without targeted upgrades reject `Some(name)` explicitly.
    fn command(&self, operation: Operation, package: Option<&str>) -> Result<Invocation, String>;
    /// Executes only a constructed provider command through the explicit streamed process context.
    /// A zero exit code reports process completion, not verified package lifecycle success.
    fn execute(
        &self,
        execution: &mut ExecutionContext<'_>,
        operation: Operation,
        package: Option<&str>,
    ) -> Result<i32, ProcessError> {
        let invocation = self
            .command(operation, package)
            .map_err(|_| ProcessError::adapter())?;
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
                let direct = executable_file(&candidate);
                #[cfg(windows)]
                {
                    direct
                        || (candidate.extension().is_none()
                            && ["exe", "cmd", "bat"].iter().any(|extension| {
                                executable_file(&candidate.with_extension(extension))
                            }))
                }
                #[cfg(not(windows))]
                direct
            })
        })
    }
}
#[cfg(unix)]
fn executable_file(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}
#[cfg(windows)]
fn executable_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| {
            ["exe", "com", "cmd", "bat"]
                .iter()
                .any(|allowed| extension.eq_ignore_ascii_case(allowed))
        })
        && path.is_file()
}
#[cfg(not(any(unix, windows)))]
fn executable_file(path: &std::path::Path) -> bool {
    path.is_file()
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
        assert_eq!(commands[1].args, ["list"]);
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
            Chocolatey
                .command(Operation::Inventory, Some("ignored"))
                .unwrap_err()
                .contains("does not accept")
        );
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
                executable == "pacman" || executable == "dpkg-query"
            }
        }
        assert!(
            Pacman
                .executable_available(Operation::Inventory, &Probe)
                .unwrap()
        );
        assert!(
            Apt.executable_available(Operation::Inventory, &Probe)
                .unwrap()
        );
        assert!(
            Apt.executable_available(Operation::UpgradeAll, &Probe)
                .unwrap_err()
                .contains("unsupported")
        );
        assert!(Winget.applies(Host::Windows));
        assert!(!Winget.applies(Host::Linux));
        assert!(Github.applies(Host::Macos));
        assert!(Aqua.applies(Host::Linux));
    }
}
