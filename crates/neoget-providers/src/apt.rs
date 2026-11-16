//! apt adapter: command construction only; execution is not authorized here.
use crate::{Adapter, Host, Invocation, Operation};
/// Stable apt provider adapter.
pub struct Apt;
impl Adapter for Apt {
    fn id(&self) -> &'static str {
        "apt"
    }
    fn applies(&self, host: Host) -> bool {
        matches!(host, Host::Linux)
    }
    fn executable(&self) -> &'static str {
        "dpkg-query"
    }
    fn command(&self, operation: Operation, _package: Option<&str>) -> Result<Invocation, String> {
        if operation == Operation::Inventory && _package.is_some() {
            return Err("provider inventory does not accept a package filter".into());
        }
        let args = match operation {
            Operation::Inventory => vec!["-W", "-f=${binary:Package}\t${Version}\n"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            Operation::UpgradeAll => return Err("unsupported operation: apt-get upgrade".into()),
        };
        Ok(Invocation {
            program: "dpkg-query".into(),
            args,
        })
    }
}
