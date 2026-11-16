//! github adapter: command construction only; execution is not authorized here.
use crate::{Adapter, Host, Invocation, Operation};
/// Stable github provider adapter.
pub struct Github;
impl Adapter for Github {
    fn id(&self) -> &'static str {
        "github"
    }
    fn applies(&self, host: Host) -> bool {
        matches!(host, Host::Windows | Host::Linux | Host::Macos)
    }
    fn executable(&self) -> &'static str {
        "gh"
    }
    fn command(&self, operation: Operation, _package: Option<&str>) -> Result<Invocation, String> {
        if operation == Operation::Inventory && _package.is_some() {
            return Err("provider inventory does not accept a package filter".into());
        }
        let args = match operation {
            Operation::Inventory => vec!["extension", "list"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            Operation::UpgradeAll => {
                return Err("unsupported operation: gh extension upgrade --all".into());
            }
        };
        Ok(Invocation {
            program: "gh".into(),
            args,
        })
    }
}
