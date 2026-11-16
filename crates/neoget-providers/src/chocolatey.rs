//! chocolatey adapter: command construction only; execution is not authorized here.
use crate::{Adapter, Host, Invocation, Operation};
/// Stable chocolatey provider adapter.
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
        let args = match operation {
            Operation::Inventory => vec!["list", "--local-only"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            Operation::UpgradeAll => return Err("unsupported operation: choco upgrade all".into()),
        };
        Ok(Invocation {
            program: "choco".into(),
            args,
        })
    }
}
