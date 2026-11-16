//! yum adapter: command construction only; execution is not authorized here.
use crate::{Adapter, Host, Invocation, Operation};
/// Stable yum provider adapter.
pub struct Yum;
impl Adapter for Yum {
    fn id(&self) -> &'static str {
        "yum"
    }
    fn applies(&self, host: Host) -> bool {
        matches!(host, Host::Linux)
    }
    fn executable(&self) -> &'static str {
        "yum"
    }
    fn command(&self, operation: Operation, _package: Option<&str>) -> Result<Invocation, String> {
        let args = match operation {
            Operation::Inventory => vec!["list", "installed"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            Operation::UpgradeAll => return Err("unsupported operation: yum update".into()),
        };
        Ok(Invocation {
            program: "yum".into(),
            args,
        })
    }
}
