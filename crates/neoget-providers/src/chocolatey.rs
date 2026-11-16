//! chocolatey adapter: command construction only; execution is not authorized here.
//! Inventory assumes Chocolatey CLI v2+, where `choco list` defaults to local packages.
use crate::{Adapter, Host, Invocation, Operation};
/// Chocolatey command adapter.
///
/// Inventory assumes Chocolatey CLI v2 or later, where `choco list` lists local
/// packages by default and the removed `--local-only` switch is invalid.
pub struct Chocolatey;
impl Adapter for Chocolatey {
    fn id(&self) -> &'static str {
        "chocolatey"
    }
    fn applies(&self, host: Host) -> bool {
        matches!(host, Host::Windows)
    }
    fn executable(&self) -> &'static str {
        "choco"
    }
    fn command(&self, operation: Operation, _package: Option<&str>) -> Result<Invocation, String> {
        if operation == Operation::Inventory && _package.is_some() {
            return Err("provider inventory does not accept a package filter".into());
        }
        let args = match operation {
            Operation::Inventory => vec!["list".to_owned()],
            Operation::UpgradeAll => return Err("unsupported operation: choco upgrade all".into()),
        };
        Ok(Invocation {
            program: "choco".into(),
            args,
        })
    }
}
