//! pacman adapter: command construction only; execution is not authorized here.
use crate::{Adapter, Host, Invocation, Operation};
/// Stable pacman provider adapter.
pub struct Pacman;
impl Adapter for Pacman {
    fn id(&self) -> &'static str {
        "pacman"
    }
    fn applies(&self, host: Host) -> bool {
        matches!(host, Host::Linux)
    }
    fn executable(&self) -> &'static str {
        "pacman"
    }
    fn command(&self, operation: Operation, package: Option<&str>) -> Result<Invocation, String> {
        let args = match operation {
            Operation::Inventory => vec!["-Q"].into_iter().map(str::to_owned).collect(),
            Operation::UpgradeAll => {
                if package.is_some() {
                    return Err(
                        "targeted pacman upgrades are unsupported; all-system Syu only".into(),
                    );
                }
                vec!["-Syu".into()]
            }
        };
        Ok(Invocation {
            program: "pacman".into(),
            args,
        })
    }
}
