//! winget adapter: command construction only; execution is not authorized here.
use crate::{Adapter, Host, Invocation, Operation};
/// Stable winget provider adapter.
pub struct Winget;
impl Adapter for Winget {
    fn id(&self) -> &'static str {
        "winget"
    }
    fn applies(&self, host: Host) -> bool {
        matches!(host, Host::Windows)
    }
    fn executable(&self) -> &'static str {
        "winget"
    }
    fn command(&self, operation: Operation, _package: Option<&str>) -> Result<Invocation, String> {
        let args = match operation {
            Operation::Inventory => vec!["list"].into_iter().map(str::to_owned).collect(),
            Operation::UpgradeAll => {
                return Err("unsupported operation: winget upgrade --all".into());
            }
        };
        Ok(Invocation {
            program: "winget".into(),
            args,
        })
    }
}
