//! aqua adapter: command construction only; execution is not authorized here.
use crate::{Adapter, Host, Invocation, Operation};
/// Stable aqua provider adapter.
pub struct Aqua;
impl Adapter for Aqua {
    fn id(&self) -> &'static str {
        "aqua"
    }
    fn applies(&self, host: Host) -> bool {
        matches!(host, Host::Windows | Host::Linux | Host::Macos)
    }
    fn executable(&self) -> &'static str {
        "aqua"
    }
    fn command(&self, operation: Operation, _package: Option<&str>) -> Result<Invocation, String> {
        if operation == Operation::Inventory && _package.is_some() {
            return Err("provider inventory does not accept a package filter".into());
        }
        let args = match operation {
            Operation::Inventory => vec!["list"].into_iter().map(str::to_owned).collect(),
            Operation::UpgradeAll => {
                return Err("unsupported operation: aqua config-scoped install or upgrade".into());
            }
        };
        Ok(Invocation {
            program: "aqua".into(),
            args,
        })
    }
}
