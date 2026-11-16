//! dnf adapter: command construction only; execution is not authorized here.
use crate::{Adapter, Host, Invocation, Operation};
/// Stable dnf provider adapter.
pub struct Dnf;
impl Adapter for Dnf {
    fn id(&self) -> &'static str {
        "dnf"
    }
    fn applies(&self, host: Host) -> bool {
        matches!(host, Host::Linux)
    }
    fn executable(&self) -> &'static str {
        "dnf"
    }
    fn command(&self, operation: Operation, _package: Option<&str>) -> Result<Invocation, String> {
        if operation == Operation::Inventory && _package.is_some() {
            return Err("provider inventory does not accept a package filter".into());
        }
        let args = match operation {
            Operation::Inventory => vec!["list", "--installed"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            Operation::UpgradeAll => return Err("unsupported operation: dnf upgrade".into()),
        };
        Ok(Invocation {
            program: "dnf".into(),
            args,
        })
    }
}
